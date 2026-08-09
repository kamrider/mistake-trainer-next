# Sync Pull Asset Staging Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent failed cloud pulls from deleting pre-existing blob files by tracking whether each final file was actually promoted by the current page and centralizing the staging lifecycle.

**Architecture:** Keep `sync_pull.rs` as the public pull loop, transport/download validation, encryption, decoded-change application, database transaction, conflict merge, schedule rebuild, cursor update, and committed tombstone cleanup owner. Add a private `sync_pull_asset_staging.rs` child that owns `StagedAsset`, deterministic local paths, encrypted staging writes, guarded promotion, explicit `moved_to_final` ownership, and success/rollback cleanup; rollback removes a final blob only when this page successfully moved it there.

**Tech Stack:** Rust 2024, Tokio cloud transport, rusqlite transactions, encrypted asset blobs, tempfile unit tests, Vitest source contracts, Rust integration tests

## Global Constraints

- Preserve `SyncPullError`, stable error codes, `PullReport`, `pull_until_current`, page size 500, asset size/dimension/media limits, public signatures, cursor semantics, report counters, and all cloud protocol behavior.
- Preserve remote account/page validation, download media/hash/length/image validation, encryption, deterministic `blobs/<shard>/<uuid>.mtb` path, page staging path, merge/conflict behavior, schedule rebuild, transaction scope, cursor update, and post-commit tombstone deletion.
- Add explicit `moved_to_final: bool`, initialized false and set true only immediately after a successful staging-to-final rename.
- On failed page application, remove staged temporary files and only final files whose `moved_to_final` is true; never use `final_path.exists()` as proof that the current page owns a final file.
- On successful commit, remove staging files/directories while retaining every promoted final file.
- If a target final file already exists before promotion, return `AssetMismatch`, roll back the database page, retain the pre-existing bytes exactly, and leave the pull cursor unchanged.
- Keep the child synchronous and free of cloud transport, download validation, image decoding, encryption, database access, transactions, conflict merging, schedule rebuilds, cursor updates, and async functions.
- Format only the selected facade and new child. Preserve the dirty worktree; do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Data-Loss Regression And Structural Contract

**Files:**
- Modify: `src-tauri/tests/sync_pull.rs`
- Create: `tests/sync-pull-asset-staging-ownership.test.ts`
- Test: `src-tauri/src/modules/sync_pull.rs`

**Interfaces:**
- Consumes: existing `pull_until_current`, deterministic asset path, `SyncPullError::AssetMismatch`, transaction rollback, and pull cursor behavior.
- Produces: an integration regression proving pre-existing bytes survive failure and a source contract fixing the target ownership boundary.

- [x] **Step 1: Add the pre-existing-final-file regression**

Precreate the remote asset's deterministic final path with sentinel bytes, execute the normal four-change page, assert `cloud_asset_mismatch`, assert no asset row/cursor advance, and assert the sentinel bytes still exist unchanged.

- [x] **Step 2: Add the failing source contract**

Assert the private staging child and facade delegation, `moved_to_final` ownership semantics, guarded cleanup, child unit-test names, mutable staged-asset application, and continued facade ownership of transport/security/database work.

- [x] **Step 3: Run both red tests**

Run `cargo test --test sync_pull failed_pull_does_not_delete_a_preexisting_unowned_blob` through the MSVC wrapper and `npm test -- --run tests/sync-pull-asset-staging-ownership.test.ts`.

Expected: the Rust test fails because the sentinel final file is deleted; the source contract fails because the child boundary does not exist.

- [x] **Step 4: Record the unchanged existing baseline**

Record the already-run `sync_pull` target result of 8/8 before adding the regression.

### Task 2: Extract Ownership-Aware Asset Staging

**Files:**
- Create: `src-tauri/src/modules/sync_pull_asset_staging.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`

**Interfaces:**
- Produces: `stage_encrypted_asset(blob_root: &Path, asset: &WireAsset, page_id: &str, encrypted: &[u8]) -> Result<StagedAsset, SyncPullError>`, `StagedAsset::{asset,relative_path,promote}`, and `cleanup_page(staged: &[StagedAsset], rollback_final: bool)` with parent-only visibility.
- Consumes: `WireAsset`, `SyncPullError::{AssetMismatch,Blob}`, deterministic blob/staging path conventions, and encrypted bytes produced by the facade.

- [x] **Step 1: Move local staging paths and writes into the child**

Create `StagedAsset` with private paths plus `moved_to_final: false`. Move staging-root creation and encrypted temporary-file writing into `stage_encrypted_asset`, preserving path formats and I/O error mapping.

- [x] **Step 2: Implement guarded promotion and ownership-aware cleanup**

Implement `promote(&mut self)` to create the shard directory, reject an existing target, rename the staged file, then set `moved_to_final = true`. Implement cleanup so rollback deletes a final path only when that flag is true; success cleanup never deletes final files.

- [x] **Step 3: Add child lifecycle unit tests**

Prove rollback preserves a pre-existing unowned target byte-for-byte, rollback removes a file promoted by this page, and success cleanup keeps a promoted final while removing the page staging directory.

- [x] **Step 4: Delegate mutable asset application from the facade**

Replace inline staging construction with `stage_encrypted_asset`, pass `&mut [StagedAsset]` into `apply_page`, build a mutable ID map, pass `Option<&mut StagedAsset>` to `upsert_asset`, call `promote` before the asset insert, and replace both cleanup branches with `cleanup_page`.

- [x] **Step 5: Format only target Rust files and run focused green tests**

Run direct `rustfmt --edition 2024` for the facade and child. Rerun the source contract, the new pre-existing-file regression, the child lifecycle tests through the filtered library target, and the complete `sync_pull` integration target.

### Task 3: Adjacent And Full Regression

**Interfaces:**
- Consumes: final ownership-aware staging implementation and unchanged public pull API.
- Produces: evidence that sync store/push, database schema, and the complete product remain compatible.

- [x] **Step 1: Run adjacent sync and database contracts**

Run `sync_store`, `sync_push`, and `database_schema` integration targets and record exact totals.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, followed by the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, `vue-tsc --noEmit`, and ESLint with zero warnings.

### Task 4: File-Ownership Review, Hygiene, And Record

**Interfaces:**
- Consumes: final diff and all verification output.
- Produces: reviewed ownership semantics and exact evidence in this plan.

- [x] **Step 1: Review every file-state transition**

Compare pre/post behavior for download failure, staging-write failure, pre-existing target, successful rename, SQL failure after rename, transaction commit failure, successful commit, tombstone deletion, retry path, staging-root cleanup, cursor/report counters, and error codes. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target trailing-whitespace/final-newline checks and global `git diff --check`; confirm the staged index remains empty and only the facade, child, integration regression, source contract, and plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check completed steps and replace the pending verification record with red/green totals, regression commands, line counts, fixed data-loss scenario, preserved invariants, review verdict, and hygiene results.

## Verification Record

- Existing baseline before this batch: `sync_pull` 8/8 passed.
- Red evidence: `failed_pull_does_not_delete_a_preexisting_unowned_blob` failed 0/1 because the old rollback deleted the sentinel final file (`NotFound`); the initial structural contract passed 2/3 and failed on the missing staging child.
- Focused green evidence: staging source contract 3/3, staging child unit tests 3/3, and complete `sync_pull` integration target 10/10 passed.
- Fixed data-loss path: a pre-existing deterministic final blob is no longer treated as page-owned and survives `AssetMismatch` byte-for-byte with no asset row or cursor advance.
- Fixed partial-page leak found during review: if a later asset download/staging step fails, every asset staged earlier by that page is now cleaned; the new integration regression proves the page staging directory, final blob, and database rows remain absent.
- Adjacent targets: `sync_store` 6/6, `sync_push` 7/7, and `database_schema` 15/15 passed.
- Strict Rust gates: all-target Clippy with `-D warnings` passed; the full library suite passed 115 tests with 3 ignored (118 discovered), and every integration target passed.
- Frontend/static gates: complete Vitest passed 120 files / 675 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Boundary result: `sync_pull.rs` retains transport, validation, encryption, transaction, merge, cursor, and tombstone responsibilities; the synchronous child exclusively owns deterministic local staging paths, writes, guarded promotion, and ownership-aware cleanup.
- Preserved invariants: public APIs, stable error codes, page size, limits, cursor/report behavior, transaction scope, conflict/schedule behavior, deterministic blob paths, and committed tombstone deletion are unchanged.
- Review verdict: after fixing the partial-page staging leak, no Critical or Important findings remain across download, write, pre-existing target, rename, SQL/commit failure, success, tombstone, retry, cleanup, cursor, report, and error-code transitions.
- File sizes: facade 932 lines; staging child 155; integration target 1068; source contract 111; plan 134 lines.
- Hygiene: all five batch files have a final newline and no trailing whitespace; global `git diff --check` passed (only existing line-ending conversion warnings); the staged index is empty. Only the facade, child, integration regression, source contract, and this plan belong to the batch, and unrelated dirty files were left untouched.
