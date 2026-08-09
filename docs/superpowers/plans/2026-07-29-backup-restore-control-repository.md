# Backup Restore Control Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate backup-restore control-file persistence and owned-directory validation from the oversized backup orchestration module without changing backup packages, restore behavior, or public APIs.

**Architecture:** Add `backup_restore_repository.rs` as a private child module of `backup.rs`. It owns private restore marker/metadata models, canonical UUID-derived directory names, bounded atomic JSON control-file I/O, exact-file removal, and direct-child directory validation; `backup.rs` remains the use-case facade and owns package/database validation and restore state transitions.

**Tech Stack:** Rust 2024, serde/serde_json, tempfile, UUID v7, PowerShell architecture contracts, Cargo integration tests.

## Global Constraints

- Preserve all existing public Rust functions, Specta bindings, error variants/codes, file names, JSON field names, size limits, and restore state transitions.
- Do not change backup format version, supported database schema versions, migrations, database validation, OCR/recognition code, device-migration UX, or any excluded pre-launch work.
- Preserve every existing dirty-worktree change, especially schema-16/17 recognition-pair backup validation.
- Keep the new repository private to `backup.rs`; do not widen command or crate API visibility.
- Do not stage or commit.

---

### Task 1: Lock the restore-control architecture boundary

**Files:**

- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**

- Requires `backup_restore_repository.rs` to own `read_pending_marker`, `write_control_file`, `ensure_owned_directory_if_present`, and `restore_directory_name`.
- Rejects definitions of those functions in `backup.rs`.

- [x] Add `Require-Pattern` checks for all four repository entry points.
- [x] Add `Reject-Pattern` checks that prevent restore control I/O and path ownership checks returning to backup orchestration.
- [x] Run `.\scripts\rust-boundary-contract.ps1`; expect failure because the repository module does not exist.

### Task 2: Add the private repository and direct tests

**Files:**

- Create: `src-tauri/src/modules/backup_restore_repository.rs`
- Modify: `src-tauri/src/modules/backup.rs`

**Interfaces:**

```rust
pub(super) struct RestoreCandidateMetadata {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) prepared_at_utc_ms: i64,
    pub(super) manifest_sha256: String,
}

pub(super) struct PendingRestoreMarker {
    pub(super) candidate_id: String,
    pub(super) rollback_id: String,
    pub(super) label: String,
    pub(super) scheduled_at_utc_ms: i64,
}

pub(super) fn read_pending_marker(
    application_root: &Path,
) -> Result<PendingRestoreMarker, BackupError>;

pub(super) fn write_control_file<T: Serialize>(
    application_root: &Path,
    file_name: &str,
    value: &T,
    replace: bool,
) -> Result<(), BackupError>;

pub(super) fn restore_directory_name(candidate_id: &str) -> Result<String, BackupError>;

pub(super) fn rollback_directory_name(rollback_id: &str) -> Result<String, BackupError>;

pub(super) fn ensure_owned_directory_if_present(
    root: &Path,
    path: &Path,
) -> Result<(), BackupError>;
```

- [x] Move restore metadata/marker models, restore-control constants, UUID-derived name validation, bounded control-file writes, marker reads, exact-file removal, and direct-child directory checks into the repository.
- [x] Add direct unit tests that reject non-canonical UUID directory names, preserve `RestorePending` on a second non-replacing write, reject a directory passed to exact-file removal, and accept a valid marker round trip.
- [x] Import the repository types/constants/functions into `backup.rs` and remove only the moved definitions.
- [x] Keep `BackupRestoreReceipt`, `RestoreSwap`, all public use cases, package copying, database checks, and state-transition decisions in `backup.rs`.
- [x] Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib backup_restore_repository::tests
```

Expected: four repository unit tests pass.

### Task 3: Verify integration behavior and boundaries

**Files:**

- Verify: `src-tauri/tests/backup_store.rs`
- Verify: `scripts/rust-boundary-contract.ps1`

- [x] Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: all backup-store integration tests pass with unchanged assertions.

- [x] Run `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup` to cover pending markers, startup swaps, commit/rollback, and one-shot receipt reads.
- [x] Run `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check`.
- [x] Run `.\scripts\cargo-msvc.cmd check --manifest-path src-tauri\Cargo.toml`.
- [x] Run `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --lib --tests -- -D warnings`.
- [x] Run `.\scripts\rust-boundary-contract.ps1`.
- [x] Run frontend typechecking, linting, the complete Vitest suite, and the production build because the repository shares the release gate.
- [x] Review the scoped diff for changed JSON names, file names, size limits, error mapping, visibility, restore transitions, schema-validation overlap, or unrelated edits.
- [x] Confirm `backup.rs` line count decreases materially and `git diff --check` passes.
