# Capture Organizer Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 将采集收件箱的布局、移动、角色、合并、配对、草稿和删除事务从创建/导入 facade 中抽成私有模块，并保持命令与公开 Rust 路径兼容。

**Architecture:** `capture_inbox.rs` 继续拥有 DTO、错误、批次创建、图片导入、收集阶段更新和主题分配；新的私有 `capture_organizer_transaction.rs` 拥有组织阶段的九个公开变更入口及组织专用 SQLite 原子 helper，`capture_inbox_transaction_support.rs` 拥有裁剪与组织事务共同使用的四个 helper。父模块通过 `pub use` 保持 `modules::capture_inbox::*` 调用路径不变，共享的状态/版本 guard 留在父模块。

**Tech Stack:** Rust 2024、Rusqlite/SQLCipher、Serde、现有采集和识别集成测试、PowerShell architecture contract。

## Global Constraints

- 不修改 Tauri command、bindings、数据库 schema、迁移、依赖、公开 DTO 或错误语义。
- 保留组织阶段、revision 乐观锁、配对失效、草稿位置压缩、孤儿资产回收和文件删除顺序。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立测试基线和失败架构契约

**Files:**
- Verify: `src-tauri/tests/capture_inbox_store.rs`
- Verify: `src-tauri/tests/capture_recognition.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Public mutation functions:

```text
apply_capture_layout
move_capture_item
stage_capture_item_role
merge_capture_card
apply_capture_pair_suggestions
delete_capture_draft
update_capture_draft
remove_capture_item
discard_capture_batch
```

- [x] **Step 1: 运行采集组织基线测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store --test capture_recognition
```

Expected: 收件箱 14 项通过；识别 24 项通过、2 项因依赖真实图片语料及固定哈希 OCR 运行时而忽略；合计 38 项通过、2 项忽略。

- [x] **Step 2: 添加失败架构契约**

在 `scripts/rust-boundary-contract.ps1` 要求组织模块拥有九个公开变更入口和批次级配对失效 helper，并要求共享支持模块拥有四个裁剪/组织共用 helper：

```powershell
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn apply_capture_layout' `
    'Capture layout transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn apply_capture_pair_suggestions' `
    'Capture pair application transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn remove_capture_item' `
    'Capture item removal transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^fn invalidate_active_pairs_for_batch' `
    'Capture batch pair invalidation must remain organizer-local'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn invalidate_active_pairs_for_item' `
    'Capture item pair invalidation must remain in transaction support'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn delete_asset_row_if_orphan' `
    'Capture orphan cleanup must remain in transaction support'
```

拒绝 `capture_inbox.rs` 重新定义九个公开入口或上述事务 helper；继续要求 `ensure_organizing_revision` 留在父模块。

- [x] **Step 3: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，`capture_organizer_transaction.rs` 尚不存在。

---

### Task 2: 提取组织事务模块

**Files:**
- Create: `src-tauri/src/modules/capture_organizer_transaction.rs`
- Create: `src-tauri/src/modules/capture_inbox_transaction_support.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/modules/capture_crop.rs`

**Interfaces:**
- Child consumes:

```rust
use super::{
    ApplyCaptureLayout, ApplyCapturePairSuggestions, CaptureBatchDetail, CaptureBatchState,
    CaptureInboxError, CaptureLayoutMode, MAX_CAPTURE_BATCH_ITEMS, MergeCaptureCard,
    MoveCaptureItem, StageCaptureItemRole, UpdateCaptureDraft, ensure_organizing_revision,
    get_capture_batch_detail, normalize_subject, remove_encrypted_blob,
    validate_relative_asset_path,
};
use super::capture_inbox_repository::{get_capture_item, query_batch};
use super::capture_inbox_transaction_support::{
    delete_asset_row_if_orphan, invalidate_active_pairs_for_item, repack_link_positions,
    touch_batch,
};
```

- Parent compatibility:

```rust
#[path = "capture_organizer_transaction.rs"]
mod capture_organizer_transaction;

pub use capture_organizer_transaction::{
    apply_capture_layout, apply_capture_pair_suggestions, delete_capture_draft,
    discard_capture_batch, merge_capture_card, move_capture_item, remove_capture_item,
    stage_capture_item_role, update_capture_draft,
};
```

- [x] **Step 1: 移动连续事务实现**

将 `apply_capture_layout` 起至文件末尾移动到新模块，包括九个公开入口和：

```text
query_batch_item_ids / insert_draft / insert_draft_with_subject
repack_draft_positions / insert_link
invalidate_active_pairs_for_batch
```

将 `invalidate_active_pairs_for_item`、`touch_batch`、`repack_link_positions` 和
`delete_asset_row_if_orphan` 移到共享支持模块，由组织和裁剪事务直接导入，避免裁剪模块依赖组织模块。

将 `ensure_organizing_revision` 从移动段移回父模块并设为 `pub(super)`，供主题分配与子模块共同使用。

- [x] **Step 2: 声明私有模块并重导出入口**

添加 Interfaces 中的声明和 `pub use`；不修改 commands、LAN API 或测试调用者。

- [x] **Step 3: 运行相关集成测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store --test capture_recognition
```

Expected: 38 项通过、2 项按设计忽略。

---

### Task 3: 静态门禁和本地复审

**Files:**
- Verify: `src-tauri/src/modules/capture_inbox.rs`
- Verify: `src-tauri/src/modules/capture_organizer_transaction.rs`
- Verify: `src-tauri/src/modules/capture_inbox_transaction_support.rs`
- Verify: `src-tauri/src/modules/capture_crop.rs`
- Verify: `src-tauri/src/commands/capture_inbox.rs`
- Verify: `src-tauri/src/modules/capture_lan_api.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: command imports、LAN 删除入口、revision 冲突、数据库原子性、配对状态及孤儿文件清理。

- [x] **Step 1: 运行架构、格式和 Clippy**

Run:

```powershell
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: 全部通过。

- [x] **Step 2: 本地代码复核**

确认九个组织事务入口及组织专用 helper 只在组织子模块；四个共用 helper 只在共享支持子模块；父模块继续拥有 DTO、创建、导入和主题分配；子模块不公开额外符号；commands、LAN API、bindings、schema、依赖和 OCR 文件未因本批修改。

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

- 需求覆盖：组织事务职责、公开兼容、共享 guard、测试和静态门禁均有明确步骤。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊实现步骤。
- 类型一致性：九个父模块重导出名称与现有 commands、LAN API 和测试导入完全一致。

## Verification Record

- 基线和绿灯相关原生测试均为 38 项通过、2 项忽略：
  `capture_inbox_store` 14 项通过，`capture_recognition` 24 项通过、2 项因需要真实图片语料及固定哈希 OCR 运行时而忽略。
- 红灯架构契约按预期因 `capture_organizer_transaction.rs` 尚不存在而失败。
- 首次编译审计发现裁剪事务也合法复用四个 helper；因此增加
  `capture_inbox_transaction_support.rs`，避免形成裁剪到组织模块的概念依赖。
- `pnpm contract:rust-boundaries`、MSVC wrapper 下的 `cargo fmt --check`，
  以及 all-target/all-feature `cargo clippy -- -D warnings` 均通过。
- 行数由单一 facade 的 1290 行收敛为：facade 515 行、组织事务子模块
  725 行、共享支持子模块 74 行。
- 本地复审未发现 Critical 或 Important 问题；公开 facade 路径、签名、错误映射、事务提交顺序，以及提交后再删除加密文件的顺序均保留。
- commands、LAN API、bindings、schema、依赖和 OCR 实现未因本批修改；
  `capture_crop.rs` 仅调整四个共享 helper 的导入路径。
- `git diff --check` 未发现空白错误（Git 仍输出已有 LF 到 CRLF 提示），暂存区为空。
- 原生测试仍输出已有 OpenSSL PDB `LNK4099` 和 SQLCipher
  `VirtualLock LastError=1453` 警告，但命令成功退出。
