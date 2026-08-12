# Library Lifecycle Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make first run, reinstall, cleared data, disconnected custom storage, incomplete migration, missing credentials, and backup recovery deterministic without silently creating or overwriting a library.

**Architecture:** Add a read-only startup inventory that classifies files, the storage pointer, pending operations, and the Windows credential envelope before any SQLCipher connection is opened. Represent recovery states explicitly through the Rust access gate and generated bindings, then provide restart-based retry, reconnect, backup bootstrap, and typed-confirmation fresh-start commands that are available without a live `LibraryRuntime`.

**Tech Stack:** Rust 2024, Tauri 2, SQLCipher/rusqlite, Windows Credential Manager via `keyring`, Specta-generated TypeScript bindings, Vue 3, Vitest.

## Global Constraints

- Never create a new `library.db` when any prior-library evidence exists: a complete or partial credential envelope, a storage pointer, an assets directory, a pending migration, or a pending restore.
- A first run is exactly: no database, no assets directory, no storage pointer, no pending storage/restore/reset control file, and no local-library credentials.
- Reinstall and uninstall preserve the encrypted library, storage pointer, backup/restore control files, and Windows credentials by default.
- Every destructive recovery action requires the exact confirmation text `永久放弃原资料库` and remains unavailable while an existing library can still be opened.
- Recovery must never expose database paths, Windows usernames, account IDs, credential values, or raw storage errors to Vue, logs, diagnostics, or tests.
- Custom storage must never fall back to the default location automatically.
- A retry that can change startup state must restart the process; the immutable in-process access gate must not pretend to re-evaluate startup.
- Windows x64 and ARM64 remain supported; no new network dependency or cloud requirement is introduced.

---

## File Structure

- Create `src-tauri/src/application/library_inventory.rs`: read-only credential/artifact/control-file inventory and pure startup classification.
- Modify `src-tauri/src/application/mod.rs`: export the inventory module.
- Modify `src-tauri/src/application/startup.rs`: consume the classifier before opening or creating SQLCipher.
- Modify `src-tauri/src/infrastructure/runtime_credentials.rs`: inspect and idempotently delete the complete credential envelope.
- Modify `src-tauri/src/infrastructure/runtime.rs`: export credential inventory helpers and keep database creation behind a proven first-run decision.
- Modify `src-tauri/src/infrastructure/storage_location.rs`: expose pointer presence without weakening strict pointer validation.
- Modify `src-tauri/src/commands/access.rs`: expose structured recovery reasons and restart-based retry.
- Modify `src-tauri/src/commands/storage.rs`: add existing-library reconnection without requiring `LibraryRuntime`.
- Modify `src-tauri/src/commands/backup.rs`: add missing-library backup preparation and scheduling using stored credentials.
- Modify `src-tauri/src/modules/backup_restore.rs`: support a crash-safe restore when no live library exists.
- Modify `src-tauri/src/modules/backup_restore_repository.rs`: persist whether a restore replaces an existing library or bootstraps a missing one.
- Create `src-tauri/src/modules/library_reset.rs`: idempotent fresh-start journal and credential/control-file reset.
- Modify `src-tauri/src/modules/mod.rs`: export `library_reset`.
- Modify `src-tauri/src/lib.rs`: map startup recovery reasons into managed access state and register recovery commands.
- Modify `src/shared/api/bindings.ts`: regenerate bindings after Rust DTO/command changes.
- Modify `src/app/composables/useLibraryAccessLifecycle.ts`: consume structured states and treat retry as restart.
- Modify `src/app/LibraryAccessScreen.vue`: render reason-specific recovery actions.
- Modify `src/app/App.vue`: wire reconnect, restore, fresh-start, and restart actions.
- Test in focused Rust integration tests, Vue component/composable tests, binding contracts, installer lifecycle smoke, and support-policy docs.

### Task 1: Inventory credentials and on-disk library evidence

**Files:**
- Create: `src-tauri/src/application/library_inventory.rs`
- Modify: `src-tauri/src/application/mod.rs`
- Modify: `src-tauri/src/infrastructure/runtime_credentials.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Test: `src-tauri/tests/library_startup_inventory.rs`

**Interfaces:**
- Consumes: `SecretStore::get`, `STORAGE_POINTER_FILE`, `STORAGE_PENDING_FILE`, and the existing `restore-pending.json` contract.
- Produces: `CredentialEnvelopeState`, `LibraryArtifactState`, `StartupInventory`, `StartupDisposition`, `inspect_local_credential_envelope`, `inspect_library_artifacts`, and `classify_startup_inventory`.

- [ ] **Step 1: Write the failing inventory matrix test**

```rust
#[test]
fn startup_inventory_distinguishes_first_run_from_cleared_data() {
    use mistake_trainer_next_lib::application::library_inventory::{
        CredentialEnvelopeState, LibraryArtifactState, LibraryRecoveryReason,
        StartupDisposition, StartupInventory, classify_startup_inventory,
    };

    let first_run = StartupInventory {
        credentials: CredentialEnvelopeState::Absent,
        artifacts: LibraryArtifactState::Absent,
        pointer_present: false,
        storage_migration_pending: false,
        restore_pending: false,
        reset_pending: false,
    };
    assert_eq!(classify_startup_inventory(first_run), StartupDisposition::FirstRun);

    let cleared_data = StartupInventory {
        credentials: CredentialEnvelopeState::Complete,
        ..first_run
    };
    assert_eq!(
        classify_startup_inventory(cleared_data),
        StartupDisposition::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing),
    );
}

#[test]
fn every_partial_state_fails_closed() {
    for credentials in [CredentialEnvelopeState::Partial, CredentialEnvelopeState::Absent] {
        let inventory = StartupInventory {
            credentials,
            artifacts: LibraryArtifactState::Present,
            pointer_present: false,
            storage_migration_pending: false,
            restore_pending: false,
            reset_pending: false,
        };
        assert!(matches!(
            classify_startup_inventory(inventory),
            StartupDisposition::RecoveryRequired(_)
        ));
    }
}
```

- [ ] **Step 2: Run the new test and verify the module is missing**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test library_startup_inventory`

Expected: FAIL because `application::library_inventory` does not exist.

- [ ] **Step 3: Implement the pure inventory types and exhaustive classifier**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialEnvelopeState { Absent, Complete, Partial }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryArtifactState { Absent, Present }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LibraryRecoveryReason {
    LocalDataMissing,
    CredentialsMissing,
    ResetIncomplete,
    StorageDisconnected,
    MigrationInterrupted,
    RestoreInterrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupInventory {
    pub credentials: CredentialEnvelopeState,
    pub artifacts: LibraryArtifactState,
    pub pointer_present: bool,
    pub storage_migration_pending: bool,
    pub restore_pending: bool,
    pub reset_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupDisposition {
    FirstRun,
    OpenExisting,
    ResumeMigration,
    ResumeRestore,
    RecoveryRequired(LibraryRecoveryReason),
}

pub const fn classify_startup_inventory(value: StartupInventory) -> StartupDisposition {
    use CredentialEnvelopeState::{Absent, Complete, Partial};
    use LibraryArtifactState::{Absent as NoArtifacts, Present};
    use LibraryRecoveryReason::*;

    if value.reset_pending {
        return StartupDisposition::RecoveryRequired(ResetIncomplete);
    }
    if value.storage_migration_pending {
        return StartupDisposition::ResumeMigration;
    }
    if value.restore_pending {
        return StartupDisposition::ResumeRestore;
    }
    match (value.credentials, value.artifacts, value.pointer_present) {
        (Absent, NoArtifacts, false) => StartupDisposition::FirstRun,
        (Complete, Present, _) => StartupDisposition::OpenExisting,
        (Complete, NoArtifacts, _) => StartupDisposition::RecoveryRequired(LocalDataMissing),
        (Absent, Present, _) => StartupDisposition::RecoveryRequired(CredentialsMissing),
        (Partial, _, _) => StartupDisposition::RecoveryRequired(ResetIncomplete),
        (Absent, NoArtifacts, true) => StartupDisposition::RecoveryRequired(LocalDataMissing),
    }
}
```

`inspect_local_credential_envelope` must read exactly `database-key`, `asset-key`, `account-id`, `device-id`, and `library-lock-state`. All five absent is `Absent`. Valid database/asset/account values with an absent or valid device ID and an absent, `locked`, or `unlocked` lock marker is `Complete`, preserving upgrades from versions that did not yet write the optional values. Every other combination is `Partial`. It must return `RuntimeError::SecretStore` on any read error rather than treating an unavailable credential service as absence.

- [ ] **Step 4: Run inventory tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test library_startup_inventory --lib infrastructure::runtime::tests`

Expected: PASS, including table rows for first run, cleared default data, missing credentials, partial credentials, pointer-only, database-only, assets-only, pending migration, pending restore, and pending reset. Pending migration/restore rows classify as resumable, not immediately failed.

- [ ] **Step 5: Commit the inventory boundary**

```powershell
git add src-tauri/src/application/library_inventory.rs src-tauri/src/application/mod.rs src-tauri/src/infrastructure/runtime.rs src-tauri/src/infrastructure/runtime_credentials.rs src-tauri/tests/library_startup_inventory.rs
git commit -m "feat: classify local library startup evidence"
```

### Task 2: Enforce the startup state machine before SQLCipher creation

**Files:**
- Modify: `src-tauri/src/application/startup.rs`
- Modify: `src-tauri/src/infrastructure/storage_location.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/runtime_state.rs`
- Test: `src-tauri/tests/storage_location.rs`

**Interfaces:**
- Consumes: Task 1's inventory and classifier.
- Produces: `LibraryStartup::RecoveryRequired(LibraryRecoveryReason)` and `storage_pointer_present(control_root) -> Result<bool, StorageLocationError>`.

- [ ] **Step 1: Add failing tests for the no-empty-library invariant**

```rust
#[test]
fn complete_credentials_and_missing_default_data_require_recovery() {
    let control = tempfile::tempdir().unwrap();
    let secrets = complete_secrets();

    let result = initialize_configured_application_library_if_accessible(
        control.path(), &secrets, 42,
    ).unwrap();

    assert!(matches!(
        result,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
    ));
    assert!(!control.path().join("library/library.db").exists());
}

#[test]
fn genuinely_empty_install_creates_the_first_library() {
    let control = tempfile::tempdir().unwrap();
    let result = initialize_configured_application_library_if_accessible(
        control.path(), &MemorySecrets::default(), 42,
    ).unwrap();
    assert!(matches!(result, LibraryStartup::Ready(_)));
    assert!(control.path().join("library/library.db").is_file());
}
```

- [ ] **Step 2: Run the focused startup tests and verify the cleared-data case fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test runtime_state --test storage_location`

Expected: FAIL because complete credentials plus missing default data currently creates an empty database.

- [ ] **Step 3: Gate initialization on the classified disposition**

In `initialize_configured_application_library_if_accessible`, preserve the existing lock check, strictly resolve custom storage, inspect the resolved library root without creating directories, classify the inventory, and use this exhaustive branch:

```rust
match classify_startup_inventory(inventory) {
    StartupDisposition::FirstRun => initialize_application_library(
        storage.library_root(), secrets, now_utc_ms,
    ).map(LibraryStartup::Ready),
    StartupDisposition::OpenExisting => initialize_application_library(
        storage.library_root(), secrets, now_utc_ms,
    ).map(LibraryStartup::Ready),
    StartupDisposition::ResumeMigration => match apply_pending_storage_migration(
        control_root, secrets, now_utc_ms,
    ) {
        Ok(Some(runtime)) => Ok(LibraryStartup::Ready(runtime)),
        Ok(None) | Err(_) => Ok(LibraryStartup::RecoveryRequired(
            LibraryRecoveryReason::MigrationInterrupted,
        )),
    },
    StartupDisposition::ResumeRestore => initialize_application_library(
        storage.library_root(), secrets, now_utc_ms,
    ).map(LibraryStartup::Ready).or_else(|_| Ok(LibraryStartup::RecoveryRequired(
        LibraryRecoveryReason::RestoreInterrupted,
    ))),
    StartupDisposition::RecoveryRequired(reason) => {
        Ok(LibraryStartup::RecoveryRequired(reason))
    }
}
```

Map an unavailable strict custom pointer to `StorageDisconnected`, and map a pending migration application failure to `MigrationInterrupted`. Do not call `create_dir_all`, `open_encrypted_database`, or credential creation before the branch chooses `FirstRun` or `OpenExisting`.

- [ ] **Step 4: Run startup, storage, and command-boundary tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test runtime_state --test storage_location --test command_contract`

Expected: PASS; the missing-default-data test must also assert that no `library` directory was created.

- [ ] **Step 5: Commit the startup invariant**

```powershell
git add src-tauri/src/application/startup.rs src-tauri/src/infrastructure/storage_location.rs src-tauri/src/infrastructure/runtime.rs src-tauri/src/lib.rs src-tauri/tests/runtime_state.rs src-tauri/tests/storage_location.rs
git commit -m "fix: fail closed when prior library evidence is incomplete"
```

### Task 3: Expose structured recovery state and make retry restart-based

**Files:**
- Modify: `src-tauri/src/commands/access.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/shared/api/bindings.ts`
- Modify: `src/app/composables/useLibraryAccessLifecycle.ts`
- Test: `src-tauri/tests/command_contract.rs`
- Test: `src/app/composables/useLibraryAccessLifecycle.test.ts`

**Interfaces:**
- Consumes: `LibraryRecoveryReason` from Task 1.
- Produces: `LibraryAccessState`, a structured `LibraryAccessStatus`, and `library_access_retry(app) -> AppResult<bool>`.

- [ ] **Step 1: Write failing Rust and Vue tests for structured recovery**

```rust
let AppResult::Success { data, .. } = access_status_for(
    &LibraryAccessGate::recovery(LibraryRecoveryReason::LocalDataMissing),
) else { panic!("a known recovery state is not an IPC failure") };
assert_eq!(data.state, LibraryAccessState::RecoveryRequired);
assert_eq!(data.recovery_reason, Some(LibraryRecoveryReason::LocalDataMissing));
```

```ts
await lifecycle.checkLibraryAccess()
expect(lifecycle.phase.value).toBe('recovery')
expect(lifecycle.recoveryReason.value).toBe('local_data_missing')
expect(initializeWorkspace).not.toHaveBeenCalled()
```

- [ ] **Step 2: Run tests and verify current error-code inference fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract; pnpm test -- src/app/composables/useLibraryAccessLifecycle.test.ts`

Expected: FAIL because the DTO has only `locked` and Vue infers storage from one generic failure code.

- [ ] **Step 3: Replace Boolean/error inference with a closed DTO**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LibraryAccessState { Unlocked, Locked, RecoveryRequired }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAccessStatus {
    pub state: LibraryAccessState,
    pub trusted_windows_account: bool,
    pub recovery_reason: Option<LibraryRecoveryReason>,
}
```

Known recovery states return `AppResult::success`; only an actual command/credential-service failure returns `AppResult::failure`. Add `library_access_retry` that accepts only a recovery gate, schedules `app.restart()` after 180 ms, and returns `true`; it must not mutate the immutable gate or claim that an in-process recheck occurred.

- [ ] **Step 4: Regenerate bindings and update the composable**

Run: `pnpm bindings:generate`

Update `useLibraryAccessLifecycle` so `checking`, `unlocked`, `locked`, `recovery`, `unlocking`, and `restarting` are the only phases. Store `recoveryReason` from `result.data.recoveryReason`; remove the `LIBRARY_STORAGE_UNAVAILABLE` string comparison. The retry action calls `libraryAccessRetry`, moves to `restarting`, and never invokes `libraryAccessStatus` a second time in the same process.

- [ ] **Step 5: Run binding and lifecycle tests**

Run: `pnpm test -- src/shared/api/bindings.test.ts src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.profile.test.ts`

Expected: PASS; tests must assert that retry invokes restart once and does not call workspace initialization.

- [ ] **Step 6: Commit structured access state**

```powershell
git add src-tauri/src/commands/access.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs src-tauri/tests/command_contract.rs src/shared/api/bindings.ts src/app/composables/useLibraryAccessLifecycle.ts src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.profile.test.ts
git commit -m "feat: expose explicit library recovery states"
```

### Task 4: Reconnect an existing library safely

**Files:**
- Modify: `src-tauri/src/commands/storage.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/infrastructure/storage_location.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts`
- Test: `src-tauri/tests/storage_location.rs`
- Test: `src-tauri/tests/library_command.rs`

**Interfaces:**
- Consumes: `ApplicationControlRoot`, stored restore credentials, strict product-owned suffix checks, and the recovery gate.
- Produces: `validate_existing_library(root, credentials) -> Result<(), RuntimeError>` and `storage_reconnect_select(app, gate, control_root) -> AppResult<bool>`.

- [ ] **Step 1: Write failing reconnect tests**

Create a valid encrypted library under `<selected>/Mistake Trainer Next Data/library`, remove the original pointer, invoke the pure reconnect helper, and assert that the pointer changes only after SQLCipher opens with the stored database key, the database account matches the stored account ID, and all asset paths remain contained. Add negative rows for wrong key, wrong account, missing `library.db`, symlink/reparse ancestry, and a non-recovery gate.

- [ ] **Step 2: Run reconnect tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location --test library_command`

Expected: FAIL because no reconnect command or non-mutating validation helper exists.

- [ ] **Step 3: Implement validate-then-commit reconnect**

The native dialog title is `选择原来的 Mistake Trainer Next Data 文件夹`. Treat cancellation as `AppResult::success(false)`. Resolve only `selected.join("library")`, require the exact owned suffix, open the database with stored credentials, run `PRAGMA quick_check`, verify one matching local account, close the connection, atomically write `storage-location.json`, and then schedule a restart. Never write a pointer before validation succeeds and never create a directory while validating.

- [ ] **Step 4: Run reconnect and binding tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location --test library_command --test command_contract; pnpm bindings:generate; pnpm test -- src/shared/api/bindings.test.ts`

Expected: PASS; serialized failures contain neither the selected path nor `library.db`.

- [ ] **Step 5: Commit reconnect recovery**

```powershell
git add src-tauri/src/commands/storage.rs src-tauri/src/infrastructure/runtime.rs src-tauri/src/infrastructure/storage_location.rs src-tauri/src/bindings.rs src-tauri/tests/storage_location.rs src-tauri/tests/library_command.rs src/shared/api/bindings.ts src/shared/api/bindings.test.ts
git commit -m "feat: reconnect a validated existing library"
```

### Task 5: Bootstrap a missing library from an encrypted backup

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/modules/backup_restore.rs`
- Modify: `src-tauri/src/modules/backup_restore_repository.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts`
- Test: `src-tauri/tests/backup_restore_startup.rs`
- Test: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Consumes: existing `prepare_backup_restore`, backup validation, stored credentials, and Task 3's recovery gate.
- Produces: `RestoreMode::{ReplaceExisting,BootstrapMissing}`, `backup_recovery_prepare`, and `backup_recovery_restore`.

- [ ] **Step 1: Add the missing-live restore state test**

```rust
#[test]
fn verified_backup_bootstraps_when_live_library_is_absent() {
    let fixture = prepared_restore_fixture_without_live_library();
    schedule_backup_restore_with_mode(
        &fixture.application_root,
        &fixture.candidate_id,
        &fixture.database_key,
        &fixture.asset_key,
        &fixture.account_id,
        100,
        RestoreMode::BootstrapMissing,
    ).unwrap();

    let swap = begin_pending_restore(
        &fixture.application_root,
        &fixture.database_key,
        &fixture.asset_key,
        &fixture.account_id,
        101,
    ).unwrap().unwrap();
    assert!(fixture.application_root.join("library/library.db").is_file());
    swap.commit(102).unwrap();
    assert!(!fixture.application_root.join("restore-pending.json").exists());
}
```

Also test that `BootstrapMissing` rejects an existing live library and `ReplaceExisting` continues to reject the `(false, true, false)` state.

- [ ] **Step 2: Run restore startup tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup --test backup_store`

Expected: FAIL because the existing state table treats `(live=false, stage=true, rollback=false)` as integrity failure.

- [ ] **Step 3: Version the pending marker and implement bootstrap mode**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode { ReplaceExisting, BootstrapMissing }

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingRestoreMarker {
    pub schema_version: u32,
    pub candidate_id: String,
    pub rollback_id: String,
    pub label: String,
    pub scheduled_at_utc_ms: i64,
    pub mode: RestoreMode,
}
```

For bootstrap mode, validate the staged candidate again, rename stage to live, and return a `RestoreSwap` with no rollback. If live initialization fails, `RestoreSwap::rollback` must rename live back to its exact stage path and keep the validated candidate available; it must never delete the only recovered copy. Existing replace-mode behavior remains unchanged.

- [ ] **Step 4: Add recovery commands without `LibraryRuntime`**

`backup_recovery_prepare` obtains credentials from the managed secret store, derives the default application root from `ApplicationControlRoot`, and calls existing bounded backup preparation. `backup_recovery_restore` accepts only `LocalDataMissing`, schedules `RestoreMode::BootstrapMissing`, and restarts. It rejects storage-disconnected recovery so an offline custom library cannot accidentally be replaced by a default copy.

- [ ] **Step 5: Run restore, command, and binding tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup --test backup_store --test command_contract; pnpm bindings:generate; pnpm test -- src/shared/api/bindings.test.ts`

Expected: PASS, including fault injection before rename, after rename, on initialization, on commit, and on rollback.

- [ ] **Step 6: Commit bootstrap restore**

```powershell
git add src-tauri/src/commands/backup.rs src-tauri/src/modules/backup_restore.rs src-tauri/src/modules/backup_restore_repository.rs src-tauri/src/bindings.rs src-tauri/tests/backup_restore_startup.rs src-tauri/tests/backup_store.rs src/shared/api/bindings.ts
git commit -m "feat: recover a missing library from encrypted backup"
```

### Task 6: Add an explicit idempotent fresh-start recovery

**Files:**
- Create: `src-tauri/src/modules/library_reset.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/infrastructure/runtime_credentials.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/commands/access.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts`
- Test: `src-tauri/tests/library_reset.rs`

**Interfaces:**
- Consumes: the recovery gate, `ApplicationControlRoot`, and strict control-file helpers.
- Produces: `SecretStore::delete`, `reset_missing_library`, and `library_recovery_start_fresh(app, confirmation)`.

- [ ] **Step 1: Write failing reset idempotency tests**

Test complete credentials, partial credentials, a failure after two credential deletions, retry after failure, wrong confirmation text, an unlocked/live library, a custom-storage pointer, and a reparse control root. The successful assertion is: all five local secrets (`database-key`, `asset-key`, `account-id`, `device-id`, `library-lock-state`) are absent, only product-owned recovery control files are removed, no external/custom directory is deleted, and the next startup classifies as `FirstRun`.

- [ ] **Step 2: Run reset tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test library_reset`

Expected: FAIL because `SecretStore` has no delete operation and no reset journal exists.

- [ ] **Step 3: Implement an idempotent reset journal**

Write `library-reset-pending.json` beneath the validated control root before deleting any credential. The journal contains only `{"schemaVersion":1,"reason":"user_confirmed_fresh_start"}`. Delete the five exact credential names idempotently, remove only `storage-location.json`, `storage-migration-pending.json`, `storage-migration-receipt.json`, and an invalid missing-library restore marker, then remove the reset journal last. A failed deletion leaves the journal so startup reports `ResetIncomplete` and the same action can resume.

The command accepts only `LocalDataMissing`, `CredentialsMissing`, or `ResetIncomplete`, requires `confirmation == "永久放弃原资料库"`, and restarts only after the journal is removed. It always rejects `StorageDisconnected`; that state exposes reconnect and restart only, so an offline but intact custom library cannot be abandoned from this command.

- [ ] **Step 4: Run reset, runtime, and command tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test library_reset --test runtime_state --test command_contract`

Expected: PASS; the injected partial deletion must converge on the second invocation without changing any path outside the control root.

- [ ] **Step 5: Commit fresh-start recovery**

```powershell
git add src-tauri/src/modules/library_reset.rs src-tauri/src/modules/mod.rs src-tauri/src/infrastructure/runtime_credentials.rs src-tauri/src/infrastructure/runtime.rs src-tauri/src/commands/access.rs src-tauri/src/bindings.rs src-tauri/tests/library_reset.rs src/shared/api/bindings.ts
git commit -m "feat: recover safely from an incomplete local reset"
```

### Task 7: Build the reason-specific recovery experience

**Files:**
- Modify: `src/app/LibraryAccessScreen.vue`
- Modify: `src/app/LibraryAccessScreen.test.ts`
- Modify: `src/app/composables/useLibraryAccessLifecycle.ts`
- Modify: `src/app/composables/useLibraryAccessLifecycle.test.ts`
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`
- Create: `src/app/LibraryFreshStartDialog.vue`
- Create: `src/app/LibraryFreshStartDialog.test.ts`

**Interfaces:**
- Consumes: Tasks 3-6 commands and `LibraryRecoveryReason`.
- Produces: retry/restart, reconnect, restore-backup, and confirmed-fresh-start user flows.

- [ ] **Step 1: Write failing component tests for each recovery reason**

Assert this exact action matrix:

| Reason | Primary actions | Destructive action |
|---|---|---|
| `storage_disconnected` | `重新连接原位置`, `重新启动并检查` | none |
| `local_data_missing` | `从加密备份恢复`, `查找已有资料库` | `放弃原资料并重新开始` |
| `credentials_missing` | `重新启动并检查` | none |
| `reset_incomplete` | `继续完成重新开始` | confirmation dialog |
| `migration_interrupted` | `重新启动并继续迁移` | none |
| `restore_interrupted` | `重新启动并继续恢复` | none |

The fresh-start confirm button remains disabled until the exact text `永久放弃原资料库` is entered; Escape cancels and restores focus.

- [ ] **Step 2: Run UI tests**

Run: `pnpm test -- src/app/LibraryAccessScreen.test.ts src/app/LibraryFreshStartDialog.test.ts src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.profile.test.ts`

Expected: FAIL because the current screen accepts only `credentials | storage` and its retry does not restart.

- [ ] **Step 3: Implement reason-specific copy and event contracts**

`LibraryAccessScreen` emits `retry`, `reconnect`, `restore`, and `startFresh`. It never claims that data still exists for `local_data_missing`; it says the encrypted identity remains but the database file was not found. For `storage_disconnected`, retain the promise that no default empty library is created. For migration/restore interruption, say that the application stopped before opening the library and will resume on restart.

- [ ] **Step 4: Wire commands with single-flight guards**

In `App.vue`, coalesce each recovery operation, disable all sibling actions while one is active, move to `restarting` only after native success, preserve the dialog on retryable failure, and never initialize the workspace from a recovery state. Reuse `BackupRestoreDialog` candidate presentation for backup bootstrap, but invoke `backupRecoveryRestore` rather than the live-runtime restore command.

- [ ] **Step 5: Run focused UI and accessibility tests**

Run: `pnpm test -- src/app/LibraryAccessScreen.test.ts src/app/LibraryFreshStartDialog.test.ts src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.profile.test.ts src/app/BackupRestoreDialog.test.ts`

Expected: PASS with no duplicate native invocation, no action available in the wrong reason state, focus restoration on every dialog exit, and correct `alert`/`status` live regions.

- [ ] **Step 6: Commit the recovery UI**

```powershell
git add src/app/LibraryAccessScreen.vue src/app/LibraryAccessScreen.test.ts src/app/LibraryFreshStartDialog.vue src/app/LibraryFreshStartDialog.test.ts src/app/composables/useLibraryAccessLifecycle.ts src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.vue src/app/App.profile.test.ts
git commit -m "feat: guide users through library recovery"
```

### Task 8: Lock the installer lifecycle contract and full regression matrix

**Files:**
- Modify: `scripts/windows-installer-smoke.ps1`
- Modify: `tests/windows-installer-smoke-selection.test.ts`
- Create: `tests/windows-library-lifecycle-contract.test.ts`
- Modify: `docs/windows-support-policy.md`
- Modify: `docs/windows-release-runbook.md`
- Create: `docs/windows-library-lifecycle-acceptance.md`

**Interfaces:**
- Consumes: completed startup/recovery flows and the isolated smoke harness from the companion installer-smoke plan.
- Produces: release gates proving upgrade/uninstall data preservation and all partial-state outcomes.

- [ ] **Step 1: Add failing source and installed-product contracts**

The source contract must assert that `initialize_local_library` is reached only from `FirstRun | OpenExisting`, that Vue contains no `LIBRARY_STORAGE_UNAVAILABLE` inference, and that retry invokes `library_access_retry`. The installed-product smoke seeds a random sentinel under the isolated control root, reinstalls the same version, silently uninstalls binaries, and asserts that the sentinel and encrypted library remain unchanged.

- [ ] **Step 2: Run the contracts before updating the smoke harness**

Run: `pnpm test -- tests/windows-library-lifecycle-contract.test.ts tests/windows-installer-smoke-selection.test.ts`

Expected: FAIL because lifecycle preservation and state-machine structure are not yet asserted.

- [ ] **Step 3: Add the explicit release matrix**

Document and execute these rows: new install; same-version reinstall; upgrade; app running during reinstall; default library present; default library cleared with credentials retained; custom pointer present/target online; custom pointer present/target offline; pointer cleared/custom library retained; database present/credentials absent; partial credentials; pending migration; pending replace restore; pending bootstrap restore; reset interrupted after every credential deletion; and uninstall/reinstall with data preservation.

- [ ] **Step 4: Run the full repository gates**

Run: `pnpm lint`

Expected: PASS with zero warnings.

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm test`

Expected: PASS.

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

Expected: PASS.

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

Expected: PASS.

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Expected: PASS.

- [ ] **Step 5: Record manual Windows acceptance**

On a clean x64 Windows account and an ARM64 Windows account, run the lifecycle matrix, interrupt migration/restore/reset at each journal boundary, confirm no empty library appears without explicit confirmation, confirm no recovery message exposes a path, and attach only fixed failure codes plus aggregate counts to the release record.

- [ ] **Step 6: Commit lifecycle gates and documentation**

```powershell
git add scripts/windows-installer-smoke.ps1 tests/windows-installer-smoke-selection.test.ts tests/windows-library-lifecycle-contract.test.ts docs/windows-support-policy.md docs/windows-release-runbook.md docs/windows-library-lifecycle-acceptance.md
git commit -m "test: gate resilient Windows library lifecycle"
```

## Self-Review

- Spec coverage: the plan covers first run, partial clear, custom-storage loss, immutable retry, existing-library reconnection, missing-library backup restore, idempotent fresh start, installer preservation, crash recovery, redaction, x64, and ARM64.
- Placeholder scan: no task delegates unspecified error handling or generic tests; every state and public action is named.
- Type consistency: `LibraryRecoveryReason`, `LibraryStartup::RecoveryRequired`, `LibraryAccessGate::recovery`, `LibraryAccessStatus.recovery_reason`, `RestoreMode`, and the four recovery commands keep the same names across Rust, generated bindings, Vue, and tests.
