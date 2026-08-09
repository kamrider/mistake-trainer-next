# Backup Restore Lifecycle Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the complete backup-restore lifecycle from the backup facade into one private child without changing public APIs, package safety, startup recovery, or user-visible behavior.

**Architecture:** Add `backup_restore.rs` as the private restore use-case child of `backup.rs`. It owns candidate preparation and validation, scheduling, crash-recoverable directory swaps, commit/rollback, receipt handling, and verified package copying; `backup_restore_repository.rs` remains the persistence/path repository, while `backup.rs` retains shared public models, errors, limits, and stable re-exports.

**Tech Stack:** Rust 2024, serde/serde_json, UUID v7, Vitest source contracts, Cargo unit/integration tests, PowerShell architecture contracts.

## Global Constraints

- Preserve the exact public names, signatures, visibility, Specta/serde shapes, error variants/messages, backup format, database schema support, file names, JSON field names, candidate TTL, size budgets, and restore state-transition table.
- Keep `backup_restore.rs` and `backup_restore_repository.rs` private direct children of `backup.rs`; callers must continue importing restore APIs through `modules::backup`.
- Keep control-file I/O, UUID-derived names, exact-file removal, and owned-directory checks in `backup_restore_repository.rs`; do not duplicate repository responsibilities in lifecycle orchestration.
- Preserve creation and package-validation child boundaries; do not move creation, SQLCipher validation, schema validation, or generic package persistence into restore lifecycle code.
- Do not change recognition/OCR code, migrations, device-migration UX, update recovery, licensing, privacy, support, account deletion, or SLA work.
- Preserve all pre-existing dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Format only `src-tauri/src/modules/backup.rs` and `src-tauri/src/modules/backup_restore.rs` with direct `rustfmt --edition 2024`; do not run repository-wide Cargo formatting.
- Do not stage or commit.

---

### Task 1: Lock The Restore Lifecycle Boundary

**Files:**

- Create: `tests/backup-restore-boundary.test.ts`
- Modify: `scripts/rust-boundary-contract.ps1`
- Verify: `tests/backup-creation-boundary.test.ts`
- Verify: `tests/backup-validation-boundary.test.ts`
- Verify: `src-tauri/src/modules/backup.rs`

**Interfaces:**

- Consumes: the current public restore functions and `RestoreSwap` implementation in `backup.rs`.
- Produces: a failing source contract requiring one private lifecycle child and stable facade re-exports.

- [x] **Step 1: Record the unchanged baseline**

Run:

```powershell
npm test -- --run tests/backup-creation-boundary.test.ts tests/backup-validation-boundary.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup
```

Expected: source contracts pass 6/6, `backup_store` passes 20/20, and `backup_restore_startup` passes 7/7.

- [x] **Step 2: Add the failing restore source contract**

Create `tests/backup-restore-boundary.test.ts` with three tests:

```ts
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/backup.rs')
const lifecyclePath = resolve('src-tauri/src/modules/backup_restore.rs')
const repositoryPath = resolve('src-tauri/src/modules/backup_restore_repository.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('backup restore lifecycle boundary', () => {
  it('keeps stable restore APIs on the facade through one private child', () => {
    const facade = readSource(facadePath)
    expect(facade).toMatch(/#\[path = "backup_restore\.rs"\]\r?\nmod backup_restore;/)
    expect(facade).toContain('pub use backup_restore::{')
    expect(existsSync(lifecyclePath)).toBe(true)
    for (const name of [
      'prepare_backup_restore', 'validate_restore_candidate',
      'schedule_backup_restore', 'begin_pending_restore',
      'record_failed_restore', 'take_restore_receipt', 'RestoreSwap',
    ]) {
      expect(facade).toContain(name)
    }
  })

  it('moves every restore transition and verified candidate copy out of the facade', () => {
    expect(existsSync(lifecyclePath)).toBe(true)
    if (!existsSync(lifecyclePath)) return
    const facade = readSource(facadePath)
    const lifecycle = readSource(lifecyclePath)
    for (const token of [
      'pub struct RestoreSwap', 'pub fn prepare_backup_restore(',
      'pub fn validate_restore_candidate(', 'pub fn schedule_backup_restore(',
      'pub fn begin_pending_restore(', 'fn rollback_interrupted_restore(',
      'impl RestoreSwap', 'pub fn record_failed_restore(',
      'pub fn take_restore_receipt(', 'fn copy_verified_manifest_entry(',
    ]) {
      expect(lifecycle).toContain(token)
      expect(facade).not.toContain(token)
    }
  })

  it('keeps persistence in the repository and unrelated backup work outside restore', () => {
    expect(existsSync(lifecyclePath)).toBe(true)
    if (!existsSync(lifecyclePath)) return
    const lifecycle = readSource(lifecyclePath)
    const repository = readSource(repositoryPath)
    for (const token of [
      'read_pending_marker', 'write_control_file',
      'ensure_owned_directory_if_present', 'restore_directory_name',
    ]) {
      expect(repository).toContain(`fn ${token}`)
      expect(lifecycle).not.toMatch(new RegExp(`(?:pub(?:\\(super\\))? )?fn ${token}\\(`))
    }
    expect(lifecycle).not.toContain('pub fn create_backup(')
    expect(lifecycle).not.toContain('pub fn validate_backup(')
    expect(lifecycle).not.toContain('open_encrypted_database_read_only')
  })
})
```

- [x] **Step 3: Run the contract red**

Run:

```powershell
npm test -- --run tests/backup-restore-boundary.test.ts
```

Expected: 0/3 because `backup_restore.rs`, its module declaration, and facade re-exports do not exist.

- [x] **Step 4: Extend the production architecture contract**

Require `backup_restore.rs` to own `RestoreSwap` and all six public restore functions; reject their definitions in `backup.rs`; reject restore-repository function definitions in `backup_restore.rs`. Run the contract after extraction and expect it to pass.

### Task 2: Extract Restore Lifecycle Orchestration

**Files:**

- Create: `src-tauri/src/modules/backup_restore.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `tests/backup-creation-boundary.test.ts`
- Modify: `tests/backup-validation-boundary.test.ts`
- Modify: `scripts/rust-boundary-contract.ps1`
- Test: `tests/backup-restore-boundary.test.ts`

**Interfaces:**

- Consumes: shared `BackupError`, `BackupManifest`, `BackupRestoreCandidate`, `BackupRestoreReceipt`, `BackupSummary`, `ManifestFile`, constants, `validate_backup`, package-repository helpers, and restore-repository helpers.
- Produces unchanged public items re-exported by `backup.rs`:

```rust
pub struct RestoreSwap;

pub fn prepare_backup_restore(
    source: &Path,
    application_root: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupRestoreCandidate, BackupError>;

pub fn validate_restore_candidate(...) -> Result<BackupSummary, BackupError>;
pub fn schedule_backup_restore(...) -> Result<BackupSummary, BackupError>;
pub fn begin_pending_restore(...) -> Result<Option<RestoreSwap>, BackupError>;
pub fn record_failed_restore(...) -> Result<BackupRestoreReceipt, BackupError>;
pub fn take_restore_receipt(...) -> Result<Option<BackupRestoreReceipt>, BackupError>;
```

- [x] **Step 1: Create the private lifecycle child**

Move `RestoreSwap`, all six public restore functions, `validate_candidate_directory`, `rollback_interrupted_restore`, the `RestoreSwap` implementation, and `copy_verified_manifest_entry` verbatim into `backup_restore.rs`. Import only `std::{fs, path::{Path, PathBuf}}`, `uuid::Uuid`, the required shared parent types/constants/function, and helpers from both repositories.

- [x] **Step 2: Convert the facade to stable re-exports**

Add:

```rust
#[path = "backup_restore.rs"]
mod backup_restore;

pub use backup_restore::{
    RestoreSwap, begin_pending_restore, prepare_backup_restore, record_failed_restore,
    schedule_backup_restore, take_restore_receipt, validate_restore_candidate,
};
```

Remove restore-only imports and inline bodies from `backup.rs`. Retain all public data models, `BackupError`, manifest models, format/schema/file/size constants, and `RESTORE_CANDIDATE_TTL_MS` in the facade for existing children.

- [x] **Step 3: Evolve the creation and validation source contracts**

Change their restore-boundary assertions to require each restore public name in the facade re-export and lifecycle child, reject inline definitions from the facade, and continue rejecting restore definitions from `backup_creation.rs` and `backup_validation.rs`.

- [x] **Step 4: Format only scoped Rust files**

Run:

```powershell
rustfmt --edition 2024 src-tauri/src/modules/backup.rs src-tauri/src/modules/backup_restore.rs
```

Expected: exit code 0 with no unrelated file changes.

- [x] **Step 5: Run focused green tests**

Run:

```powershell
npm test -- --run tests/backup-restore-boundary.test.ts tests/backup-creation-boundary.test.ts tests/backup-validation-boundary.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup
```

Expected: all nine source-contract tests pass, `backup_store` retains 20/20, and `backup_restore_startup` retains 7/7.

### Task 3: Adjacent And Full Regression

**Files:**

- Verify: `src-tauri/src/modules/backup.rs`
- Verify: `src-tauri/src/modules/backup_restore.rs`
- Verify: `src-tauri/src/commands/backup.rs`
- Verify: `src-tauri/tests/storage_migration.rs`

**Interfaces:**

- Consumes: facade-compatible restore re-exports.
- Produces: evidence that commands, storage migration, startup recovery, and the complete application retain the same behavior.

- [x] **Step 1: Run architecture and adjacent targets**

Run `npm run contract:rust-boundaries` and the `storage_migration` Rust integration target. Expected: architecture passes and storage migration retains 11/11.

- [x] **Step 2: Run strict Rust gates**

Run all-target/all-feature Clippy with `-D warnings`, then the complete Rust suite. Expected: exit code 0 for both, with only the three documented environment-dependent recognition tests ignored in the library target.

- [x] **Step 3: Run frontend/static gates serially**

Run complete Vitest, TypeScript checking, and ESLint with zero warnings. Expected: exit code 0 for each.

### Task 4: Review, Hygiene, And Evidence

**Files:**

- Modify: `docs/superpowers/plans/2026-08-01-backup-restore-lifecycle-boundary.md`

**Interfaces:**

- Consumes: final scoped sources, diff, and gate outputs.
- Produces: an auditable completion record.

- [x] **Step 1: Review every restore state transition**

Compare the extracted code with the pre-extraction facade for candidate preparation cleanup, metadata and manifest binding, expiry, label integrity, pending-marker exclusivity, all six live/stage/rollback directory states, interrupted rollback, swap commit, swap rollback, failure receipts, one-shot receipt reads, verified copy budgets, path containment, and error mapping. Fix every Critical or Important issue.

- [x] **Step 2: Verify scope and workspace hygiene**

Check scoped line counts, final newlines, trailing whitespace, global `git diff --check`, staged-index emptiness, source-contract compatibility, and that the pre-existing `recognition_visual_split.rs` modification remains untouched.

- [x] **Step 3: Record exact evidence**

Check every plan step and replace the pending record with baseline/red/green totals, adjacent/full totals, before/after line counts, preserved invariants, review findings, environmental warnings, and hygiene results.

## Self-Review

- Spec coverage: stable public APIs, private lifecycle/repository separation, complete transition extraction, prior contract evolution, adjacent consumers, full gates, and dirty-worktree protection all have explicit tasks.
- Placeholder scan: no TBD, TODO, “implement later,” undefined error handling, or unspecified test request remains.
- Type consistency: facade re-exports preserve all six public function signatures and `RestoreSwap`; child imports the existing private shared types/constants and repository helpers without widening visibility.

## Verification Record

- Baseline: the creation and validation source contracts passed 6/6; `backup_store` passed 20/20; `backup_restore_startup` passed 7/7 before extraction.
- Red: the new restore-lifecycle source contract failed 0/3 because `backup_restore.rs`, its module declaration, and facade re-exports did not exist.
- Focused green: creation, validation, and restore source contracts passed 9/9 together; `backup_store` retained 20/20; `backup_restore_startup` retained 7/7.
- Adjacent regression: the extended Rust architecture boundary contract passed and `storage_migration` retained 11/11, including custom-storage restore controls.
- Strict/full Rust: all-target/all-feature Clippy passed with `-D warnings`; the complete Rust suite exited 0. The library target ran 127 tests: 124 passed and the three documented environment-dependent recognition probes were ignored. Every integration target passed.
- Frontend/static: complete Vitest passed 124 files and 687 tests; `vue-tsc --noEmit` passed; ESLint passed with zero warnings.
- Architecture result: `backup.rs` fell from 557 to 111 lines. The complete restore lifecycle now lives in the 461-line private direct child `backup_restore.rs`, protected by an 84-line source contract and the production Rust boundary contract. All public restore names and `RestoreSwap` remain facade re-exports.
- Preserved invariants: candidate preparation still validates before and after bounded copying, binds metadata to the manifest hash and label, cleans failed temporary candidates, rejects invalid/expired IDs, enforces one pending marker, handles the unchanged live/stage/rollback state table, and preserves commit, rollback, failed-validation, and one-shot receipt semantics. Generic package I/O and restore control persistence remain in their repositories.
- Lifecycle/security review: candidate containment, reparse rejection, copy size/hash budgets, metadata/manifest binding, label integrity, all directory-state transitions, interrupted rollback, swap commit/rollback, receipt validation, and error mapping were compared with the pre-extraction implementation. No Critical or Important issue was found.
- Environment notes: existing OpenSSL PDB `LNK4099` and SQLCipher `VirtualLock` LastError 1453 warnings remained non-fatal; the complete Rust run finished successfully within the expanded timeout.
- Hygiene: all seven scoped files have final newlines and zero trailing-whitespace matches; global `git diff --check` exited 0 apart from existing LF/CRLF notices; the staged index is empty. The pre-existing dirty `recognition_visual_split.rs` and all unrelated workspace changes were left untouched.
