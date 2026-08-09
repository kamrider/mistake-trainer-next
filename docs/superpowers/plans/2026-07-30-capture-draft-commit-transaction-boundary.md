# Capture Draft Commit Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将就绪采集草稿转换为正式题目、资源关系和同步操作的原子事务从 `capture_inbox` 编排模块提取为独立私有提交模块，同时保持公开 API、错误语义和数据库行为不变。

**Architecture:** 新建 `capture_commit.rs` 作为 `capture_inbox` 的私有子模块，拥有提交报告 DTO、就绪草稿及资源查询、题目与同步操作写入、草稿清理和批次完成状态迁移。父模块继续拥有采集、布局、编辑、删除等编排职责以及这些操作共同使用的 Organizing/revision 前置条件，并通过 `pub use` 保留 `modules::capture_inbox::{CaptureCommitReport, commit_ready_capture_drafts}` 路径。

**Tech Stack:** Rust 2024、rusqlite/SQLCipher、serde/serde_json、specta、UUID v7、PowerShell 架构契约、Cargo tests。

## Global Constraints

- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs` 及任何 OCR/识别算法。
- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 保持 Tauri 命令签名、Specta bindings、数据库 schema、同步 payload 结构、错误类型、事务原子性和草稿选择顺序完全不变。
- `InvalidState` 必须在状态与 revision 同时不合法时优先于 `RevisionConflict`。
- 不新增依赖，不暂存、不提交；保留工作区全部既有修改。
- 先让架构契约失败，再移动生产代码；不得复制保留两套实现。

---

### Task 1: 锁定提交事务所有权和错误优先级

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`
- Modify: `src-tauri/tests/capture_inbox_store.rs`

**Interfaces:**
- Requires: `src-tauri/src/modules/capture_commit.rs`
- Enforces: 提交入口和提交专属查询只由提交事务模块定义；共享状态校验留在父编排模块。
- Characterizes: `commit_ready_capture_drafts(&mut Connection, &str, &str, &str, u32, i64) -> Result<CaptureCommitReport, CaptureInboxError>`。

- [x] **Step 1: 添加失败架构契约**

```powershell
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^pub fn commit_ready_capture_drafts' `
    'Capture draft commit transaction must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_ready_drafts' `
    'Ready draft selection must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_draft_asset_links' `
    'Committed asset ordering must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_asset_sync_payload' `
    'Committed asset sync payloads must remain in the commit module'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn commit_ready_capture_drafts' `
    'Capture commit implementation must not move back into inbox orchestration'
Require-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn ensure_organizing_revision' `
    'Shared organizing state validation must remain in inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_ready_drafts' `
    'Ready draft SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_draft_asset_links' `
    'Committed asset SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_asset_sync_payload' `
    'Committed asset sync serialization must not move back into inbox orchestration'
```

- [x] **Step 2: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，指出 `capture_commit.rs` 缺失。

- [x] **Step 3: 添加状态优先级回归**

```rust
#[test]
fn committing_requires_organizing_before_revision_match() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );

    let error = commit_ready_capture_drafts(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        batch.revision.saturating_add(1),
        20,
    )
    .expect_err("collecting state must take precedence over stale revision");

    assert!(matches!(error, CaptureInboxError::InvalidState));
}
```

- [x] **Step 4: 运行定向回归确认既有语义**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store committing_requires_organizing_before_revision_match`

Expected: 1 项通过。

---

### Task 2: 提取完整草稿提交事务

**Files:**
- Create: `src-tauri/src/modules/capture_commit.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`

**Interfaces:**
- Produces:

```rust
pub struct CaptureCommitReport {
    pub committed_problem_ids: Vec<String>,
    pub committed_count: u32,
    pub remaining_draft_count: u32,
}

pub fn commit_ready_capture_drafts(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    now_utc_ms: i64,
) -> Result<CaptureCommitReport, CaptureInboxError>;
```

- Consumes from parent: `query_batch`、`ensure_organizing_revision`、`CaptureBatchState`、`CaptureInboxError`。
- Public compatibility:

```rust
pub use capture_commit::{CaptureCommitReport, commit_ready_capture_drafts};
```

- [x] **Step 1: 移动提交 DTO 和专属私有类型**

把 `CaptureCommitReport`、`ReadyDraft` 和 `DraftAssetLink` 移入 `capture_commit.rs`；保持 serde/specta 派生和 camelCase 序列化不变。

- [x] **Step 2: 移动事务和查询实现**

原样移动 `commit_ready_capture_drafts`、`query_ready_drafts`、`query_draft_asset_links`、`asset_ids_for_role` 和 `query_asset_sync_payload`。共享的 `ensure_organizing_revision` 留在父模块供提交、布局、移动、合并、编辑和删除操作共同调用，并保持：

```rust
if batch.state != CaptureBatchState::Organizing {
    return Err(CaptureInboxError::InvalidState);
}
if batch.revision != expected_revision {
    return Err(CaptureInboxError::RevisionConflict);
}
```

并保持一个 `rusqlite::Transaction` 覆盖题目、资源关系、同步操作、草稿清理和批次状态迁移。

- [x] **Step 3: 建立私有模块和兼容 re-export**

父模块添加：

```rust
#[path = "capture_commit.rs"]
mod capture_commit;

pub use capture_commit::{CaptureCommitReport, commit_ready_capture_drafts};
```

删除父模块中已移动实现及仅因此存在的 import；不修改命令和调用者。

- [x] **Step 4: 运行采集箱定向测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store --test capture_inbox_command`

Expected: `capture_inbox_command` 2 项及 `capture_inbox_store` 全部通过。

---

### Task 3: 事务兼容与静态门禁

**Files:**
- Verify: `src-tauri/src/commands/capture_inbox.rs`
- Verify: `src-tauri/src/bindings.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: 命令返回的 `CaptureCommitReport` 类型路径、Specta 类型注册和数据库事务行为。

- [x] **Step 1: 运行架构与格式检查**

Run: `pnpm contract:rust-boundaries`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

Expected: 均通过。

- [x] **Step 2: 运行 Clippy**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

Expected: 零告警通过。

- [x] **Step 3: 本地代码复核**

确认父模块不存在提交 SQL、同步 payload 或提交专属查询的重复实现；`capture_commit` 仍是私有子模块；命令、bindings、schema、依赖和 OCR 文件未因本批修改；运行 `git diff --check` 并确认暂存区为空。

- [x] **Step 4: 记录验证结果**

记录测试数量、错误优先级回归、架构契约、格式、Clippy、模块行数、暂存区状态和范围排除。

## Self-Review

- 需求覆盖：计划提取完整提交事务，不拆散其数据库原子性；所有外部类型和函数路径均有兼容 re-export。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊测试步骤。
- 类型一致性：函数参数、返回类型、字段名和当前实现完全一致；状态优先级测试使用当前 `CaptureInboxError`。

## Verification Record

- 架构 TDD：新增规则后先因 `capture_commit.rs` 缺失而失败；完成提取并修正共享校验归属后 `pnpm contract:rust-boundaries` 通过。
- 状态优先级：新增 Collecting 状态与 stale revision 同时发生的回归，确认 `InvalidState` 优先；定向测试 1/1 及完整测试中的同一用例均通过。
- 采集箱：`capture_inbox_command` 2/2、`capture_inbox_store` 14/14，共 16 项通过；覆盖提交成功、不完整草稿保留和注入失败时完整回滚。
- 静态门禁：`cargo fmt --check`、全 targets/features `cargo clippy -D warnings`、Rust 边界契约和 `git diff --check` 均通过。
- 本地代码审查：最初把 `ensure_organizing_revision` 误判为提交专属；编译证明父模块另有 7 个调用点后，将它恢复为共享编排前置条件，并同步修正计划与契约。最终无 Critical、Important 或 Minor 遗留问题。
- 所有权：提交入口、就绪草稿查询、资源排序和同步 payload 只在 `capture_commit.rs` 定义；父模块中没有提交 SQL 或同步 payload 重复实现。
- 模块体量：`capture_inbox.rs` 1290 行、`capture_commit.rs` 238 行；本批父模块由 1517 行降至 1290 行，连续三批由原 2502 行降至 1290 行。
- API/数据边界：`capture_commit` 是私有子模块；`CaptureCommitReport` 与 `commit_ready_capture_drafts` 继续经 `capture_inbox` 公开。命令、bindings、schema、依赖和同步 payload 结构未改。
- 工作区：Git 暂存区为空，未暂存、未提交；用户已有的 `recognition_visual_split.rs` 修改未触碰。
- 环境噪声：Windows 测试仍输出既有 OpenSSL PDB 与 SQLCipher `VirtualLock` 警告，但所有测试命令退出码为 0。
- 范围排除：未处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA。
