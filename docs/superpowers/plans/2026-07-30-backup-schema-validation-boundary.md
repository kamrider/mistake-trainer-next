# Backup Schema Validation Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将备份数据库的 schema 形状、版本兼容和单账户完整性验证从恢复编排中抽成私有模块，同时保持备份格式与公开行为不变。

**Architecture:** `backup.rs` 继续拥有备份创建、包验证、恢复候选、原子切换和失败恢复；新的私有 `backup_schema_validation.rs` 只拥有 SQLite page budget、版本化表/列/索引形状检查及单账户归属检查。父模块仅导入两个 `pub(super)` 入口，所有低层 schema helper 保持子模块私有。

**Tech Stack:** Rust 2024、Rusqlite/SQLCipher、PowerShell architecture contract、现有集成测试。

## Global Constraints

- 不修改备份 manifest、数据库 schema、迁移、Tauri command、bindings、依赖或公开 Rust API。
- 保留 4 GB 数据库预算及 schema 1–17 的现有兼容策略。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立基线与失败架构契约

**Files:**
- Verify: `src-tauri/tests/backup_store.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Existing validation entry points:

```rust
fn ensure_database_budget(connection: &Connection) -> Result<(), BackupError>
fn ensure_single_account(
    connection: &Connection,
    account_id: &str,
    schema_version: i64,
) -> Result<(), BackupError>
```

- [x] **Step 1: 运行备份基线测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: `backup_store` 全部现有测试通过。

- [x] **Step 2: 添加失败架构契约**

在 `scripts/rust-boundary-contract.ps1` 添加：

```powershell
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^pub\(super\) fn ensure_database_budget' `
    'Backup database size validation must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^pub\(super\) fn ensure_single_account' `
    'Backup account and schema integrity policy must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^fn table_columns_match' `
    'Backup table shape inspection must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^fn index_columns_match' `
    'Backup index shape inspection must remain in the schema validation module'
```

并拒绝 `backup.rs` 重新定义 `ensure_database_budget`、`ensure_single_account`、`table_exists`、`column_exists`、`table_columns_match` 或 `index_columns_match`。

- [x] **Step 3: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，`backup_schema_validation.rs` 尚不存在。

---

### Task 2: 提取 schema 验证策略

**Files:**
- Create: `src-tauri/src/modules/backup_schema_validation.rs`
- Modify: `src-tauri/src/modules/backup.rs`

**Interfaces:**
- Child consumes:

```rust
use super::{BackupError, MAX_DATABASE_BYTES};
```

- Parent consumes:

```rust
use backup_schema_validation::{ensure_database_budget, ensure_single_account};
```

- [x] **Step 1: 移动完整验证实现**

将 `ensure_database_budget` 起至文件末尾的连续实现移动到新模块，包括：

```text
ensure_database_budget / pragma_u64 / ensure_single_account
table_exists / column_exists / table_columns_match / index_columns_match
```

新文件头：

```rust
use rusqlite::Connection;

use super::{BackupError, MAX_DATABASE_BYTES};
```

仅 `ensure_database_budget` 与 `ensure_single_account` 使用 `pub(super)`；其余 helper 保持私有。

- [x] **Step 2: 声明私有模块并导入入口**

在 `backup.rs` 添加：

```rust
#[path = "backup_schema_validation.rs"]
mod backup_schema_validation;

use backup_schema_validation::{ensure_database_budget, ensure_single_account};
```

不修改两个入口的调用点、错误类型或返回值。

- [x] **Step 3: 运行备份集成测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: 基线测试数量不变且全部通过。

---

### Task 3: 静态门禁与本地复审

**Files:**
- Verify: `src-tauri/src/modules/backup.rs`
- Verify: `src-tauri/src/modules/backup_schema_validation.rs`
- Verify: `src-tauri/src/commands/backup.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: 备份创建、密文/manifest 校验、schema 1–17 兼容、单账户隔离、恢复候选及原子恢复。

- [x] **Step 1: 运行架构、格式和 Clippy**

Run:

```powershell
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: 全部通过。

- [x] **Step 2: 本地代码复核**

确认所有版本化表/列/索引检查只在子模块；父模块继续拥有包和恢复编排；新模块除两个入口外无对父模块可见符号；commands、bindings、schema、依赖和 OCR 文件未因本批修改。

- [x] **Step 3: 运行最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 4: 记录验证结果**

在本文追加基线、契约红/绿灯、测试数量、静态门禁、模块行数、本地审查和范围排除。

## Self-Review

- 需求覆盖：职责提取、公开兼容、版本策略保持、测试和静态门禁均有对应步骤。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊实现步骤。
- 类型一致性：子模块只消费父模块现有 `BackupError` 与数据库预算常量，父模块只消费两个验证入口。

## Verification Record

- 基线：拆分前 `backup_store` 20/20 通过。
- 红灯：新增架构契约先因 `backup_schema_validation.rs` 不存在而失败。
- 绿灯：拆分后 `backup_store` 仍为 20/20，覆盖备份创建、密文完整性、schema 1–17 兼容、单账户隔离、恢复候选和失败清理。
- 静态门禁：Rust architecture boundary contract、`cargo fmt --all --check`、全目标全特性 Clippy `-D warnings` 均通过。
- 模块规模：`backup.rs` 从 1547 行降为 919 行；私有 `backup_schema_validation.rs` 为 634 行。
- 本地复审：未发现 Critical 或 Important 问题；子模块只有 `ensure_database_budget` 与 `ensure_single_account` 两个 `pub(super)` 入口，低层 schema helper 均为私有。
- 范围核对：本批未改 backup command、bindings、schema、迁移、依赖或 `recognition_visual_split.rs`；关键排除文件时间戳仍为本批开始前。
- 工作区：`git diff --check` 通过，暂存区为空；所有本批修改保持未暂存、未提交。
- 环境告警：测试仍输出既有 OpenSSL PDB `LNK4099` 与 SQLCipher `VirtualLock LastError=1453` 告警，但命令退出码为 0。
