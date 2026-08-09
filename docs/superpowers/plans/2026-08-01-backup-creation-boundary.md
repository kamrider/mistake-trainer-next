# Backup Creation Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move backup creation out of the oversized backup facade and give each unpublished package one explicit filesystem owner that cleans only its own temporary directory and never replaces an existing completed package.

**Architecture:** Add a private `backup_creation.rs` child that owns the complete database-snapshot, asset-copy, manifest, and package-publication use case. `backup.rs` remains the stable public facade and retains package validation and restore state transitions; a non-cloneable `StagedBackupPackage` provides RAII cleanup and a guarded final publication transition.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher online backup, encrypted assets, RAII filesystem ownership, tempfile unit tests, Vitest source contracts.

## Global Constraints

- Preserve `create_backup`'s public signature, `BackupSummary`, all `BackupError` variants and command error mapping, backup format version 1, supported schema versions 1–17, manifest JSON fields, UUIDv7-derived labels, SQLCipher snapshot behavior, immutable asset ordering, hash algorithms, and every size/count budget.
- Keep validation, restore-candidate preparation, pending restore, startup swap, rollback, receipt handling, and their public APIs in `backup.rs`.
- Reuse `backup_package_repository.rs` for contained paths, reparse checks, copy/hash, manifest-file metadata, normalization, hashing, and durable writes; reuse `backup_schema_validation.rs` for database budget and single-account policy.
- `StagedBackupPackage` must not implement `Clone`. Dropping an unpublished owner removes only its UUID-derived temporary directory. Publication must reject an existing final package before rename and must never remove or overwrite that final package.
- The successful rename is the final fallible filesystem transition. Once it succeeds, dropping the owner must retain the completed package.
- Do not add dependencies or change commands, bindings, schema, migrations, UI, restore/device-migration behavior, or excluded licensing, privacy/legal, support, account deletion, update recovery, and SLA work.
- Do not stage, commit, revert, or modify unrelated dirty files, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Format only `src-tauri/src/modules/backup.rs` and `src-tauri/src/modules/backup_creation.rs` with direct Rustfmt.

---

### Task 1: Baseline And Failing Architecture Contract

**Files:**
- Create: `tests/backup-creation-boundary.test.ts`
- Verify: `src-tauri/tests/backup_store.rs`
- Verify: `src-tauri/tests/backup_restore_startup.rs`

**Interfaces:**
- Consumes: the current inline `backup.rs::create_backup` implementation and its explicit tail cleanup.
- Produces: a red source contract for the facade/child boundary and staged-package lifecycle.

- [x] **Step 1: Record the unchanged backup baseline**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store --test backup_restore_startup
```

Expected: `backup_store` passes 20/20 and `backup_restore_startup` passes 7/7 before production changes.

- [x] **Step 2: Add the failing source contract**

Create a Vitest file that reads `backup.rs` and `backup_creation.rs`, then asserts:

```ts
expect(facade).toMatch(/#\[path = "backup_creation\.rs"\]\r?\nmod backup_creation;/)
expect(facade).toContain('pub use backup_creation::create_backup;')
expect(facade).not.toMatch(/\bpub fn create_backup\(/)
expect(facade).not.toContain('Backup::new')
expect(creation).toContain('pub fn create_backup(')
expect(creation).toContain('struct StagedBackupPackage')
expect(creation).toContain('impl Drop for StagedBackupPackage')
expect(creation).toContain('fn publish(&mut self)')
expect(creation).toContain('match fs::symlink_metadata(&self.final_path)')
expect(creation).toContain('fs::rename(&self.temporary_path, &self.final_path)?')
expect(creation).toContain('self.published = true')
```

Also require the three lifecycle unit-test names from Task 2, reject `Clone` on the owner, reject validation/restore entry points from the child, and reject the old creation-specific `if result.is_err() && temporary.parent() == Some(destination.as_path())` cleanup from the facade without forbidding restore-candidate cleanup.

- [x] **Step 3: Run the source contract red**

Run:

```powershell
npm test -- --run tests/backup-creation-boundary.test.ts
```

Expected: FAIL because the child module and staged-package owner do not exist and `create_backup` remains inline in the facade.

### Task 2: Private Creation Use Case And Linear Package Owner

**Files:**
- Create: `src-tauri/src/modules/backup_creation.rs`
- Modify: `src-tauri/src/modules/backup.rs`

**Interfaces:**
- Parent produces the unchanged public facade:

```rust
#[path = "backup_creation.rs"]
mod backup_creation;

pub use backup_creation::create_backup;
```

- Child produces the unchanged public use case and private ownership transition:

```rust
pub fn create_backup(
    connection: &Mutex<Connection>,
    blob_root: &Path,
    database_key: &str,
    account_id: &str,
    destination: &Path,
    now_utc_ms: i64,
) -> Result<BackupSummary, BackupError>;

struct StagedBackupPackage {
    temporary_path: PathBuf,
    final_path: PathBuf,
    published: bool,
}

impl StagedBackupPackage {
    fn create(destination: &Path, label: &str) -> Result<Self, BackupError>;
    fn path(&self) -> &Path;
    fn publish(&mut self) -> Result<(), BackupError>;
}
```

- [x] **Step 1: Add lifecycle unit tests**

Add unit tests in `backup_creation.rs` named:

```rust
dropping_unpublished_backup_removes_only_its_temporary_directory
publishing_rejects_and_preserves_a_preexisting_completed_package
publishing_keeps_the_completed_backup_package
```

The first creates a staged owner plus an unrelated sentinel, drops it, and asserts only the owner temporary directory disappeared. The second precreates the matching final directory with sentinel bytes, asserts publication returns `BackupError::Io` with `AlreadyExists`, drops the owner, and asserts the completed sentinel remains byte-for-byte. The third writes a file into the staged directory, publishes, drops the owner, and asserts the final file remains.

- [x] **Step 2: Implement the staged package owner**

Implement:

```rust
#[derive(Debug)]
struct StagedBackupPackage {
    temporary_path: PathBuf,
    final_path: PathBuf,
    published: bool,
}

impl StagedBackupPackage {
    fn create(destination: &Path, label: &str) -> Result<Self, BackupError> {
        let temporary_path = destination.join(format!(".{label}.tmp"));
        let final_path = destination.join(label);
        fs::create_dir(&temporary_path)?;
        Ok(Self { temporary_path, final_path, published: false })
    }

    fn path(&self) -> &Path {
        &self.temporary_path
    }

    fn publish(&mut self) -> Result<(), BackupError> {
        match fs::symlink_metadata(&self.final_path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "backup package already exists",
                ).into());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::rename(&self.temporary_path, &self.final_path)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagedBackupPackage {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.temporary_path);
        }
    }
}
```

- [x] **Step 3: Move the complete creation use case**

Move `create_backup` and `map_target_database_error` from `backup.rs` to the child without changing SQL, ordering, limits, hashes, summary construction, or error mapping. Replace `temporary` joins with `package.path()` and replace direct rename plus explicit result cleanup with:

```rust
let mut package = StagedBackupPackage::create(&destination, &label)?;
// Existing snapshot, asset-copy, and manifest logic uses package.path().
package.publish()?;
```

In `backup.rs`, declare the private child, publicly re-export `create_backup`, and remove imports used only by creation: `Mutex`, `Duration`, `rusqlite::backup::Backup`, `DatabaseError`, and `open_encrypted_database`.

- [x] **Step 4: Format only the two target Rust files**

Run:

```powershell
rustfmt --edition 2024 src-tauri/src/modules/backup.rs src-tauri/src/modules/backup_creation.rs
```

Expected: only those two Rust files are formatted.

- [x] **Step 5: Run focused green tests**

Run:

```powershell
npm test -- --run tests/backup-creation-boundary.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib backup_creation::tests
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store --test backup_restore_startup
```

Expected: source contract passes 3/3, lifecycle tests pass 3/3, and both integration targets retain their baseline totals.

### Task 3: Architecture, Adjacent, And Full Regression Gates

**Files:**
- Verify: `src-tauri/src/modules/backup.rs`
- Verify: `src-tauri/src/modules/backup_creation.rs`
- Verify: `src-tauri/src/commands/backup.rs`
- Verify: `src-tauri/tests/storage_migration.rs`

**Interfaces:**
- Consumes: the facade-compatible creation extraction.
- Produces: evidence that backup commands, storage migration, restore startup, and the complete product remain compatible.

- [x] **Step 1: Run existing architecture and adjacent gates**

Run `npm run contract:rust-boundaries`, the backup command target if present, and the storage-migration integration target. Expected: all discovered tests pass.

- [x] **Step 2: Run strict Rust gates**

Run all-target/all-feature Clippy with `-D warnings`, then the complete Rust suite. Expected: exit code 0 for both; only already documented environment-dependent tests may remain ignored.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, TypeScript checking, and ESLint with zero warnings. Expected: exit code 0 for each.

### Task 4: Lifecycle Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-backup-creation-boundary.md`

**Interfaces:**
- Consumes: final diff and verification output.
- Produces: an auditable record of the new ownership boundary and preserved behavior.

- [x] **Step 1: Review every creation transition**

Review invalid destination, application-root destination, temporary-directory creation, lock poisoning, schema/account/budget rejection, snapshot open/copy/quick-check failure, asset source validation, partial asset copy, total budget overflow, manifest write, pre-existing final package, rename failure, successful publication, and summary construction. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Check target trailing whitespace/final newlines, run global `git diff --check`, confirm the staged index remains empty, confirm `backup_store` test counts are unchanged, and confirm only the new child, facade wiring, source contract, and this plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check all steps and replace the pending record below with baseline/red/green totals, full-suite totals, module line counts before/after, fixed scenarios, preserved invariants, review verdict, environment warnings, and hygiene results.

## Self-Review

- Spec coverage: the plan covers a stable public facade, complete creation extraction, exclusive temporary-directory ownership, existing-final preservation, failure cleanup, successful retention, behavior regression, architecture gates, full gates, and workspace preservation.
- Placeholder scan: no TBD, TODO, “implement later,” undefined error handling, or unspecified test request remains.
- Type consistency: `backup.rs` re-exports the child's unchanged public function; the child consumes parent DTO/error/manifest/constants and the two existing repositories; `StagedBackupPackage` is private and mutable only for publication.

## Verification Record

- Baseline: before production changes, `backup_store` passed 20/20 and `backup_restore_startup` passed 7/7.
- Red evidence: the new source contract failed 0/3 because the private creation child, staged-package owner, guarded publication, and facade delegation did not yet exist.
- Green evidence: the final source contract passed 3/3; direct `StagedBackupPackage` lifecycle tests passed 3/3; `backup_store` retained 20/20 and `backup_restore_startup` retained 7/7.
- Adjacent evidence: `storage_migration` passed 11/11 through the unchanged public `create_backup` facade; the existing Rust architecture boundary contract passed.
- Strict Rust evidence: all-target/all-feature Clippy with `-D warnings` passed. The final complete Rust suite discovered 125 library tests (122 passed, 3 environment-dependent tests ignored), and every integration target passed.
- Frontend/static evidence: final complete Vitest passed 122 files and 681 tests; TypeScript checking passed; ESLint passed with zero warnings. One earlier unrelated `App.test.ts` navigation wait timed out at 680/681; its focused target passed 6/6 and both later complete runs passed 681/681.
- Architecture result: `backup.rs` is now the stable facade plus validation/restore coordinator; `backup_creation.rs` exclusively owns SQLCipher snapshot creation, immutable asset copying, manifest construction, and package publication. Existing package and schema repositories remain reused rather than duplicated.
- Ownership result: every post-staging early return drops one non-cloneable owner and removes only its UUID-derived temporary directory. Publication rejects every existing final filesystem object, treats only an explicit `NotFound` as publishable, propagates all other metadata errors, and retains the package after a successful rename.
- Review result: local review under the `requesting-code-review` workflow found one Important fail-closed issue where `Path::exists()` hid metadata errors. It was replaced with `symlink_metadata`, and all affected/full gates were rerun. No Critical or Important finding remains.
- Lifecycle coverage: invalid/application-owned destination, staging collision, lock poison, schema/account/budget rejection, database open/snapshot/quick-check failure, asset path/reparse/source/copy failure, total budget overflow, manifest failure, final-package collision, metadata error, rename failure, successful publication, and summary construction were reviewed.
- Preserved invariants: public function and command signatures, DTO/error types, error mapping, backup format and JSON names, schema 1–17 compatibility, UUIDv7 label shape, deterministic asset ordering, hashes, byte/count budgets, validation, restore state transitions, commands, bindings, migrations, and UI remain unchanged.
- Module sizes: `backup.rs` decreased from 919 to 740 lines; the focused `backup_creation.rs` is 308 lines; the source contract is 104 lines.
- Hygiene: all batch files have final newlines and zero trailing whitespace; global `git diff --check` exits 0; the staged index is empty. The unrelated dirty `recognition_visual_split.rs` and all excluded pre-launch areas were not modified, staged, or reverted.
- Environment notes: existing OpenSSL PDB `LNK4099` and SQLCipher `VirtualLock LastError=1453` warnings remain noisy on this Windows host, but all relevant commands exited 0.
