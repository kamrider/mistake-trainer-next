# Sync Conflict Resolution Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 将用户选择本地或远端版本后的同步冲突解决事务，从同步拉取合并 facade 中提取为私有模块，同时保持现有命令、Rust 路径、数据库原子性和错误语义。

**Architecture:** `sync_conflicts.rs` 继续拥有公开 DTO/错误、冲突列表查询、同步拉取所需的快照/冲突记录/合并/outbox helper，以及本地实体加载；新的私有 `sync_conflict_resolution.rs` 完整拥有按字段和按实体解决冲突的事务入口、选值校验、实体写入、远端删除和最终 revision/outbox 收敛。父模块通过 `pub use` 保持 `modules::sync_conflicts::*` 调用路径不变。

**Tech Stack:** Rust 2024、Rusqlite/SQLCipher、Serde、现有同步冲突和拉取集成测试、PowerShell architecture contract。

## Global Constraints

- 不修改 Tauri command、TypeScript bindings、数据库 schema、迁移、依赖或公开 DTO。
- 保留 account/profile 隔离、字段与整实体原子解决、resolution audit、revision 单调性、tombstone、outbox 和最后一个学习档案保护。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立失败架构契约

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`
- Verify: `src-tauri/tests/sync_conflicts.rs`
- Verify: `src-tauri/tests/sync_pull.rs`

**Interfaces:**
- 新模块公开入口：

```rust
pub fn resolve_sync_conflict_field(
    connection: &mut rusqlite::Connection,
    account_id: &str,
    profile_id: &str,
    input: ResolveSyncConflictFieldInput,
    now_utc_ms: i64,
) -> Result<Vec<SyncConflictSummary>, SyncConflictError>;

pub fn resolve_sync_conflict_entity(
    connection: &mut rusqlite::Connection,
    account_id: &str,
    profile_id: &str,
    input: ResolveSyncConflictEntityInput,
    now_utc_ms: i64,
) -> Result<Vec<SyncConflictSummary>, SyncConflictError>;
```

- [x] **Step 1: 记录当前行为基线**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_conflicts --test sync_pull
```

Expected: `sync_conflicts` 9 项、`sync_pull` 8 项，共 17 项通过。

- [x] **Step 2: 添加解决事务边界契约**

在 `scripts/rust-boundary-contract.ps1` 添加：

```powershell
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^pub fn resolve_sync_conflict_field' `
    'Field conflict resolution must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^pub fn resolve_sync_conflict_entity' `
    'Entity conflict resolution must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn resolve_rows' `
    'Conflict resolution orchestration must remain transaction-local'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn apply_remote_delete' `
    'Remote deletion application must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn finalize_resolved_entity' `
    'Conflict revision and outbox finalization must remain transaction-local'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^pub fn resolve_sync_conflict_(field|entity)' `
    'Sync conflict facade must not own resolution transaction bodies'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^fn (resolve_rows|apply_remote_delete|finalize_resolved_entity)' `
    'Sync conflict facade must not absorb resolution internals'
Require-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^pub fn list_sync_conflicts' `
    'Conflict list reads must remain in the sync conflict facade'
```

- [x] **Step 3: 运行契约确认红灯**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，因为 `src-tauri/src/modules/sync_conflict_resolution.rs` 尚不存在。

---

### Task 2: 提取完整解决事务

**Files:**
- Create: `src-tauri/src/modules/sync_conflict_resolution.rs`
- Modify: `src-tauri/src/modules/sync_conflicts.rs`

**Interfaces:**
- 子模块从父模块消费：

```rust
use super::{
    ResolveSyncConflictEntityInput, ResolveSyncConflictFieldInput, SyncConflictChoice,
    SyncConflictError, SyncConflictSummary, cleanup_deleted_profile_sync_state,
    list_sync_conflicts, load_local_export, load_local_problem, load_local_profile,
    replace_entity_outbox,
};
```

- 父模块兼容导出：

```rust
#[path = "sync_conflict_resolution.rs"]
mod sync_conflict_resolution;

pub use sync_conflict_resolution::{
    resolve_sync_conflict_entity, resolve_sync_conflict_field,
};
```

- [x] **Step 1: 创建事务子模块并移动类型与入口**

把 `ConflictRow`、`ValidExportConfiguration`、`resolve_sync_conflict_field`、
`resolve_sync_conflict_entity`、`load_conflict_by_id`、
`load_conflicts_for_entity` 和 `conflict_row` 原样移动到
`sync_conflict_resolution.rs`。两个公开入口继续各自创建一个 SQLite
transaction，在读取剩余冲突后提交并返回。

- [x] **Step 2: 移动选择应用和最终收敛**

把 `resolve_rows`、`apply_field_value`、`apply_problem_field`、
`apply_export_field`、`apply_remote_delete` 和 `finalize_resolved_entity`
原样移动到子模块。保留以下不变量：

```text
invalid selected value => entire transaction rolls back
remote __deleted__ => resolve all sibling fields atomically
last learner profile => SyncConflictError::LastProfile
accepted remote delete => clear snapshot and canonical outbox
kept local delete => advance revision above tombstone and remove tombstone
all fields resolved => exactly one final revision/outbox convergence
```

- [x] **Step 3: 建立私有模块和兼容 re-export**

在 `sync_conflicts.rs` 声明私有子模块并按 Interfaces 重导出两个入口。
`load_local_profile`、`load_local_problem`、`load_local_export` 保持
父模块私有 `fn`；Rust 的 descendant privacy 已允许私有子模块复用，
无需扩大到 `pub(super)`。`replace_entity_outbox` 和
`cleanup_deleted_profile_sync_state` 保持 `pub(crate)`，因为
`sync_pull.rs` 仍需要它们。

- [x] **Step 4: 清理 facade 的专用依赖**

从 `sync_conflicts.rs` 删除只被解决事务使用的 `ProfileName` 和
`ExportLayout` import；保留 `BTreeMap`、`BTreeSet`（冲突字段去重仍使用）、
`DeserializeOwned`、同步合并 DTO 和拉取 helper。不得修改 commands、
bindings、sync pull 或测试调用路径。

- [x] **Step 5: 运行相关同步测试确认绿灯**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_conflicts --test sync_pull
```

Expected: 17/17 通过。

---

### Task 3: 静态门禁和本地复审

**Files:**
- Verify: `src-tauri/src/modules/sync_conflicts.rs`
- Verify: `src-tauri/src/modules/sync_conflict_resolution.rs`
- Verify: `src-tauri/src/modules/sync_pull.rs`
- Verify: `src-tauri/src/commands/sync.rs`
- Verify: `src-tauri/src/bindings.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: `modules::sync_conflicts::{resolve_sync_conflict_field, resolve_sync_conflict_entity}`、command imports、account/profile scope、SQLite transaction、audit、revision、tombstone、snapshot 与 outbox。

- [x] **Step 1: 运行架构、格式和 Clippy**

Run:

```powershell
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: 全部通过。

- [x] **Step 2: 核对职责与公开面**

确认父模块拥有同步拉取合并支持和只读列表；子模块只公开两个解决入口，
并拥有全部选择应用及最终收敛逻辑；命令继续从父模块导入；schema、迁移、
依赖、bindings、sync pull 和 OCR 文件未因本批修改。

- [x] **Step 3: 运行最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 4: 记录验证结果**

在本文追加基线、契约红/绿灯、17 项测试、静态门禁、模块行数、本地复审、
已有工具链警告和范围排除。

## Self-Review

- 需求覆盖：字段/实体解决、远端删除、事务回滚、audit、revision、tombstone、
  snapshot 和 outbox 均由同一个私有事务模块覆盖。
- Placeholder scan：无 TBD、TODO、未定义接口或“稍后处理”步骤。
- 类型一致性：两个公开函数签名和父模块 re-export 与现有 command、测试调用者完全一致；
  三个本地加载函数保持父模块私有，未形成新的 crate-wide 接口。

## Verification Record

- 基线和绿灯同步测试均为 17/17 通过：
  `sync_conflicts` 9 项、`sync_pull` 8 项。
- 红灯架构契约按预期因 `sync_conflict_resolution.rs` 尚不存在而失败；
  提取后契约通过。
- 首次编译审计发现 `record_conflicts` 仍需要 `BTreeSet` 做字段去重，
  因此该 import 正确保留在 facade；业务和事务边界没有改变。
- 本地复审进一步收紧了计划：三个 `load_local_*` helper 保持父模块私有
  `fn`，利用 Rust descendant privacy 供子模块访问，没有扩大可见性。
- `pnpm contract:rust-boundaries`、MSVC wrapper 下的 `cargo fmt --check`
  和 all-target/all-feature `cargo clippy -- -D warnings` 均通过。
- `command_contract` 8/8 通过；公开函数仍由
  `modules::sync_conflicts::*` 重导出，commands 和 bindings 不需要改动。
- 行数由单一 `sync_conflicts.rs` 的 1328 行收敛为：facade 693 行、
  私有解决事务模块 657 行。
- 子模块只公开 `resolve_sync_conflict_field` 和
  `resolve_sync_conflict_entity`；两个入口各自创建并提交一个 SQLite
  transaction，所有选择应用、删除级联和最终 revision/outbox 收敛均在
  transaction 内完成。
- 本地复审未发现 Critical 或 Important 问题。sync pull、commands、
  bindings、schema、迁移、依赖和 OCR 文件未因本批修改。
- `git diff --check` 和本批文件尾随空白扫描均通过；计划无未勾选步骤，
  暂存区为空。
- `bindings_contract` 不是当前 Cargo 清单中的测试目标；该误调用未执行
  测试、未修改文件。绑定没有 DTO 变化，并由全目标 Clippy 编译覆盖。
- 原生测试仍输出已有 OpenSSL PDB `LNK4099` 警告，但命令成功退出。
