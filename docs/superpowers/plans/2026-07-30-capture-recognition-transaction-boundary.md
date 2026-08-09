# Capture Recognition Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将识别结果应用、撤销、操作账本、加密文件 staging 和恢复清理从识别公开门面提取为独立私有事务模块，同时保持原子性、故障注入和旧账本回滚兼容。

**Architecture:** 新建 `capture_recognition_transaction.rs` 作为 `capture_recognition` 的私有子模块，完整拥有从建议选择、源文件解密、区域编码、加密 staging 到数据库提交、账本记录、撤销校验和孤儿资源清理的事务边界。父模块继续拥有公开 DTO/错误类型、任务生命周期 re-export、快照哈希和共享区域校验，并 re-export 三个现有事务入口。

**Tech Stack:** Rust 2024、rusqlite/SQLCipher、AES-GCM encrypted assets、image crop encoding、serde/serde_json、UUID v7、PowerShell 架构契约、Cargo integration tests。

## Global Constraints

- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`、识别 Worker 或任何 OCR/版面识别算法。
- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 保持 Tauri 命令、Specta bindings、数据库 schema、识别建议 JSON、操作账本 JSON、资源加密格式和公开错误码不变。
- `BeforeStaging`、`AfterStaging`、`InTransaction` 三处故障注入必须继续保证无数据库、明文、staging 或 final 文件残留。
- 保持旧账本中的 `created_drafts` 兼容字段和撤销校验，不能仅支持新视觉切分操作。
- 不新增依赖，不暂存、不提交；保留工作区全部既有修改。
- 先让架构契约失败，再移动生产代码；不得在父子模块复制事务实现。

---

### Task 1: 锁定识别事务所有权

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Requires: `src-tauri/src/modules/capture_recognition_transaction.rs`
- Enforces: 应用、撤销、最新账本读取、撤销校验、stale 标记和 staging 清理只由事务模块定义。

- [x] **Step 1: 添加失败架构契约**

```powershell
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^pub fn apply_capture_recognition' `
    'Recognition apply must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^pub fn revert_capture_recognition' `
    'Recognition revert must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^pub fn latest_capture_recognition_operation' `
    'Recognition operation reads must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^fn validate_recognition_revert_state' `
    'Recognition revert validation must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^fn mark_recognition_suggestions_stale' `
    'Recognition stale persistence must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^fn cleanup_staged_recognition_assets' `
    'Recognition staged asset cleanup must remain in the recognition transaction module'

$transactionFunctions = @(
    'apply_capture_recognition',
    'revert_capture_recognition',
    'latest_capture_recognition_operation',
    'validate_recognition_revert_state',
    'mark_recognition_suggestions_stale',
    'cleanup_staged_recognition_assets'
)
foreach ($function in $transactionFunctions) {
    Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Recognition transaction function $function must not move back into the facade"
}
```

- [x] **Step 2: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，指出 `capture_recognition_transaction.rs` 缺失。

- [x] **Step 3: 添加事务开启失败的资源清理回归**

```rust
#[test]
fn transaction_begin_failure_cleans_staged_recognition_assets() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "begin-failure");
    let (job_id, suggestion_id) =
        accepted_job(&library, &mut connection, &batch_id, &item_id);
    let revision = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get::<_, u32>(0),
        )
        .unwrap();
    let before_blob_count = recursive_file_count(&library.directory.path().join("blobs"));
    let before_asset_count = connection
        .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get::<_, i64>(0))
        .unwrap();

    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let error = apply_capture_recognition(
        &mut connection,
        ApplyCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id,
            job_id,
            expected_revision: revision,
            accepted_suggestion_ids: vec![suggestion_id],
            blob_root: library.directory.path().to_owned(),
            asset_key: ASSET_KEY,
            now_utc_ms: 30,
            failure_point: None,
        },
    )
    .expect_err("nested transaction must fail");
    connection.execute_batch("ROLLBACK").unwrap();

    assert!(matches!(error, CaptureRecognitionError::Database(_)));
    assert_eq!(
        recursive_file_count(&library.directory.path().join(".staging")),
        0
    );
    assert_eq!(
        recursive_file_count(&library.directory.path().join("blobs")),
        before_blob_count
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        before_asset_count
    );
}
```

- [x] **Step 4: 运行回归确认现有缺口**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition transaction_begin_failure_cleans_staged_recognition_assets`

Expected: FAIL，`.staging` 中残留识别临时文件。

---

### Task 2: 提取完整应用与撤销事务

**Files:**
- Create: `src-tauri/src/modules/capture_recognition_transaction.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`

**Interfaces:**
- Consumes from parent:

```rust
ApplyCaptureRecognition
CaptureRecognitionApplyReport
CaptureRecognitionError
CaptureRecognitionFailurePoint
CaptureRecognitionOperationSummary
CaptureRecognitionRegionProposal
CaptureRecognitionRevertReport
CaptureRecognitionRole
RevertCaptureRecognition
MAX_JOB_ITEMS
capture_item_snapshot_hash
validate_regions
```

- Consumes from capture inbox and assets: `CaptureCropRecipe`、`EncodedCrop`、`MAX_CAPTURE_BATCH_BYTES`、`encode_crop`、`get_capture_batch_detail`、加密/解密和 blob repository helpers。
- Public compatibility:

```rust
pub use capture_recognition_transaction::{
    apply_capture_recognition, latest_capture_recognition_operation,
    revert_capture_recognition,
};
```

- [x] **Step 1: 移动事务专属类型和完整连续实现**

从 `RecognitionSource` 开始，原样移动以下私有类型：

```rust
RecognitionSource
SelectedSuggestion
PreparedRegion
StagedRecognitionAsset
RecognitionOperationLedger
RecognitionLedgerSource
RecognitionLedgerItem
RecognitionLedgerDraft
```

以及 `apply_capture_recognition`、`revert_capture_recognition`、`latest_capture_recognition_operation`、`validate_recognition_revert_state`、`mark_recognition_suggestions_stale` 和 `cleanup_staged_recognition_assets`。移动边界在父模块 `capture_item_snapshot_hash` 之前结束。

- [x] **Step 2: 保持原子应用和文件补偿顺序**

确认实现仍按下列顺序执行：

```text
validate input/state/revision
-> decrypt and encode all selected regions
-> capacity check
-> stage encrypted assets
-> open one SQLite transaction with explicit staging cleanup if BEGIN fails
-> revalidate revision/snapshots inside the transaction
-> insert asset rows and rename staged files inside the persist closure
-> persist items/derivations/pairs/ledger/job/batch in the same transaction
-> cleanup staged and final files on every transaction or pre-commit error
-> load updated batch detail
```

不得拆分 SQLite transaction，也不得删除 `cleanup_staged_recognition_assets(..., true)` 的错误补偿。事务创建必须使用显式 `match`，在 `Err` 分支调用 `cleanup_staged_recognition_assets(&staged_assets, false)` 后再返回数据库错误。

- [x] **Step 3: 保持撤销账本兼容**

保留 `created_drafts` 的反序列化、状态校验和删除循环，即使新视觉切分操作写入空数组；继续在事务提交后才删除孤儿加密 blob。

- [x] **Step 4: 建立私有模块和兼容 re-export**

父模块添加：

```rust
#[path = "capture_recognition_transaction.rs"]
mod capture_recognition_transaction;
```

并按 Interfaces re-export。父模块删除仅由事务区使用的 collections/path、UUID、资产加密和裁剪 repository imports，不修改调用者。

- [x] **Step 5: 运行识别集成测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition`

Expected: 23 项通过、2 项真实语料测试按既有配置忽略。

---

### Task 3: 事务故障与静态门禁

**Files:**
- Verify: `src-tauri/tests/capture_recognition.rs`
- Verify: `src-tauri/src/commands/capture_recognition.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: 三处故障注入、手工编辑后拒绝撤销、stale 批量回滚和旧账本恢复。

- [x] **Step 1: 运行故障注入与事务开启失败定向测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition failures_before_staging_after_staging_and_inside_transaction_leave_no_partial_apply`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition transaction_begin_failure_cleans_staged_recognition_assets`

Expected: 两个定向测试均通过。

- [x] **Step 2: 运行架构、格式和 Clippy**

Run: `pnpm contract:rust-boundaries`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

Expected: 均通过。

- [x] **Step 3: 本地代码复核**

确认事务 SQL、账本和 staging 清理没有在父模块重复；共享快照哈希和区域校验仍在父模块；子模块保持私有；命令、Worker、bindings、schema、依赖和 OCR 文件未因本批修改；运行 `git diff --check` 并确认暂存区为空。

- [x] **Step 4: 记录验证结果**

记录识别测试、故障注入、架构契约、格式、Clippy、模块行数、暂存区状态和范围排除。

## Self-Review

- 需求覆盖：应用、撤销、账本读取、补偿清理和旧账本校验全部位于同一边界，没有拆散原子性；事务 BEGIN 失败窗口有独立回归和修复步骤。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊验证步骤。
- 类型一致性：公开输入、报告和错误类型继续由父模块定义，函数签名原样 re-export；子模块只消费共享快照与区域规则。

## Verification Record

- 架构契约红灯：子模块尚未创建时，`pnpm contract:rust-boundaries` 按预期因缺少事务边界失败。
- 资源清理红灯：首次运行 `transaction_begin_failure_cleans_staged_recognition_assets` 时，SQLite 嵌套事务开启失败后 `.staging` 残留 1 个临时文件。
- 修复：事务创建改为显式 `match`；`BEGIN` 失败时调用 `cleanup_staged_recognition_assets(&staged_assets, false)` 后返回数据库错误。
- 定向回归：事务开启失败清理测试通过；原有 `BeforeStaging`、`AfterStaging`、`InTransaction` 三处故障注入测试通过。
- 完整识别集成测试：26 项中 24 项通过，2 项真实图片语料/OCR 运行时测试按既有配置忽略。
- 静态门禁：Rust 架构边界契约、`cargo fmt --check`、全目标全特性 Clippy（`-D warnings`）均通过。
- 最终模块规模：父 facade 415 行，job lifecycle 子模块 623 行，transaction 子模块 1025 行。
- 边界复核：事务 SQL、账本类型和 staging 清理只在 transaction 子模块；快照哈希和区域校验仍在父模块；子模块私有，公开函数由父模块兼容 re-export。
- 工作区复核：`git diff --check` 通过，暂存区为空。本批未修改命令、Worker、bindings、schema、依赖或 `recognition_visual_split.rs`。
- Windows 环境仍输出既有 OpenSSL 静态 PDB 与 SQLCipher `VirtualLock` 警告，不影响本轮命令退出码。
