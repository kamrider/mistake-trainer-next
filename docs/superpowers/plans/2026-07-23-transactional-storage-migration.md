# Transactional Storage Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a Windows user move the encrypted local library to another folder or drive from Settings without exposing paths to Vue, losing data, opening a partial copy, or breaking backup/restore.

**Architecture:** Keep a small, atomically written storage pointer and migration journal in Tauri's fixed application-data control directory. The selected destination contains a product-owned `Mistake Trainer Next Data/library` tree; Rust snapshots SQLCipher while holding the database mutex, copies only database-referenced encrypted assets into a staging tree, verifies every copied byte and account boundary, then schedules a restart. Startup opens and validates the destination before committing the pointer, otherwise it reopens the untouched source and records a rollback receipt.

**Tech Stack:** Rust stable, rusqlite/SQLCipher backup API, SHA-256, serde JSON, Tauri 2.11, native `rfd` folder dialog, Vue 3, TypeScript strict, Vitest.

## Global Constraints

- The fixed control directory remains `app.path().app_data_dir()`; moving the library never moves the storage pointer, migration journal, or receipt.
- Missing `storage-location.json` means the existing default `<control>/library` location.
- A malformed pointer, unavailable custom drive, invalid journal, or failed destination validation fails closed and must never create a replacement empty library.
- Vue never submits or stores a filesystem path. The destination is produced only by the Rust folder dialog; DTOs expose a redacted drive/folder label, byte counts, and status.
- The selected parent receives exactly one product-owned child named `Mistake Trainer Next Data`; non-empty unowned destinations are rejected.
- Source and destination may not be equal, nested, ancestors of one another, or inside the fixed control directory.
- Directory traversal must reject symlinks and Windows reparse points and must never follow a junction.
- The command stops LAN capture before snapshotting and holds the database mutex until the database snapshot and referenced-asset copy are complete.
- Only assets referenced by the current account's encrypted database are copied. SQLite `-wal`, `-shm`, staging, preview, export, and cache files are not copied.
- Every source/destination asset pair is compared by byte length and SHA-256. The staged SQLCipher database must open with the existing key, match the existing account, pass migrations, and reference exactly the staged assets.
- The source library is untouched until destination verification and pointer commit succeed. A failure removes only the new product-owned staging/final destination.
- Successful migration enters the root `restarting` phase before the 450 ms native restart, so `AppShell` and every mutation command are unmounted.
- New UI motion uses only `opacity` and `transform`, lasts at most 240 ms, and is disabled by `prefers-reduced-motion: reduce`.
- No new runtime dependency is introduced.

---

### Task 1: Fixed control pointer and fail-closed startup resolution

**Files:**
- Create: `src-tauri/src/infrastructure/storage_location.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/application/startup.rs`
- Modify: `src-tauri/src/commands/access.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/storage_location.rs`

**Interfaces:**
- Produces:

```rust
pub const STORAGE_POINTER_FILE: &str = "storage-location.json";
pub const STORAGE_PENDING_FILE: &str = "storage-migration-pending.json";
pub const STORAGE_RECEIPT_FILE: &str = "storage-migration-receipt.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoragePointer {
    pub schema_version: u32,
    pub library_root: PathBuf,
}

pub enum ResolvedStorage {
    Default { library_root: PathBuf },
    Custom { library_root: PathBuf },
}

pub fn resolve_storage(
    control_root: &Path,
) -> Result<ResolvedStorage, StorageLocationError>;
```

- Consumes: the fixed Tauri control directory before `initialize_application_library`.
- Changes `LibraryStartup::AccessUnavailable` to carry a public-safe reason `credentials | storage`.

- [ ] **Step 1: Write failing pointer and startup tests**

Create tests for:

```rust
#[test]
fn missing_pointer_uses_only_the_existing_default_root() {
    let control = tempdir().unwrap();
    assert_eq!(
        resolve_storage(control.path()).unwrap().library_root(),
        control.path().join("library"),
    );
}

#[test]
fn malformed_or_relative_pointer_fails_closed() {
    // Write invalid JSON, an unknown field, then a relative libraryRoot.
    // Each case returns StorageLocationError and never creates <control>/library.
}

#[test]
fn unavailable_custom_root_never_falls_back_to_an_empty_default() {
    // Point at a missing custom root while a valid default library exists.
    // Startup returns AccessUnavailable(Storage), not Ready(default).
}
```

Also assert an exact version `1`, an absolute path, and the fixed child name `Mistake Trainer Next Data/library`.

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location
```

Expected: FAIL because the module and startup resolver do not exist.

- [ ] **Step 3: Implement strict pointer parsing and atomic writes**

Use `serde_json::from_slice` with `deny_unknown_fields`, reject files over 64 KiB, reject non-absolute paths, and validate the product-owned suffix. Write JSON to a sibling UUIDv7 temporary file, call `sync_all`, rename it over the target, then sync the parent directory where supported.

`resolve_storage` must:

```rust
if !pointer_path.exists() {
    return Ok(ResolvedStorage::Default {
        library_root: control_root.join("library"),
    });
}
let pointer = read_pointer_strict(&pointer_path)?;
validate_existing_library_root(&pointer.library_root)?;
Ok(ResolvedStorage::Custom {
    library_root: pointer.library_root,
})
```

It must not create either the custom or default library directory.

- [ ] **Step 4: Route Tauri startup through the resolver**

Change the startup boundary to:

```rust
pub fn initialize_configured_application_library_if_accessible(
    control_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<LibraryStartup, StartupError>
```

Read the lock marker first. Only an unlocked marker may call `resolve_storage`; only a successfully resolved root may call `initialize_application_library`. `lib.rs` passes `app.path().app_data_dir()?` and manages no `LibraryRuntime` for locked or unavailable outcomes. Extend the immutable `LibraryAccessGate` with `credentials | storage`; `library_access_status` maps the latter to “配置的资料库位置当前不可用，未打开或创建任何资料，请重新连接磁盘后重试。” without serializing a path.

- [ ] **Step 5: Run focused and existing lifecycle tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location --test runtime_state
```

Expected: PASS, including existing lock-cycle coverage.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/infrastructure/storage_location.rs src-tauri/src/infrastructure/mod.rs src-tauri/src/application/startup.rs src-tauri/src/commands/access.rs src-tauri/src/lib.rs src-tauri/tests/storage_location.rs
git commit -m "feat: resolve the configured library location safely"
```

---

### Task 2: Verified cross-volume snapshot, journal, commit, and rollback

**Files:**
- Create: `src-tauri/src/modules/storage_migration.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/application/startup.rs`
- Test: `src-tauri/tests/storage_migration.rs`
- Modify: `src-tauri/tests/backup_restore_startup.rs`

**Interfaces:**
- Consumes: `LibraryRuntime`, existing SQLCipher keys, fixed control root, and a Rust-selected parent directory.
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageMigrationJournal {
    pub schema_version: u32,
    pub migration_id: String,
    pub source_library_root: PathBuf,
    pub destination_library_root: PathBuf,
    pub created_at_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationReceipt {
    pub outcome: StorageMigrationOutcome,
    pub destination_label: String,
    pub copied_asset_count: u32,
    pub copied_bytes: f64,
}

pub fn stage_storage_migration(
    runtime: &LibraryRuntime,
    control_root: &Path,
    selected_parent: &Path,
    now_utc_ms: i64,
) -> Result<StorageMigrationReceipt, StorageMigrationError>;

pub fn apply_pending_storage_migration(
    control_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<Option<LibraryRuntime>, StorageMigrationError>;
```

- `outcome` uses `scheduled | moved | rolled_back | cleanup_required`.

- [ ] **Step 1: Write failing path-boundary and copy tests**

Cover:

```rust
#[test]
fn rejects_current_nested_control_and_unowned_nonempty_destinations() {}

#[test]
fn rejects_a_source_asset_reparse_point_before_copying() {}

#[test]
fn snapshot_copies_only_database_referenced_encrypted_assets() {}

#[test]
fn byte_mismatch_or_copy_failure_leaves_source_and_pointer_unchanged() {}
```

The fixture must include a valid SQLCipher library, one question asset, one answer asset, an unreferenced cache file, and a fault injector that fails after the first copied asset.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_migration
```

Expected: FAIL because the migration engine does not exist.

- [ ] **Step 3: Implement the bounded snapshot**

Inside one `LibraryRuntime.connection` guard:

1. Resolve and validate source and selected destination boundaries.
2. Create `Mistake Trainer Next Data/.migration-<uuid>/library`.
3. Use `rusqlite::backup::Backup` to create a consistent encrypted database snapshot with the existing database key.
4. Query `assets.encrypted_path` for the current account in deterministic order.
5. Reject absolute, parent, alternate-stream, symlink, or reparse paths.
6. Stream-copy each encrypted asset in 1 MiB chunks while hashing source and destination.
7. Verify byte length and SHA-256 equality and accumulate counts using checked arithmetic.
8. Open the staged SQLCipher database, run migrations, verify the account boundary and exact referenced-asset set.
9. Rename the staged product directory to `Mistake Trainer Next Data`.
10. Atomically write `storage-migration-pending.json`.

On any error, remove only `.migration-<uuid>` or the just-created owned final directory; never remove a pre-existing path.

- [ ] **Step 4: Write failing startup commit and rollback tests**

Add:

```rust
#[test]
fn restart_commits_pointer_only_after_destination_opens() {}

#[test]
fn tampered_destination_rolls_back_to_source_and_records_receipt() {}

#[test]
fn committed_custom_storage_keeps_backup_restore_control_files_beside_that_library() {}
```

The first asserts the pointer does not exist before applying the journal. The second modifies one destination asset and asserts the original profile/problem still opens. The third schedules and applies an existing backup restore while custom storage is active.

- [ ] **Step 5: Apply the journal during startup**

For an unlocked startup:

1. If no journal exists, resolve the pointer and open normally.
2. If a journal exists, strictly validate both roots and the journal age (maximum 24 hours).
3. Attempt to open and validate the destination.
4. On success, atomically write the pointer, remove the journal, write a `moved` receipt, and return the destination runtime.
5. On destination failure, remove only the owned destination, remove the journal, write a `rolled_back` receipt, and open the untouched source.
6. After a successful pointer commit, delete the old source only when its canonical path still equals the journal source and it contains the expected library database. A delete failure writes `cleanup_required` without failing the new runtime.

- [ ] **Step 6: Run migration, restore, and runtime tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_migration --test backup_restore_startup --test runtime_state
```

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/modules/storage_migration.rs src-tauri/src/modules/mod.rs src-tauri/src/application/startup.rs src-tauri/tests/storage_migration.rs src-tauri/tests/backup_restore_startup.rs
git commit -m "feat: migrate encrypted storage transactionally"
```

---

### Task 3: Typed storage commands and restart boundary

**Files:**
- Create: `src-tauri/src/commands/storage.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/shared/api/bindings.ts` through generation
- Test: `src-tauri/tests/command_contract.rs`
- Test: `src-tauri/tests/bindings_contract.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocationStatus {
    pub kind: StorageLocationKind,
    pub location_label: String,
    pub database_bytes: f64,
    pub asset_bytes: f64,
    pub migration_pending: bool,
}

storage_status() -> AppResult<StorageLocationStatus>
storage_migrate_select() -> Result<AppResult<Option<StorageMigrationReceipt>>, ()>
storage_migration_receipt() -> AppResult<Option<StorageMigrationReceipt>>
```

- Consumes: `CaptureLanManager::stop`, `LibraryRuntime`, control root managed as `ApplicationControlRoot`, and the root restarting controller already used by lock/restore.

- [ ] **Step 1: Write failing command and contract tests**

Assert:

- cancellation returns `ok: true, data: null`;
- `StorageLocationStatus` never serializes `C:\`, `/Users/`, `library.db`, or an account ID;
- LAN stop failure prevents staging;
- migration failure returns stable code `storage_migration_failed`, retryability, and no internal path;
- successful scheduling returns before a 450 ms restart;
- command signatures accept no path or account/profile ID.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract --test bindings_contract
```

Expected: FAIL because the commands and bindings do not exist.

- [ ] **Step 3: Implement commands**

Manage:

```rust
pub struct ApplicationControlRoot(pub PathBuf);
```

in `lib.rs`. `storage_migrate_select` stops LAN, opens `rfd::FileDialog` in `spawn_blocking`, calls `stage_storage_migration`, and schedules `app.restart()` after 450 ms only for `AppResult::Success { data: Some(_) }`. Map cancellation to success without restart.

Public errors:

- `storage_destination_invalid`: “请选择资料库之外的空文件夹；现有资料没有变化。”
- `storage_destination_in_use`: “所选位置已有其他文件，未写入任何资料。”
- `storage_space_or_copy_failed`: “迁移没有完成，请检查磁盘空间或连接后重试；原资料仍在原位置。”
- `storage_integrity_failed`: “新位置的校验没有通过，已保留原资料库。”

- [ ] **Step 4: Register and generate bindings**

Run:

```powershell
corepack pnpm bindings:generate
```

Expected: `commands.storageStatus`, `commands.storageMigrateSelect`, and `commands.storageMigrationReceipt` exist with no path parameters.

- [ ] **Step 5: Run command, binding, and drift tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract --test bindings_contract
corepack pnpm bindings:check
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands/storage.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs src/shared/api/bindings.ts src-tauri/tests/command_contract.rs src-tauri/tests/bindings_contract.rs
git commit -m "feat: expose safe storage migration commands"
```

---

### Task 4: Settings storage card, confirmation, progress, and restart UX

**Files:**
- Create: `src/app/StorageMigrationDialog.vue`
- Create: `src/app/StorageMigrationDialog.test.ts`
- Modify: `src/app/LibraryAccessScreen.vue`
- Modify: `src/app/LibraryAccessScreen.test.ts`
- Modify: `src/app/App.vue`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: generated storage commands and `libraryAccessController.enterRestarting()`.
- Produces: a new `settings-storage` navigation target and an explicit “更换存储位置” flow.

- [ ] **Step 1: Write failing dialog and settings tests**

Assert:

- current location kind and redacted label render without an absolute path;
- database, encrypted-image, and total sizes use localized bounded units;
- the confirmation dialog initially focuses “取消” and traps focus;
- the copy explains “会复制并校验、自动重启，失败仍使用原位置”;
- cancellation invokes no command;
- folder-dialog cancellation returns to the card without an error;
- success immediately calls `enterRestarting`;
- failure keeps the dialog open with the stable Rust `userMessage`;
- busy state prevents duplicate submission;
- a `moved`, `rolled_back`, or `cleanup_required` receipt is announced once.
- a missing custom drive renders the Rust storage-specific startup error, never credential advice or `AppShell`.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
corepack pnpm exec vitest run src/app/StorageMigrationDialog.test.ts src/app/views/SettingsView.test.ts src/app/App.profile.test.ts
```

Expected: FAIL because the storage UI does not exist.

- [ ] **Step 3: Implement the accessible dialog**

Use the same safe-dialog conventions as `LibraryLockDialog`: cancel receives initial focus, Escape cancels while idle, Tab/Shift+Tab remain inside, and the panel receives focus while busy. The only confirm label is `选择文件夹并开始迁移`; no text path input or drag/drop target exists.

- [ ] **Step 4: Add the Settings storage section**

Add:

```ts
{ id: 'settings-storage', label: '存储位置', hint: '容量与迁移' }
```

Load `storageStatus` and `storageMigrationReceipt` with other settings reads. Render one paper card with location type, redacted label, database size, encrypted-image size, total size, and “更换存储位置”. On success call `libraryAccessController.enterRestarting()` before Rust restarts.

Preserve `error.userMessage` from `libraryAccessStatus` in `App.vue` and pass it to `LibraryAccessScreen`; the screen must render this safe message in its error state. Retry reruns the startup status command without mounting `AppShell`.

- [ ] **Step 5: Add restrained motion**

The dialog uses opacity plus `translateY(8px)` for 240 ms. Size rows may use a 180 ms opacity transition after refresh. No width, height, blur, shadow, or infinite animation is introduced; reduced motion removes both transitions.

- [ ] **Step 6: Run focused frontend tests**

Run:

```powershell
corepack pnpm exec vitest run src/app/StorageMigrationDialog.test.ts src/app/views/SettingsView.test.ts src/app/App.profile.test.ts
corepack pnpm typecheck
corepack pnpm lint
```

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add src/app/StorageMigrationDialog.vue src/app/StorageMigrationDialog.test.ts src/app/LibraryAccessScreen.vue src/app/LibraryAccessScreen.test.ts src/app/App.vue src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts src/app/App.profile.test.ts
git commit -m "feat: add transactional storage migration settings"
```

---

### Task 5: Documentation, full gates, and Windows acceptance

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/release.md`
- Create: `docs/windows-storage-migration-acceptance.md`
- Modify: this plan.

**Interfaces:**
- Consumes: the complete storage migration lifecycle.
- Produces: documented invariants, clean local commits, and release evidence.

- [ ] **Step 1: Document the control/data split**

Record:

- the fixed control root and movable encrypted library root;
- pointer and journal fail-closed parsing;
- snapshot, asset hashing, destination validation, restart commit, rollback, and cleanup semantics;
- backup/restore markers live beside the active library and continue to work after migration;
- custom storage unavailability never creates a new default library.

- [ ] **Step 2: Add Windows acceptance cases**

The document must cover:

1. default SSD folder to another local drive;
2. same-drive folder change;
3. dialog cancellation;
4. non-empty destination;
5. nested/current/control destination;
6. destination disconnect during copy;
7. application termination after staging but before restart;
8. tampering before restarted validation;
9. successful restart and same profile/problem/image review;
10. backup creation and restore after migration;
11. missing custom drive on a later launch;
12. reduced motion and keyboard-only dialog operation.

- [ ] **Step 3: Run full repository gates**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
corepack pnpm bindings:check
git diff --check
```

Expected: all commands exit `0`; initial JavaScript remains below 300 KB gzip.

- [ ] **Step 4: Perform native disposable-location acceptance**

Use a disposable destination under a temporary local volume/folder, not the user's actual library:

1. seed one profile and one problem with question and answer assets;
2. migrate, observe the root restarting state, and relaunch;
3. verify the same profile/problem/assets;
4. create and validate a backup from the new root;
5. remove the disposable custom drive/folder and verify startup fails closed;
6. restore the location before ending the test.

- [ ] **Step 5: Review and create the final local checkpoint**

Review the complete range for path traversal, junction following, account-boundary leaks, partial-copy cleanup, UI duplicate submission, and unrelated changes. Fix every Critical or Important finding, then:

```powershell
git add src src-tauri docs
git commit -m "feat: complete transactional storage migration"
```

Do not push without fresh user authorization.

## Self-Review

- Spec coverage: folder selection, cross-volume copy, SQLCipher snapshot, encrypted assets, path boundaries, crash journal, startup commit, rollback, cleanup, backup/restore compatibility, UI progress, restart gating, and Windows acceptance each have an explicit task and evidence.
- Placeholder scan: no `TBD`, `TODO`, “handle errors”, or unspecified test step remains.
- Type consistency: `StoragePointer`, `ResolvedStorage`, `StorageMigrationJournal`, `StorageMigrationReceipt`, `StorageLocationStatus`, command names, and Vue calls match across tasks.
- Scope boundary: this plan moves only the local encrypted library; it does not add cloud sync, portable passwords, removable-drive auto-mounting, cache migration, or installer changes.
