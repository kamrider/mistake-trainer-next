# Storage Migration Snapshot Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate database snapshot, storage usage, referenced-asset copying, and copied-library integrity checks from the storage migration state-machine orchestrator without changing any public API or migration behavior.

**Architecture:** Keep `storage_migration.rs` as the public type/API owner and lifecycle orchestrator for staging, journal validation, pointer switching, startup reopen, rollback, cleanup, and receipts. Add a private `storage_migration_snapshot.rs` child that owns the database backup, account boundary, asset manifest, bounded copying, decryption verification, and usage calculation; the facade delegates through three `pub(super)` operations.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher backup API, SHA-256, encrypted assets, Vitest source contracts, existing Rust storage migration integration tests

## Global Constraints

- Preserve every public function signature, serialized type shape, error variant/code, journal/owner schema, receipt outcome, destination validation rule, migration ordering, rollback path, and cleanup rule.
- Preserve account isolation, database and asset count/byte budgets, source/destination hash verification, symlink/reparse-point rejection, encrypted asset authentication, plaintext hash/length checks, and exact SQL ordering.
- Keep `stage_storage_migration`, `stage_storage_migration_from_source`, `storage_usage_bytes`, `apply_pending_storage_migration`, pending/receipt functions, journal validation, pointer switching, runtime reopen, owner cleanup, and redacted labels at their existing public paths.
- Declare the snapshot implementation privately inside `storage_migration.rs`; do not modify `modules/mod.rs`, commands, startup, existing Rust integration tests, or any pre-existing dirty file.
- Format only the selected facade and new child; preserve the dirty worktree and do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/storage-migration-snapshot-boundary.test.ts`
- Test: `src-tauri/tests/storage_migration.rs`

**Interfaces:**
- Consumes: the facade and proposed child as source text.
- Produces: an architecture contract that prevents snapshot/integrity code returning to the lifecycle orchestrator.

- [x] **Step 1: Add the failing source contract**

Assert the facade privately declares `storage_migration_snapshot.rs`; retains all public migration APIs; delegates staging, usage, and destination validation; and no longer owns SQLCipher backup, asset query/copy/decrypt, account boundary, or database-budget helpers. Assert the child exposes only `stage_library_snapshot`, `storage_usage_bytes`, and `validate_library_tree` as `pub(super)`, while lifecycle pointer/reopen/receipt/cleanup operations remain absent.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/storage-migration-snapshot-boundary.test.ts`

Expected: FAIL because the private snapshot module and delegations do not exist.

- [x] **Step 3: Run the existing migration characterization suite**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_migration`

Expected: every existing happy-path, rollback, tamper, ownership, restart, and receipt test passes before extraction.

### Task 2: Extract Snapshot And Integrity Ownership

**Files:**
- Create: `src-tauri/src/modules/storage_migration_snapshot.rs`
- Modify: `src-tauri/src/modules/storage_migration.rs`

**Interfaces:**
- Produces: `pub(super) fn stage_library_snapshot(&StorageMigrationSource, &Path, &Path, &str) -> Result<(usize, u64), StorageMigrationError>`.
- Produces: `pub(super) fn storage_usage_bytes(&StorageMigrationSource) -> Result<(u64, u64), StorageMigrationError>`.
- Produces: `pub(super) fn validate_library_tree(&Path, &str, &[u8; 32], &str, Option<&Path>, Option<&str>) -> Result<(), StorageMigrationError>`.
- Consumes: facade-owned source/error types, safety budgets/path helpers, owner validation, relative-file collection, and file hashing.

- [x] **Step 1: Create the private snapshot repository**

Move `AssetRecord`, the current usage implementation, `create_database_snapshot`, `query_assets`, `copy_referenced_assets`, `copy_and_verify`, `validate_library_tree`, `validate_account_boundary`, `ensure_database_budget`, and `pragma_u64` into the child. Add `stage_library_snapshot` to own the current lock/validate/snapshot/query/copy/revalidate sequence and return the asset count and copied bytes.

- [x] **Step 2: Reduce the facade to lifecycle delegation**

Add:

```rust
#[path = "storage_migration_snapshot.rs"]
mod snapshot;
```

Replace the inline staging block with `snapshot::stage_library_snapshot`, replace the public usage body with `snapshot::storage_usage_bytes`, and call `snapshot::validate_library_tree` during startup application. Remove child-only imports and definitions while preserving all lifecycle code.

- [x] **Step 3: Format only the two target Rust files**

Run: `rustfmt --edition 2024 src-tauri/src/modules/storage_migration.rs src-tauri/src/modules/storage_migration_snapshot.rs`

Expected: no collateral formatting writes.

- [x] **Step 4: Run focused structure and migration behavior tests**

Run: `npm test -- --run tests/storage-migration-snapshot-boundary.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_migration`

Expected: the boundary contract and complete storage migration characterization suite pass unchanged.

### Task 3: Adjacent Compatibility And Full Regression

**Files:**
- Modify only Task 1 or Task 2 files if verification exposes a regression.

**Interfaces:**
- Consumes: startup/runtime/command contracts, complete Rust crate, and frontend architecture suite.
- Produces: commercial quality-gate evidence for the extraction.

- [x] **Step 1: Run adjacent storage/startup contracts**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test runtime_state --test backup_restore_startup --test command_contract`

Expected: runtime reopen, startup restore ordering, and command serialization remain compatible.

- [x] **Step 2: Run strict Rust lint and complete Rust tests**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml`

Expected: zero Rust warnings and every non-ignored test passes.

- [x] **Step 3: Run frontend contracts and static quality gates**

Run: `npm test -- --run`

Run: `npm run typecheck`

Run: `npm run lint`

Expected: all source/interaction contracts, Vue types, and zero-warning lint pass.

### Task 4: Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-storage-migration-snapshot-boundary.md`

**Interfaces:**
- Consumes: target source/diffs, validation output, file status, and review findings.
- Produces: checked implementation plan and exact evidence without staging or committing.

- [x] **Step 1: Perform final code review**

Review public API identity, lifecycle ordering, source lock scope, account isolation, SQL/asset ordering, byte/count limits, hash and decrypt verification, path safety, visibility, rollback ownership, receipt behavior, source-contract robustness, and overlap with existing dirty files. Fix every Critical or Important finding.

- [x] **Step 2: Verify patch hygiene and scope**

Run targeted trailing-whitespace checks and `git diff --check`, then `git diff --cached --name-only`.

Expected: target files are clean, the staged index is empty, and only the previously clean facade plus three new files belong to this batch.

- [x] **Step 3: Record evidence without committing**

Check every completed step and append red/green totals, focused/adjacent/full results, file-line reduction, preserved security invariants, review verdict, hygiene, index, and exact scope. Do not stage or commit.

## Verification Record

- Red phase: the new architecture contract failed 2/2 before extraction because the private snapshot module and delegations did not exist.
- Characterization baseline: `storage_migration` passed 11/11 before extraction, including copy failure atomicity, account-boundary rejection, junction/reparse rejection, destination tamper rollback, pointer commit ordering, receipt handling, and custom-storage restart behavior.
- Focused green phase: the source contract passed 2/2 and `storage_migration` passed 11/11 after extraction.
- Adjacent compatibility: `backup_restore_startup` passed 7/7, `command_contract` 8/8, and `runtime_state` 5/5 (20/20 total).
- Rust quality gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited 0. Three environment-dependent OCR runtime/corpus probes remained explicitly ignored.
- Frontend quality gates: Vitest passed 110/110 files and 646/646 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- File shape: `storage_migration.rs` reduced from 1,183 to 835 lines. The 390-line private snapshot repository now has one owner for database backup, usage calculation, account-boundary queries, referenced-asset copying, copy verification, and decrypted destination validation.
- Preserved invariants: public APIs, serialized types, error codes, journal/pointer/receipt ordering, source connection lock scope, database and asset limits, ordered asset SQL, duplicate/path/reparse rejection, source/destination hashes, encrypted-asset authentication, plaintext length/hash checks, rollback ownership, and redacted labels remain unchanged.
- Review verdict: no Critical or Important findings. The source contract was strengthened to require exactly three `pub(super)` snapshot operations and to keep lifecycle mutation dependencies out of the child.
- Hygiene and scope: target trailing-whitespace and `git diff --check` checks passed; the staged index is empty. Only the previously clean facade plus the new snapshot repository, architecture contract, and this plan belong to this batch. Existing dirty/untracked files were preserved; nothing was staged or committed.
