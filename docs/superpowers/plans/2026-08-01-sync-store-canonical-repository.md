# Sync Store Canonical Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate canonical sync-entity reconstruction from push lease orchestration so the SQLite repository boundary is explicit while every public sync API and wire payload remains unchanged.

**Architecture:** Keep `sync_store.rs` as the application-facing facade for remote binding, leasing, acknowledgement, retry, cursor, and transfer cleanup. Move operation-row selection plus profile/asset/problem/review/export/tombstone reconstruction into a private `sync_store_canonical.rs` child module; it consumes the facade's wire DTOs and error type through `super`, and exports only two `pub(super)` repository operations.

**Tech Stack:** Rust 2024, rusqlite, serde, Vitest source contract, existing Rust integration tests

## Global Constraints

- Do not change public function signatures, wire structs/enums, serialization names, SQL predicates/order, transaction scope, lease semantics, retry timing, asset transfer metadata, or error mapping.
- Do not modify `src-tauri/src/modules/mod.rs`; declare the repository with `#[path = "sync_store_canonical.rs"] mod canonical;` inside `sync_store.rs`.
- Do not modify dirty sync pull/push/conflict files, application ports, capture, backup, recognition, migrations, Cargo metadata, or `recognition_visual_split.rs`.
- The child module may expose only `OperationRow`, `select_due_operations`, and `canonical_entity` as `pub(super)`; all entity-specific loaders remain private.
- Preserve the dirty worktree; do not stage or commit.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.

---

### Task 1: Structural Contract And Baseline

**Files:**
- Create: `tests/sync-store-canonical-boundary.test.ts`
- Test: `src-tauri/tests/sync_store.rs`

**Interfaces:**
- Consumes: `src-tauri/src/modules/sync_store.rs` and `src-tauri/src/modules/sync_store_canonical.rs` as source text.
- Produces: an architecture contract proving the facade delegates canonical reconstruction to the private repository.

- [x] **Step 1: Add the failing source contract**

Assert the facade contains `#[path = "sync_store_canonical.rs"]` and calls `canonical::select_due_operations` / `canonical::canonical_entity`. Assert entity loader definitions are absent from the facade. Assert the child file declares the two repository functions as `pub(super)` and keeps `load_profile`, `load_asset`, `load_problem`, `load_review_event`, `load_export_snapshot`, and `load_tombstone` private.

- [x] **Step 2: Run the structure test and verify red**

Run: `npm test -- --run tests/sync-store-canonical-boundary.test.ts`

Expected: FAIL because the child repository file and delegation do not exist.

- [x] **Step 3: Run the existing Rust characterization suite**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_store`

Expected: the existing suite passes before the refactor, proving the starting sync behavior.

### Task 2: Extract Canonical Entity Repository

**Files:**
- Create: `src-tauri/src/modules/sync_store_canonical.rs`
- Modify: `src-tauri/src/modules/sync_store.rs`

**Interfaces:**
- Produces: `pub(super) fn select_due_operations(...) -> Result<Vec<OperationRow>, SyncStoreError>`.
- Produces: `pub(super) fn canonical_entity(...) -> Result<(WireEntity, Option<PendingAssetTransfer>), SyncStoreError>`.
- Consumes: parent DTOs `WireProfile`, `WireAsset`, `WireProblemAsset`, `WireProblemAggregate`, `WireReviewEvent`, `WireExportSnapshot`, `WireTombstone`, `WireEntity`, `PendingAssetTransfer`, and `SyncStoreError`.

- [x] **Step 1: Create the child repository**

Move `OperationRow`, `select_due_operations`, `canonical_entity`, and all six `load_*` functions verbatim into `sync_store_canonical.rs`. Import `rusqlite::{OptionalExtension, Transaction, params}` and the parent DTOs/error/`validate_uuid`. Mark the row fields, row type, and two facade-called functions `pub(super)`; keep loaders private.

- [x] **Step 2: Reduce the facade to orchestration**

Add:

```rust
#[path = "sync_store_canonical.rs"]
mod canonical;
```

Delete the moved definitions from `sync_store.rs`. In `lease_push_batch`, replace direct calls with:

```rust
let rows = canonical::select_due_operations(&transaction, account_id, now_utc_ms, limit)?;
let (payload, transfer) =
    canonical::canonical_entity(&transaction, account_id, remote_user_id, row)?;
```

- [x] **Step 3: Format and run focused structural/behavior tests**

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml`

Run: `npm test -- --run tests/sync-store-canonical-boundary.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_store`

Expected: formatting exits 0 and both test suites pass with unchanged behavior.

### Task 3: Regression And Architecture Verification

**Files:**
- Modify only Task 1 or Task 2 files if checks reveal a regression.

**Interfaces:**
- Consumes: the complete Rust crate and frontend architecture-contract suite.
- Produces: compile, lint, behavior, and boundary evidence.

- [x] **Step 1: Run adjacent sync integration tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull`

Expected: pull application remains compatible with the unchanged store facade.

- [x] **Step 2: Run Rust lint and complete tests**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml`

Expected: all targets compile without warnings and every Rust test passes.

- [x] **Step 3: Run frontend structural/full tests**

Run: `npm test -- --run tests/sync-store-canonical-boundary.test.ts`

Run: `npm test -- --run`

Expected: the architecture contract and full frontend suite pass.

### Task 4: Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-sync-store-canonical-repository.md`

**Interfaces:**
- Consumes: diffs, test output, lint output, status, and final review.
- Produces: checked steps and exact verification evidence.

- [x] **Step 1: Perform final code review**

Review facade/repository responsibility, visibility, SQL identity, transaction lifetime, ordering, boundary validation, wire payload equality, error propagation, source-contract robustness, and overlap with existing user changes. Fix every Critical or Important finding.

- [x] **Step 2: Verify patch hygiene and scope**

Run: `git diff --check -- src-tauri/src/modules/sync_store.rs src-tauri/src/modules/sync_store_canonical.rs tests/sync-store-canonical-boundary.test.ts docs/superpowers/plans/2026-08-01-sync-store-canonical-repository.md`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm this task touched no pre-existing dirty Rust file except the previously clean `sync_store.rs` selected for the refactor.

- [x] **Step 3: Record evidence without committing**

Check every completed step and append exact red/green test totals, Rust lint/full totals, frontend totals, file/line reduction, public API/SQL invariants, review verdict, hygiene, index, and scope evidence. Do not stage or commit.

## Verification Evidence

- Red contract: `npm test -- --run tests/sync-store-canonical-boundary.test.ts` failed as expected with 1 file / 2 tests failing because delegation and the child repository did not yet exist.
- Characterization baseline: `sync_store` passed 6/6 before extraction.
- Focused green: formatting exited 0; the architecture contract passed 1 file / 2 tests; `sync_store` passed 6/6 after extraction.
- Adjacent behavior: `sync_pull` passed 8/8.
- Rust static/full verification: Clippy passed all targets with `-D warnings`; full `cargo test` exited 0. The library target reported 110 passed / 3 ignored, and test enumeration found 382 tests / 0 benchmarks across all targets.
- Frontend verification: the final full Vitest run passed 107/107 files and 640/640 tests. `vue-tsc --noEmit` and ESLint with `--max-warnings 0` also exited 0.
- Size: `sync_store.rs` fell from 712 to 408 lines. The private canonical repository is 315 lines; combined code is 723 lines, with the small increase limited to imports and explicit boundary visibility.
- Invariants: all public sync-store signatures and wire DTO definitions remain in the facade. SQL text, predicates, dependency ordering, transaction lifetime, retry behavior, canonical payload construction, transfer metadata, boundary validation, and error propagation were moved without semantic changes.
- Review: no Critical or Important production-code findings. The review tightened the source contract so it proves `mod canonical;` is private as well as path-bound; that contract passed 2/2 after the change.
- Hygiene and scope: targeted `git diff --check` exited 0; no trailing whitespace was found in new files; `git diff --cached --name-only` was empty. Only the previously clean `sync_store.rs` plus the new repository, contract, and this plan were changed for this task. A timestamp audit found no collateral Rust formatting writes.
- Index: nothing was staged or committed. Existing unrelated modified and untracked files were preserved.
