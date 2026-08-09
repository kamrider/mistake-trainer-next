# Capture Crop Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将采集图片预览、裁剪编码、原子应用和撤销从超大 `capture_inbox` 模块提取为私有裁剪事务模块，同时保持全部公开调用路径和行为不变。

**Architecture:** 新建 `capture_crop.rs` 作为 `capture_inbox` 的私有子模块，拥有裁剪 DTO、配方验证、图像编码、文件 staging、数据库事务和撤销清理。父模块继续拥有批次/草稿编排及共享私有辅助函数，并通过 `pub use`/`pub(crate) use` 保留命令、LAN、识别和基础设施当前使用的 `modules::capture_inbox::*` 路径。

**Tech Stack:** Rust 2024、rusqlite/SQLCipher、image、AES-GCM encrypted assets、serde/specta、PowerShell 架构契约、Cargo tests。

## Global Constraints

- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs` 及任何 OCR/识别算法。
- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 保持 Tauri 命令签名、Specta bindings、数据库 schema、裁剪配方 JSON、错误类型、容量限制、加密格式、事务原子性和资源清理行为完全不变。
- 不新增依赖，不暂存、不提交；保留工作区全部既有修改。
- 先让架构契约失败，再移动生产代码；不得通过复制在两个模块保留实现。

---

### Task 1: 锁定裁剪事务所有权

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Requires: `src-tauri/src/modules/capture_crop.rs`
- Enforces: 预览、编码、应用、撤销均由裁剪模块定义，父模块不得重新定义。

- [x] **Step 1: 添加失败契约**

```powershell
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn get_capture_item_preview' `
    'Capture preview reads must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn get_capture_crop_source_preview' `
    'Capture crop source preview reads must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub\(crate\) fn encode_crop' `
    'Capture crop encoding must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn apply_capture_crop' `
    'Capture crop apply must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn revert_capture_crop' `
    'Capture crop revert must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^fn ensure_crop_revision' `
    'Capture crop state and revision validation must remain in the crop transaction module'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_item_preview' `
    'Capture preview implementation must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_crop_source_preview' `
    'Capture crop source preview must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub\(crate\) fn encode_crop' `
    'Capture crop encoding must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn apply_capture_crop' `
    'Capture crop apply must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn revert_capture_crop' `
    'Capture crop revert must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn ensure_crop_revision' `
    'Capture crop validation must not move back into inbox orchestration'
```

- [x] **Step 2: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，指出 `capture_crop.rs` 缺失。

---

### Task 2: 提取完整裁剪事务模块

**Files:**
- Create: `src-tauri/src/modules/capture_crop.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`

**Interfaces:**
- Public re-export:

```rust
pub use capture_crop::{
    ApplyCaptureCrop, CaptureCropApplyReport, CaptureCropRecipe, CaptureItemPreview,
    NormalizedCropRect, RevertCaptureCrop, apply_capture_crop,
    get_capture_crop_source_preview, get_capture_item_preview, revert_capture_crop,
};
pub(crate) use capture_crop::{EncodedCrop, encode_crop};
```

- Consumes from parent: `CaptureBatchDetail`、`CaptureBatchState`、`CaptureInboxError`、`MAX_CAPTURE_BATCH_BYTES`、`MAX_CAPTURE_BATCH_ITEMS`、`query_batch`、`get_capture_batch_detail`、`sanitize_source_name`、`invalidate_active_pairs_for_item`、`touch_batch`、`repack_link_positions`、`delete_asset_row_if_orphan` and capture asset repository re-exports。

- [x] **Step 1: 移动裁剪模型和常量**

把 `CaptureItemPreview`、`NormalizedCropRect`、`CaptureCropRecipe`、`ApplyCaptureCrop`、`RevertCaptureCrop`、`CaptureCropApplyReport` 以及预览/裁剪私有常量移入新模块，派生属性和 serde/specta 字段命名保持原样。

- [x] **Step 2: 移动预览、编码、应用与撤销实现**

原样移动 `get_capture_item_preview`、`get_capture_crop_source_preview`、`CropSource`、`EncodedCrop`、`StagedCropAsset`、`validate_crop_recipe`、`encode_crop`、`apply_capture_crop`、`revert_capture_crop` 和仅由裁剪使用的 `ensure_crop_revision`。

- [x] **Step 3: 建立私有模块和兼容 re-export**

父模块添加：

```rust
#[path = "capture_crop.rs"]
mod capture_crop;
```

并按 Interfaces re-export；删除已无用途的 `HashMap`、`Cursor`、base64 和 `decrypt_asset` import，不修改任何外部调用者。

- [x] **Step 4: 运行采集箱定向测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store --test capture_inbox_command`

Expected: 15 项全部通过。

---

### Task 3: 验证识别与 LAN 兼容路径

**Files:**
- Verify: `src-tauri/src/modules/capture_recognition.rs`
- Verify: `src-tauri/src/modules/capture_lan.rs`
- Verify: `src-tauri/src/infrastructure/recognition_anchor_layout.rs`
- Verify only: `src-tauri/src/infrastructure/recognition_visual_split.rs`

- [x] **Step 1: 运行识别集成测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition`

Expected: 非真实语料测试全部通过；真实语料测试按其既有 ignore 条件处理。

- [x] **Step 2: 运行 LAN 裁剪回归**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib modules::capture_lan::tests`

Expected: LAN 采集单元测试全部通过。

---

### Task 4: 架构与静态门禁

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-capture-crop-transaction-boundary.md`

- [x] **Step 1: 运行边界和格式检查**

Run: `pnpm contract:rust-boundaries`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

- [x] **Step 2: 运行 Clippy**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

- [x] **Step 3: 本地代码复核**

确认裁剪 SQL/文件 staging 没有重复实现，私有模块未扩大外部 API，父模块行数继续下降，命令/bindings/schema 未变，并运行 `git diff --check`。

- [x] **Step 4: 记录验证结果**

记录测试数量、架构契约、格式、Clippy、模块行数、暂存区状态和用户已有识别文件未被触碰。

## Self-Review

- 需求覆盖：完整裁剪事务而非仅纯编码函数被提取；兼容路径和回归均有对应任务。
- Placeholder scan：无 TBD、TODO 或未定义实现步骤。
- 类型一致性：所有 re-export 名称与当前外部 import 完全一致；crate-private `EncodedCrop`/`encode_crop` 保持可见性。

## Verification Record

- 架构 TDD：新增边界规则后先因 `capture_crop.rs` 缺失而失败；完成提取后 `pnpm contract:rust-boundaries` 通过。
- 采集箱：`capture_inbox_command` 2/2、`capture_inbox_store` 13/13，共 15 项通过。
- 状态优先级：新增“Collecting 且 revision 同时过期”回归，确认返回 `InvalidState` 而不是 `RevisionConflict`；定向测试 1/1 通过。
- 识别兼容：`capture_recognition` 23 项通过，2 项真实语料/固定哈希运行时测试按既有配置忽略。
- LAN 兼容：`modules::capture_lan::tests` 10/10 通过。
- 静态门禁：Rust 边界契约、`cargo fmt --check`、全 targets/features `cargo clippy -D warnings` 与 `git diff --check` 均通过。
- 模块体量：`capture_inbox.rs` 1517 行、`capture_inbox_repository.rs` 267 行、`capture_crop.rs` 745 行；本批父模块由 2234 行降至 1517 行，连续两批由原 2502 行降至 1517 行。
- API/数据边界：命令签名、Specta bindings、数据库 schema、依赖及外部 `modules::capture_inbox::*` 调用路径未改。
- 工作区：Git 暂存区为空，未暂存、未提交；用户已有的 `recognition_visual_split.rs` 修改未触碰。
- 范围排除：未处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA。
