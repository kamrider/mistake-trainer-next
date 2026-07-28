# Encrypted Backup Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing read-only backup verifier into a restart-safe encrypted restore workflow that never exposes paths to Vue and always preserves the current library when preparation or startup replacement fails.

**Architecture:** The desktop command copies one selected, fully verified backup into an application-owned staging directory and returns only an opaque candidate ID. Confirmation writes an atomic pending marker and schedules a Tauri restart; before the encrypted SQLite connection opens, startup re-verifies the staged package, swaps the entire library directory on the same volume, and retains the old directory until the restored runtime opens successfully. A bounded receipt lets the application shell explain whether restoration succeeded or the old library was rolled back.

**Tech Stack:** Rust stable, rusqlite + SQLCipher, serde, SHA-256, Tauri 2.11.x, tauri-specta, Vue 3, TypeScript strict, Vitest and Testing Library.

## Global Constraints

- Windows is the only v1 release platform and all restore swaps must work with Windows file locking.
- Vue receives no filesystem paths, database keys, asset keys, account IDs, or manifest hashes.
- A prepared candidate expires after 24 hours and is re-verified immediately before scheduling.
- Restore is applied only before opening the live database; no command replaces files while the application is serving requests.
- The current library remains available until the restored database has opened, migrated, and selected a valid active profile.
- A failed or interrupted apply must recover from directory presence, not from an optimistic in-memory flag.
- Only application-owned children matching `.mistake-trainer-restore-<UUIDv7>` and `.mistake-trainer-rollback-<UUIDv7>` may be renamed or removed.
- Restore UI must require an explicit second confirmation, trap focus, and respect `prefers-reduced-motion`.

---

### Task 1: Verified Application-Owned Restore Candidate

**Files:**
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Consumes: `validate_backup(source, database_key, asset_key, account_id)` and the existing manifest/path safety helpers.
- Produces: `prepare_backup_restore(...) -> Result<BackupRestoreCandidate, BackupError>` and `validate_restore_candidate(...) -> Result<BackupSummary, BackupError>`.

- [ ] **Step 1: Write failing candidate tests**

Add tests that prepare a package beneath an application-data parent, assert the returned ID is opaque, assert the staged database/assets/manifest validate, and assert tampering after preparation is rejected without touching the live library.

```rust
let candidate = prepare_backup_restore(
    &package,
    application_root.path(),
    DATABASE_KEY,
    &ASSET_KEY,
    ACCOUNT_ID,
    1_725_000_000_000,
)?;
assert!(Uuid::parse_str(&candidate.id).is_ok());
assert!(!serde_json::to_string(&candidate)?.contains(package.to_string_lossy().as_ref()));
assert!(validate_restore_candidate(
    application_root.path(), &candidate.id, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID,
    1_725_000_000_001,
).is_ok());
```

- [ ] **Step 2: Run the focused test and observe the missing interface**

Run: `scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store prepare_restore -- --nocapture`

Expected: compilation fails because `prepare_backup_restore` and `BackupRestoreCandidate` do not exist.

- [ ] **Step 3: Implement exact candidate metadata and copying**

Add serializable public DTOs and private metadata:

```rust
pub const RESTORE_CANDIDATE_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreCandidate {
    pub id: String,
    pub summary: BackupSummary,
    pub expires_at_utc_ms: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreCandidateMetadata {
    id: String,
    label: String,
    prepared_at_utc_ms: i64,
    manifest_sha256: String,
}
```

`prepare_backup_restore` must validate the selected package, create `.<name>.tmp` with `create_new` metadata, copy only `manifest.json`, `library.db`, and manifest-listed assets through the existing bounded hash copier, validate the copy, and atomically rename it to `.mistake-trainer-restore-<id>`. On every error it removes only its newly created temporary directory.

- [ ] **Step 4: Implement revalidation and expiry**

`validate_restore_candidate` must parse the ID as UUID, derive the directory beneath the supplied application root, reject reparse points, read metadata with a 64 KiB bound, require the directory name and metadata ID to match, require `now - prepared_at` in `0..=RESTORE_CANDIDATE_TTL_MS`, compare the current manifest SHA-256 with metadata, and then call `validate_backup` again.

- [ ] **Step 5: Run the full backup store test**

Run: `scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store`

Expected: every backup test passes and the original package/live library hashes are unchanged.

### Task 2: Restart-Time Swap and Automatic Rollback

**Files:**
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/backup_store.rs`
- Create: `src-tauri/tests/backup_restore_startup.rs`

**Interfaces:**
- Consumes: the candidate produced by Task 1 and `initialize_local_library`.
- Produces: `schedule_backup_restore`, `begin_pending_restore`, `RestoreSwap::commit`, `RestoreSwap::rollback`, and `read_restore_receipt`.

- [ ] **Step 1: Write startup swap and rollback tests**

Cover these filesystem states with temporary directories:

```text
normal:       library + stage + marker
mid-swap:     rollback + stage + marker, library absent
post-swap:    rollback + library + marker, stage absent
invalid:      library + tampered stage + marker
```

The normal and mid-swap cases must produce a `RestoreSwap` with the restored library at the fixed path. The invalid case must leave the original library byte-for-byte unchanged. Calling `rollback()` must restore the old database/assets; calling `commit()` must delete only the verified rollback directory and write a success receipt.

- [ ] **Step 2: Expose restore credentials without exposing secrets to commands**

Add a crate-visible runtime helper:

```rust
pub(crate) struct RestoreCredentials {
    pub database_key: String,
    pub asset_key: [u8; 32],
    pub account_id: String,
}

pub(crate) fn load_restore_credentials(
    secrets: &dyn SecretStore,
) -> Result<RestoreCredentials, RuntimeError>;
```

It must require all three existing secrets, validate UUID/key formats, and never create replacement credentials.

- [ ] **Step 3: Implement the pending marker and recoverable swap**

Use `restore-pending.json` under the application-data parent with a 64 KiB maximum. `schedule_backup_restore` revalidates the candidate and writes the marker through `restore-pending.<uuid>.tmp`, `sync_all`, and same-directory rename. `begin_pending_restore` derives only safe child names, recognizes the three recoverable states above, and returns a guard holding the live/rollback paths and label.

- [ ] **Step 4: Integrate before runtime initialization**

In `src-tauri/src/lib.rs`, load restore credentials and call `begin_pending_restore` before `initialize_local_library`. If restored initialization succeeds, commit the swap and manage that runtime. If it fails, roll back, initialize the old library, and write a `rolled_back` receipt. An invalid marker/candidate is cleared, recorded as `failed_validation`, and the old library opens normally.

- [ ] **Step 5: Run startup and runtime tests**

Run: `scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup`

Expected: all crash-state, commit, rollback, and old-library-fallback tests pass.

### Task 3: Typed Commands and Restart Scheduling

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/commands/insights.rs`
- Modify: `src/shared/api/bindings.test.ts`
- Regenerate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `backupPrepareRestore()`, `backupRestore(candidateId)`, and `backupRestoreStatus()`.

- [ ] **Step 1: Add binding contract assertions**

```ts
expect(source).toContain('backupPrepareRestore: () =>')
expect(source).toContain('backupRestore: (candidateId: string) =>')
expect(source).toContain('backupRestoreStatus: () =>')
expect(source).not.toContain('sourcePath')
```

- [ ] **Step 2: Replace the read-only validation command**

`backup_prepare_restore` opens the folder picker, stages the candidate under the runtime application-data parent, and returns `AppResult<Option<BackupRestoreCandidate>>`. Cancellation returns successful `null` and clears no existing data.

- [ ] **Step 3: Add confirmation scheduling**

`backup_restore` accepts only `candidate_id`, stops the active `CaptureLanManager`, calls `schedule_backup_restore`, returns `AppResult<bool>`, then starts a named thread that waits 350 ms and calls `AppHandle::restart()`. If restart cannot happen, the pending marker safely applies on the next manual launch.

- [ ] **Step 4: Add one-shot receipt reading**

`backup_restore_status` reads and removes a valid bounded receipt, returns `AppResult<Option<BackupRestoreReceipt>>`, and maps malformed receipts to a path-free diagnostic error.

- [ ] **Step 5: Regenerate and test bindings**

Run: `pnpm bindings:generate`

Run: `pnpm test -- src/shared/api/bindings.test.ts`

Expected: the typed commands and DTOs are present, and no local path appears in public types.

### Task 4: Explicit, Calm Restore Experience

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/App.vue`
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/AppShell.test.ts`

**Interfaces:**
- Consumes: `BackupRestoreCandidate`, `BackupRestoreReceipt`, `backupPrepareRestore`, `backupRestore`, and `backupRestoreStatus`.

- [ ] **Step 1: Write the settings interaction test**

Test that preparation renders label/time/size, “开始安全恢复” opens a focus-trapped dialog, the confirm button remains disabled until the user checks “我知道当前资料库将被备份包替换”, Escape restores focus, and confirmation invokes only `backupRestore(candidate.id)`.

- [ ] **Step 2: Replace misleading validation copy**

Use these states exactly:

```text
idle:       选择并验证恢复包
preparing:  正在复制并验证…
ready:      已安全暂存，尚未替换当前资料库
restarting: 正在关闭资料库并安全重启…
```

Explain that the existing library is retained until the restored one opens successfully and that interrupted restarts resume on next launch.

- [ ] **Step 3: Implement the confirmation dialog**

The dialog uses `role="dialog"`, `aria-modal="true"`, initial focus on Cancel, Tab/Shift+Tab containment, Escape cancellation, and focus restoration. Its panel enters with opacity plus `translateY(8px) scale(.985)` over the standard 180–240 ms tokens; reduced motion removes transforms and transitions.

- [ ] **Step 4: Show the restart receipt globally**

`App.vue` calls `backupRestoreStatus()` once after system status loads. Pass a dismissible notice to `AppShell`: success uses “加密备份已恢复，题库与采集草稿已重新载入。”; rollback uses “恢复没有完成，已自动回到原资料库。” The notice uses `role="status"` for success and `role="alert"` for rollback.

- [ ] **Step 5: Run the frontend tests**

Run: `pnpm test -- src/app/views/SettingsView.test.ts src/app/AppShell.test.ts src/app/App.test.ts`

Expected: preparation, focus management, restart busy state, receipt display, and dismissal pass.

### Task 5: Documentation and Full Quality Gate

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/foundation.md`
- Modify: `docs/windows-acceptance.md`

**Interfaces:**
- Documents: candidate staging, startup-only swap, rollback guarantee, receipt behavior, and Windows interruption checks.

- [ ] **Step 1: Document recovery invariants**

Record that restore never runs against an open database, candidates are application-owned and opaque, account/key/integrity validation runs twice, and the previous root is deleted only after restored runtime initialization.

- [ ] **Step 2: Add Windows acceptance cases**

Document normal restore, cancel, corrupt candidate, disk-full preparation, force-close after confirmation, and power-loss simulations for the pre-swap/mid-swap/post-swap directory states.

- [ ] **Step 3: Run all quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
pnpm tauri build
```

Expected: every command exits 0. Existing OpenSSL missing-PDB and SQLCipher `VirtualLock` warnings may remain non-fatal; no new warning is accepted.

- [ ] **Step 4: Perform visual verification**

In the desktop-width browser preview, inspect idle, prepared, confirmation, and restarting states. Verify no horizontal overflow at 760 px, focus ring visibility, and no transform animation with reduced motion.

- [ ] **Step 5: Commit the complete vertical slice**

```powershell
git add docs src src-tauri
git commit -m "feat: restore encrypted backups safely"
```

Expected: the worktree is clean and `pnpm bindings:check` remains green after the commit.

## Self-Review

- Spec coverage: the plan covers preparation, path secrecy, double validation, expiry, startup-only application, Windows-safe directory replacement, rollback, restart, global feedback, focus management, reduced motion, crash recovery, tests, docs, and build gates.
- Placeholder scan: every task names concrete files, interfaces, commands, expected states, and failure assertions; no deferred implementation step remains.
- Type consistency: `BackupRestoreCandidate.id` is the sole frontend token; `backupRestore(candidateId)` schedules the marker; `BackupRestoreReceipt` is consumed only by `backupRestoreStatus()` and the app shell.
