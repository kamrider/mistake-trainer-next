# Legacy Mutation Boundaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate legacy import and rollback file/transaction lifecycles from the public legacy facade without changing commands, DTOs, persistence semantics, progress events, or rollback safety.

**Architecture:** Keep `legacy.rs` as the stable public facade for plans, receipts, candidate lifecycle, history listing, errors, and scan re-exports. Add private direct children `legacy_import_transaction.rs` and `legacy_rollback_transaction.rs`; the first owns validation, encryption staging, atomic persistence, sync upserts, progress, and failure cleanup, while the second owns preservation checks, quarantine/restore, atomic deletion, tombstones, delete outbox entries, and rollback receipts.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher, image, SHA-256, serde_json, UUID v7, Vitest source contracts, Cargo integration tests, PowerShell architecture contracts.

## Global Constraints

- Preserve the public `modules::legacy::{import_legacy_plan, rollback_legacy_import}` paths and their exact signatures, callback behavior, DTO shapes, serde/Specta names, error variants/messages, and command bindings.
- Preserve source rescanning/fingerprint checks before and immediately before import commit, completed-import deduplication, image type/dimension/pixel budgets, SHA-256 verification, encryption, and asset deduplication.
- Preserve the invariant that staged assets are promoted before the database commit and every promoted/staged file owned by a failed import is removed.
- Preserve rollback revision/reference guards, imported-versus-user-created review handling, quarantine-before-delete behavior, quarantine restoration on failure, tombstone retention, delete outbox payloads, and account scoping.
- Keep `legacy_scan.rs`, `legacy_import_transaction.rs`, and `legacy_rollback_transaction.rs` private direct children of `legacy.rs`; no caller may import the children directly.
- Keep scan traversal, bounded reads, path validation, and fingerprint construction in `legacy_scan.rs`; import and rollback children may consume only its existing `pub(super)` helpers.
- Do not change migrations, commands, generated bindings, recognition/OCR code, device-migration UX, update recovery, licensing, privacy, support, account deletion, or SLA work.
- Preserve all pre-existing dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Format only `src-tauri/src/modules/legacy.rs`, `src-tauri/src/modules/legacy_import_transaction.rs`, and `src-tauri/src/modules/legacy_rollback_transaction.rs` with direct `rustfmt --edition 2024`; do not run repository-wide Cargo formatting.
- Do not stage or commit.

---

### Task 1: Lock The Two Mutation Boundaries

**Files:**

- Create: `tests/legacy-mutation-boundaries.test.ts`
- Verify: `src-tauri/src/modules/legacy.rs`
- Verify: `src-tauri/tests/legacy_import_store.rs`

**Interfaces:**

- Consumes: current public import/rollback functions and their private helpers in `legacy.rs`.
- Produces: a failing four-case source contract requiring two private children, stable re-exports, isolated file lifecycles, and a DTO/candidate-only facade.

- [x] **Step 1: Record the unchanged 16-test baseline**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: `legacy_command` 2/2, `legacy_import_plan` 3/3, `legacy_import_store` 4/4, and `legacy_scan` 7/7.

- [x] **Step 2: Add a failing four-case source contract**

The contract must assert:

1. `legacy.rs` privately declares both child paths, publicly re-exports both stable functions, and no longer defines either function.
2. `legacy_import_transaction.rs` owns `StagedLegacyAsset`, `import_legacy_plan`, `persist_legacy_import`, sync-upsert/audit helpers, image/hash validation, staging cleanup, both fingerprint checks, asset promotion, transaction commit, and all five progress phases; it contains no rollback quarantine or delete-outbox implementation.
3. `legacy_rollback_transaction.rs` owns `rollback_legacy_import`, `RemovedLegacyEntity`, entity lookup, deletion enqueue, quarantine restore, revision/reference preservation queries, quarantine-before-commit, commit, cleanup, tombstone insertion, and delete outbox creation; it contains no encryption, import staging, or sync-upsert implementation.
4. `legacy.rs` retains all public DTOs, `LegacyImportManager`, `list_legacy_imports`, `LegacyImportError`, and scan re-exports, while containing no filesystem mutation, transaction commit, staging/quarantine structs, or mutation SQL.

Run:

```powershell
npm test -- --run tests/legacy-mutation-boundaries.test.ts
```

Expected: FAIL because neither transaction child exists and both functions remain in the facade.

---

### Task 2: Extract Import And Rollback Lifecycles

**Files:**

- Create: `src-tauri/src/modules/legacy_import_transaction.rs`
- Create: `src-tauri/src/modules/legacy_rollback_transaction.rs`
- Modify: `src-tauri/src/modules/legacy.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**

- `legacy_import_transaction::import_legacy_plan(connection: &mut Connection, blob_root: &Path, key: &[u8; 32], account_id: &str, candidate_id: &str, plan: LegacyImportPlan, now_utc_ms: i64, progress: impl FnMut(LegacyImportProgress)) -> Result<LegacyImportReceipt, LegacyImportError>` remains public only through the private module's parent re-export.
- `legacy_rollback_transaction::rollback_legacy_import(connection: &mut Connection, blob_root: &Path, account_id: &str, import_id: &str, now_utc_ms: i64) -> Result<LegacyRollbackReceipt, LegacyImportError>` remains public only through the private module's parent re-export.
- The import child consumes `legacy_scan::{MAX_ASSET_BYTES, read_bounded, take_chars}`, `build_legacy_import_plan`, `legacy_tree_fingerprint`, and parent plan/progress/receipt/error/rating types.
- The rollback child consumes `legacy_scan::is_safe_relative_path` and parent rollback receipt/error types.

- [x] **Step 1: Move the import lifecycle verbatim**

Move `StagedLegacyAsset`, `import_legacy_plan`, `persist_legacy_import`, `insert_import_sync_operation`, `record_import_entity`, `unique_profile_name`, `validate_import_image`, `plaintext_digest`, and `cleanup_legacy_staging` to `legacy_import_transaction.rs`. Change only the top-level function's module path and imports; preserve statement order and strings.

- [x] **Step 2: Move the rollback lifecycle verbatim**

Move `TOMBSTONE_RETENTION_MILLIS`, `rollback_legacy_import`, `RemovedLegacyEntity`, `enqueue_legacy_rollback_deletion`, `import_entity_ids`, and `restore_quarantined_assets` to `legacy_rollback_transaction.rs`. Change only the top-level function's module path and imports; preserve statement order and strings.

- [x] **Step 3: Reduce the facade to stable public ownership**

Declare both private modules next to `legacy_scan`, add `pub use legacy_import_transaction::import_legacy_plan;` and `pub use legacy_rollback_transaction::rollback_legacy_import;`, and remove imports used only by the mutation children. The facade must retain the public plans, progress/receipt/candidate/summary DTOs, candidate manager, history list query, and error type.

- [x] **Step 4: Add architecture regression protection**

Extend `scripts/rust-boundary-contract.ps1` to require the two public child functions and their named helpers, reject their definitions in `legacy.rs`, require facade manager/list/error ownership, reject `fs::rename`, `transaction.commit`, and `INSERT INTO` in the facade, reject rollback-only tokens in the import child, and reject import-only tokens in the rollback child.

- [x] **Step 5: Format only the three touched Rust modules**

Run:

```powershell
rustfmt --edition 2024 src-tauri\src\modules\legacy.rs src-tauri\src\modules\legacy_import_transaction.rs src-tauri\src\modules\legacy_rollback_transaction.rs
```

- [x] **Step 6: Prove the focused boundary and behavior**

Run:

```powershell
npm test -- --run tests/legacy-mutation-boundaries.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_import_store --test legacy_command
```

Expected: source contract 4/4, import store 4/4, and command lifecycle 2/2.

---

### Task 3: Verify Scan And Architecture Separation

- [x] **Step 1: Run the architecture contract**

```powershell
powershell -ExecutionPolicy Bypass -File scripts\rust-boundary-contract.ps1
```

Expected: `Rust architecture boundary contract passed.`

- [x] **Step 2: Run complete legacy behavior**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: all 16 legacy tests pass with unchanged totals.

---

### Task 4: Run Commercial-Quality Gates And Review

- [x] **Step 1: Run strict Rust gates**

```powershell
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml
```

- [x] **Step 2: Run full frontend/source gates**

```powershell
npm test -- --run
npm run typecheck
npm run lint
```

Expected after the new four-case contract: 126 Vitest files and 694 tests pass.

- [x] **Step 3: Perform local code review**

Review the targeted diff for public API stability, source validation timing, asset ownership on every failure path, file promotion before import commit, account scoping, imported-entity audit completeness, rollback preservation guards, quarantine restoration, deletion/tombstone/outbox ordering, exact payload strings, and accidental scope expansion.

- [x] **Step 4: Record hygiene and evidence**

Record baseline, red, focused, adjacent, full, and review results below. Confirm no staged changes, no generated artifacts, no edits to `recognition_visual_split.rs`, and no excluded launch-readiness work.

## Verification Record

- Baseline: `legacy_command` passed 2/2, `legacy_import_plan` passed 3/3, `legacy_import_store` passed 4/4, and `legacy_scan` passed 7/7 (16 total).
- Red contract: all four new cases failed before extraction because both private children were absent and the facade still owned mutation implementation.
- Focused: the new mutation source contract passed 4/4; import-store behavior passed 4/4; candidate/command behavior passed 2/2.
- Adjacent: the Rust architecture boundary contract passed; all 16 legacy tests passed with unchanged totals.
- Full Rust: strict all-target/all-feature Clippy passed with `-D warnings`; the full Rust test command passed, including 127 library tests discovered (124 passed, 3 environment-dependent tests ignored) and every integration target.
- Full frontend/source: the first Vitest run under simultaneous Rust load had one unrelated `App.test.ts` route-wait timeout; the isolated file immediately passed 6/6 and the no-load full rerun passed 126 files and 694 tests. `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Local review: no findings. Source rebuilding and the final fingerprint check remain before import commit; owned staged/final files are cleaned on every import error. Rollback still guards revisions and user references, quarantines files before database deletion, restores quarantine on staging/finalization errors, commits tombstones and delete outbox entries atomically, then removes quarantine only after commit.
- Module size: `legacy.rs` decreased from 1296 to 283 lines; the private import transaction is 652 lines and the private rollback transaction is 392 lines.
- Hygiene: targeted diff/whitespace checks passed; no files were staged; no generated artifacts entered status; the pre-existing `recognition_visual_split.rs` modification was not touched; excluded launch-readiness work was not changed.
