# Backup Package Validation Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move complete backup-package verification out of the backup facade and give its private database-copy workspace explicit RAII ownership without changing accepted packages, restore behavior, or public APIs.

**Architecture:** Add a private `backup_validation.rs` child that owns manifest validation, a bounded immutable database copy, SQLCipher/schema/account checks, encrypted-asset authentication, and summary construction. `backup.rs` publicly re-exports `validate_backup` and retains restore preparation/state transitions; a non-cloneable `ValidationWorkspace` deletes only its UUID-derived direct child on every return path.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher, AES-GCM encrypted assets, SHA-256 integrity checks, RAII filesystem ownership, tempfile unit tests, Vitest source contracts.

## Global Constraints

- Preserve `validate_backup`'s public signature, `BackupSummary`, all `BackupError` variants and command error mapping, backup format version 1, schema versions 1–17, manifest JSON names, account hashing, package labels, `ready_for_restore`, SQLite journal/quick/foreign-key checks, asset ordering, plaintext length/hash authentication, and every size/count budget.
- Keep backup creation in `backup_creation.rs`; keep restore-candidate preparation, scheduling, startup swap, rollback, receipt handling, and their public functions in `backup.rs`.
- Reuse `backup_package_repository.rs` for path containment, reparse checks, bounded reads/copies, file hashes, sidecar rejection, labels, and normalization. Reuse `backup_schema_validation.rs` for single-account and versioned schema policy.
- `ValidationWorkspace` must not implement `Clone`; it stores both its canonical parent and UUID-derived direct-child path. Drop removes the workspace only when the path remains a direct child of that parent, and never removes the parent or unrelated entries.
- Do not add dependencies or change commands, bindings, schemas, migrations, UI, restore/device-migration behavior, or excluded licensing, privacy/legal, support, account deletion, update recovery, and SLA work.
- Do not stage, commit, revert, or modify unrelated dirty files, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Format only `src-tauri/src/modules/backup.rs` and `src-tauri/src/modules/backup_validation.rs` with direct Rustfmt.

---

### Task 1: Baseline And Failing Validation Contract

**Files:**
- Create: `tests/backup-validation-boundary.test.ts`
- Verify: `src-tauri/tests/backup_store.rs`
- Verify: `tests/backup-creation-boundary.test.ts`

**Interfaces:**
- Consumes: the current inline `backup.rs::validate_backup` and its explicit temporary-directory cleanup.
- Produces: a red source contract for facade delegation, validation ownership, and responsibility separation.

- [x] **Step 1: Record the unchanged baseline**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
npm test -- --run tests/backup-creation-boundary.test.ts
```

Expected: `backup_store` passes 20/20 and the preceding creation boundary passes 3/3.

- [x] **Step 2: Add the failing source contract**

Create `tests/backup-validation-boundary.test.ts` with three tests that assert:

```ts
expect(facade).toMatch(/#\[path = "backup_validation\.rs"\]\r?\nmod backup_validation;/)
expect(facade).toContain('pub use backup_validation::validate_backup;')
expect(facade).not.toMatch(/\bpub fn validate_backup\(/)
expect(validation).toContain('pub fn validate_backup(')
expect(validation).toContain('struct ValidationWorkspace')
expect(validation).toContain('impl Drop for ValidationWorkspace')
expect(validation).toContain('workspace.path().join(DATABASE_FILE)')
```

Also require both lifecycle unit-test names from Task 2; require manifest, SQLCipher, schema/account, and encrypted-asset verification in the child; reject backup creation and restore transition functions from the child; and reject validation-only imports/cleanup from the facade.

- [x] **Step 3: Run the contract red**

Run:

```powershell
npm test -- --run tests/backup-validation-boundary.test.ts
```

Expected: FAIL 0/3 because the validation child and workspace owner do not exist and the facade still owns validation.

### Task 2: Private Validation Use Case And Workspace Owner

**Files:**
- Create: `src-tauri/src/modules/backup_validation.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `tests/backup-creation-boundary.test.ts`

**Interfaces:**
- Parent produces:

```rust
#[path = "backup_validation.rs"]
mod backup_validation;

pub use backup_validation::validate_backup;
```

- Child produces:

```rust
pub fn validate_backup(
    source: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
) -> Result<BackupSummary, BackupError>;

struct ValidationWorkspace {
    parent: PathBuf,
    path: PathBuf,
}

impl ValidationWorkspace {
    fn create(parent: PathBuf) -> Result<Self, BackupError>;
    fn path(&self) -> &Path;
}
```

- [x] **Step 1: Add direct workspace tests**

Add tests named:

```rust
dropping_validation_workspace_removes_only_its_owned_directory
validation_workspaces_use_distinct_private_names
```

The first creates an unrelated sentinel beside the workspace, writes a nested workspace file, drops the owner, and asserts only the workspace disappeared. The second creates two owners under one canonical temporary parent and asserts both direct-child names start with `.mistake-trainer-validate-`, end with `.tmp`, and differ.

- [x] **Step 2: Implement the owner**

Implement:

```rust
#[derive(Debug)]
struct ValidationWorkspace {
    parent: PathBuf,
    path: PathBuf,
}

impl ValidationWorkspace {
    fn create(parent: PathBuf) -> Result<Self, BackupError> {
        let path = parent.join(format!(
            ".mistake-trainer-validate-{}.tmp",
            Uuid::now_v7().simple()
        ));
        fs::create_dir(&path)?;
        Ok(Self { parent, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ValidationWorkspace {
    fn drop(&mut self) {
        if self.path.parent() == Some(self.parent.as_path()) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
```

- [x] **Step 3: Move complete package validation**

Move `validate_backup` without changing checks, SQL, ordering, errors, or summary fields. Replace `validation_directory` creation, closure, and explicit cleanup with:

```rust
let validation_parent = std::env::temp_dir()
    .canonicalize()
    .map_err(|_| BackupError::InvalidPackage)?;
let workspace = ValidationWorkspace::create(validation_parent)?;
let staged_database = workspace.path().join(DATABASE_FILE);
// Existing validation body returns BackupSummary directly; Drop performs cleanup.
```

Remove validation-only facade imports: `HashMap`, `HashSet`, `Component`, `decrypt_asset`, `plaintext_sha256`, `open_encrypted_database_read_only`, `normalize_relative`, `read_verified_manifest_file`, `reject_sqlite_sidecars`, `safe_label`, and `ensure_single_account`.

- [x] **Step 4: Evolve the preceding creation contract**

Replace its expectation that `backup.rs` directly defines `validate_backup` with:

```ts
expect(facade).toContain('pub use backup_validation::validate_backup;')
expect(creation).not.toContain('pub fn validate_backup(')
```

Keep all restore functions required directly in the facade and all creation-child exclusions unchanged.

- [x] **Step 5: Format and run focused green tests**

Run:

```powershell
rustfmt --edition 2024 src-tauri/src/modules/backup.rs src-tauri/src/modules/backup_validation.rs
npm test -- --run tests/backup-validation-boundary.test.ts tests/backup-creation-boundary.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib backup_validation::tests
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: both source contracts pass 3/3, workspace tests pass 2/2, and `backup_store` retains 20/20.

### Task 3: Adjacent And Full Regression

**Files:**
- Verify: `src-tauri/src/modules/backup.rs`
- Verify: `src-tauri/src/modules/backup_validation.rs`
- Verify: `src-tauri/tests/backup_restore_startup.rs`
- Verify: `src-tauri/tests/storage_migration.rs`

**Interfaces:**
- Consumes: the facade-compatible validation extraction.
- Produces: proof that restore preparation/startup, migration, commands, and the complete product still consume the same API and behavior.

- [x] **Step 1: Run architecture and adjacent targets**

Run the Rust boundary contract, `backup_restore_startup`, and `storage_migration`. Expected: architecture passes; integration targets retain 7/7 and 11/11.

- [x] **Step 2: Run strict Rust gates**

Run all-target/all-feature Clippy with `-D warnings`, then the complete Rust suite. Expected: exit code 0 for both; only the three documented environment-dependent recognition tests remain ignored.

- [x] **Step 3: Run frontend/static gates serially**

Run complete Vitest, TypeScript checking, and ESLint with zero warnings. Expected: exit code 0 for each.

### Task 4: Security Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-backup-package-validation-boundary.md`

**Interfaces:**
- Consumes: final diff and verification output.
- Produces: an auditable verification record.

- [x] **Step 1: Review every validation transition**

Review source canonicalization, manifest path/size/JSON/version/account/count checks, SQLite sidecars, temporary-parent failure, workspace collision, partial database copy, ciphertext hash mismatch, wrong key, journal/quick/foreign-key/schema/account checks, asset count/set equality, duplicate canonical asset, AES-GCM failure, plaintext length/hash mismatch, aggregate budget overflow, success cleanup, and restore callers. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Check final newlines/trailing whitespace, global `git diff --check`, staged-index emptiness, module line counts, source-contract compatibility, and that unrelated `recognition_visual_split.rs` remains untouched.

- [x] **Step 3: Record exact evidence**

Check all steps and replace the pending record with baseline/red/green totals, adjacent/full totals, before/after line counts, preserved invariants, review findings, environment warnings, and hygiene results.

## Self-Review

- Spec coverage: stable API, complete verification extraction, direct-child RAII cleanup, validation security checks, prior contract evolution, adjacent consumers, full gates, and dirty-worktree protection all have explicit tasks.
- Placeholder scan: no TBD, TODO, “implement later,” undefined error handling, or unspecified test request remains.
- Type consistency: the facade re-exports the child's unchanged public function; the child consumes shared private manifest models/constants and existing repositories; restore callers continue calling `validate_backup` through the facade.

## Verification Record

- Baseline: `backup_store` passed 20/20 and the existing backup-creation source contract passed 3/3 before extraction.
- Red: the new backup-validation source contract failed 0/3 because the child module, RAII workspace owner, and responsibility split did not yet exist.
- Focused green: the creation and validation source contracts passed 6/6 together; validation-workspace unit tests passed 2/2; `backup_store` retained 20/20.
- Adjacent regression: the Rust architecture boundary contract passed; `backup_restore_startup` passed 7/7; `storage_migration` passed 11/11.
- Strict/full Rust: all-target/all-feature Clippy passed with `-D warnings`; the complete Rust suite exited 0. The library target ran 127 tests: 124 passed and the three documented environment-dependent recognition probes were ignored. Every integration target passed.
- Frontend/static: complete Vitest passed 123 files and 684 tests; `vue-tsc --noEmit` passed; ESLint passed with zero warnings.
- Architecture result: `backup.rs` fell from 740 to 557 lines. Complete package verification now lives in the 270-line private direct child `backup_validation.rs`, guarded by a 104-line source contract; the public `validate_backup` surface remains a facade re-export.
- Preserved invariants: source and manifest paths remain canonical and bounded; format/schema/account/count and SQLite sidecars remain fail-closed; the copied SQLCipher database still receives ciphertext-size/hash, key, journal, integrity, foreign-key, schema, and account checks; assets retain canonical uniqueness, set equality, AES-GCM, plaintext length/hash, and aggregate-budget checks; restore callers continue through the facade.
- Lifecycle/security review: `ValidationWorkspace` owns one unpredictable direct child of the canonical temporary parent, refuses to delete anything outside that parent, cleans partial copies and every error path through `Drop`, and outlives SQL statements and the database handle. Review found no Critical or Important issue.
- Environment notes: the first complete Rust attempt hit the 120-second runner ceiling during noisy linking/test output and was rerun from cache with a larger limit; the authoritative rerun passed. Existing OpenSSL PDB `LNK4099` and SQLCipher `VirtualLock` LastError 1453 warnings remain environmental and non-fatal.
- Hygiene: all five scoped files have final newlines and zero trailing-whitespace matches; global `git diff --check` exited 0 apart from existing LF/CRLF notices; the staged index is empty. The pre-existing dirty `recognition_visual_split.rs` and all unrelated workspace changes were left untouched.
