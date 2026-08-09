# Capture Inbox Read Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将采集箱批次与条目读取 SQL 从超大 `capture_inbox` 用例模块移入私有只读仓储，同时保持所有命令、序列化和数据库行为不变。

**Architecture:** 新建 `capture_inbox_repository.rs`，作为 `capture_inbox` 的私有子模块，负责批次列表、批次详情、单项读取以及数据库行映射。`capture_inbox.rs` 继续拥有公开模型、错误类型和写入编排，通过 re-export 保留现有 `list_capture_batches` 与 `get_capture_batch_detail` 路径；写入用例只依赖私有 `query_batch`、`get_capture_item`。

**Tech Stack:** Rust 2024、rusqlite/SQLCipher、serde、PowerShell 架构契约、Cargo tests。

## Global Constraints

- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs` 及任何 OCR/识别算法。
- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 保持 Tauri 命令签名、Specta bindings、数据库 schema、SQL 查询语义、排序、稳定错误类型和序列化字段完全不变。
- 不新增依赖，不暂存、不提交；保留工作区全部既有修改。
- 先让架构契约失败，再移动生产代码；运行定向 Rust 测试、边界契约、格式与 Clippy。

---

### Task 1: 用架构契约锁定只读仓储边界

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Requires: `src-tauri/src/modules/capture_inbox_repository.rs`
- Enforces: `get_capture_batch_detail`、`list_capture_batches`、`query_batch` 由只读仓储拥有，父用例模块不得重新定义。

- [x] **Step 1: 添加失败契约**

在现有 capture asset contract 后加入：

```powershell
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn get_capture_batch_detail' `
    'Capture batch detail reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn list_capture_batches' `
    'Capture batch list reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn query_batch' `
    'Capture batch summary reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn get_capture_item' `
    'Capture item reads must remain in the inbox read repository'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_batch_detail' `
    'Capture batch detail SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn list_capture_batches' `
    'Capture batch list SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_batch\(' `
    'Capture batch summary SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn get_capture_item\(' `
    'Capture item SQL must not move back into inbox orchestration'
```

- [x] **Step 2: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，指出 `capture_inbox_repository.rs` 缺失。

---

### Task 2: 提取采集箱只读仓储

**Files:**
- Create: `src-tauri/src/modules/capture_inbox_repository.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`

**Interfaces:**
- Consumes from parent: `CaptureBatchDetail`、`CaptureBatchState`、`CaptureBatchSummary`、`CaptureDraftSummary`、`CaptureInboxError`、`CaptureItemSummary`、`CapturePairSuggestionSummary`。
- Produces:

```rust
pub fn list_capture_batches(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<CaptureBatchSummary>, CaptureInboxError>

pub fn get_capture_batch_detail(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchDetail, CaptureInboxError>

pub fn query_batch(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchSummary, CaptureInboxError>

pub fn get_capture_item(
    connection: &Connection,
    account_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemSummary, CaptureInboxError>
```

- [x] **Step 1: 创建仓储并原样移动读取代码**

移动 `list_capture_batches`、`get_capture_batch_detail`、`map_batch_row`、`query_batch`、`parse_state`、`get_capture_item`；提取共享的 `map_item_row`，确保列表与单项读取的字段索引完全一致。

- [x] **Step 2: 在父模块建立私有依赖与兼容 re-export**

```rust
#[path = "capture_inbox_repository.rs"]
mod capture_inbox_repository;

pub use capture_inbox_repository::{get_capture_batch_detail, list_capture_batches};
use capture_inbox_repository::{get_capture_item, query_batch};
```

仓储位于父模块的私有子模块中；函数使用 `pub` 以允许父模块 re-export，但仓储模块自身不对外暴露。父模块保持现有 `modules::capture_inbox::*` 调用路径。

- [x] **Step 3: 运行定向测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store --test capture_inbox_command`

Expected: 两个集成测试目标全部通过。

- [x] **Step 4: 运行识别与 LAN 兼容回归**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition --test capture_inbox_command`

Expected: 外部调用仍通过原路径编译并通过测试。

---

### Task 3: 架构和仓库级验证

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-capture-inbox-read-repository.md`

- [x] **Step 1: 运行边界与格式门禁**

Run: `pnpm contract:rust-boundaries`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

- [x] **Step 2: 运行 Clippy**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

- [x] **Step 3: 核对结构结果**

确认 `capture_inbox.rs` 行数下降、仓储只依赖父模块模型/错误、父模块不再包含详情读取 SQL，并运行 `git diff --check`。

- [x] **Step 4: 记录验证结果**

记录测试目标、契约、格式、Clippy、文件行数、暂存区状态以及用户已有识别文件未被触碰。

## Self-Review

- 需求覆盖：仓储职责、兼容路径、行为回归、架构契约和排除项均有对应任务。
- Placeholder scan：无 TBD、TODO 或未定义实现项。
- 类型一致性：计划中的四个仓储函数均复用父模块现有返回类型与错误类型，公开调用路径不变。

## Verification Record

- TDD 契约：新增边界在仓储文件不存在时按预期失败；实现后通过。
- 采集箱回归：`capture_inbox_command` 2 项、`capture_inbox_store` 13 项全部通过。
- 识别兼容：`capture_recognition` 23 项通过，2 项因要求本地真实语料与 hash-pinned OCR runtime 按设计忽略。
- LAN 兼容：`modules::capture_lan::tests` 10 项全部通过。
- 静态门禁：Rustfmt check、Clippy `--all-targets --all-features -- -D warnings` 全部通过。
- 架构门禁：四个读取入口的仓储所有权与父模块拒绝规则全部通过。
- 结构结果：`capture_inbox.rs` 从 2502 行降至 2234 行；新 `capture_inbox_repository.rs` 为 267 行。
- 差异检查：`git diff --check` 通过，仅输出工作区既有 LF/CRLF 转换警告；暂存区为空。
- 本轮未修改用户已有的 `recognition_visual_split.rs`，未修改命令签名、bindings、schema、依赖或上线前事项。
- Windows 测试输出仍包含该主机既有的 OpenSSL PDB 和 SQLCipher `VirtualLock` 警告洪泛，但所有已运行测试均以退出码 0 完成。
