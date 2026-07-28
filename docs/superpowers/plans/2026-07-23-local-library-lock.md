# Local Library Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make explicit sign-out and manual lock close the encrypted local library, restart into a dedicated locked screen, and allow only the current trusted Windows account to unlock it.

**Architecture:** A persistent `library-lock-state` value lives beside the existing SQLCipher and asset keys in Windows Credential Manager. Tauri startup checks that marker before opening SQLCipher and snapshots the result in a process-scoped access gate; while locked it deliberately does not manage `LibraryRuntime`, so every forged data command fails for lack of state. Lock and unlock commands change the marker, stop LAN capture on lock, return a typed result, and schedule a process restart; Vue gates all existing shell initialization behind `library_access_status` and unmounts the root shell as soon as locking enters restart.

**Tech Stack:** Rust stable, Tauri 2.11, Windows keyring credential store, Vue 3, TypeScript, tauri-specta, Vitest, Testing Library.

## Global Constraints

- Missing `library-lock-state` means unlocked for existing installations.
- Only exact values `locked` and `unlocked` are valid; malformed values and credential-read failures fail closed and must not open SQLCipher.
- A locked startup must not call `initialize_application_library` and must not manage `LibraryRuntime`.
- The lock command persists the marker before scheduling restart and stops any active LAN capture session.
- Unlock validates the database, asset, and account credential envelope through the current Windows account; it does not ask for or derive a new weak password.
- Cloud sign-out clears local refresh/session state before a bounded remote revocation attempt, then proceeds to local locking.
- Browser preview remains unlocked and never invokes desktop lock commands.
- Locked, checking, error, and restarting states must never render `AppShell` or invoke profile/library commands.
- All new motion uses only `opacity` and `transform`, lasts at most 240 ms, and is removed by `prefers-reduced-motion: reduce`.

---

### Task 1: Persist a fail-closed library lock marker ✅

**Files:**
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/application/startup.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces:

```rust
pub const LIBRARY_LOCK_STATE: &str = "library-lock-state";
pub fn library_is_locked(secrets: &dyn SecretStore) -> Result<bool, RuntimeError>;
pub fn set_library_locked(
    secrets: &dyn SecretStore,
    locked: bool,
) -> Result<(), RuntimeError>;
```

- Consumes: existing `SecretStore`, `KeyringSecretStore`, and `initialize_application_library`.

- [x] **Step 1: Add failing lock-marker tests**

Add an in-memory `SecretStore` in `runtime.rs` tests and assert:

```rust
assert!(!library_is_locked(&store).unwrap());
set_library_locked(&store, true).unwrap();
assert!(library_is_locked(&store).unwrap());
set_library_locked(&store, false).unwrap();
assert!(!library_is_locked(&store).unwrap());
store.set(LIBRARY_LOCK_STATE, "corrupt").unwrap();
assert!(matches!(
    library_is_locked(&store),
    Err(RuntimeError::InvalidLibraryLockState)
));
```

- [x] **Step 2: Run the focused Rust test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml infrastructure::runtime::tests::library_lock_marker_is_strict
```

Expected: FAIL because the marker API does not exist.

- [x] **Step 3: Implement strict marker parsing and writes**

Add `RuntimeError::InvalidLibraryLockState`, map it to `invalid_library_lock_state`, treat `None` as unlocked, and accept only `locked | unlocked`.

- [x] **Step 4: Gate Tauri startup**

In `lib.rs`, evaluate `library_is_locked(&secrets)` before initialization:

```rust
match infrastructure::runtime::library_is_locked(&secrets) {
    Ok(false) => {
        let runtime = initialize_application_library(...)?;
        app.manage(runtime);
    }
    Ok(true) | Err(_) => {}
}
```

Credential errors deliberately skip opening the library; the status command in Task 2 will render a recoverable error instead of exposing data.

- [x] **Step 5: Run focused tests**

Run the focused Rust test again and expect PASS.

---

### Task 2: Add typed access, lock, and unlock commands ✅

**Files:**
- Create: `src-tauri/src/commands/access.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/modules/auth_sync.rs`
- Modify: `src/shared/api/bindings.ts` through binding generation

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAccessStatus {
    pub locked: bool,
    pub trusted_windows_account: bool,
}

library_access_status() -> AppResult<LibraryAccessStatus>
library_lock(app, capture_lan) -> AppResult<LibraryAccessStatus>
library_unlock(app) -> AppResult<LibraryAccessStatus>
```

- Consumes: marker API from Task 1, `CaptureLanManager::stop`, and `AppHandle::restart`.

- [x] **Step 1: Write failing pure command tests**

Test `access_status_for`, `lock_for`, and `unlock_for` with an in-memory store. Assert the stable DTO values and an `AppResult::Failure` for a store read/write failure.

- [x] **Step 2: Make cloud disconnect local-first**

Change `AuthSyncManager::disconnect` so it attempts remote revoke, always clears the local refresh token and in-memory session, and returns signed-out status even when revoke returns a retryable network error. Secret-store failure remains fatal.

- [x] **Step 3: Implement commands and delayed restart**

Each mutation calls the pure helper first. `library_lock` stops LAN capture before writing the marker. After success, spawn a 180 ms delayed task and call `AppHandle::restart()`, allowing Vue to receive the success DTO and show a restarting state.

- [x] **Step 4: Register commands and generate bindings**

Add all three commands to `bindings.rs`, then run:

```powershell
corepack pnpm bindings:generate
```

Expected: generated TypeScript exposes `libraryAccessStatus`, `libraryLock`, `libraryUnlock`, and `LibraryAccessStatus`.

- [x] **Step 5: Run command and binding contract tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
corepack pnpm bindings:check
```

Expected: PASS.

---

### Task 3: Gate the Vue shell behind a dedicated lock screen ✅

**Files:**
- Create: `src/app/LibraryAccessScreen.vue`
- Create: `src/app/LibraryAccessScreen.test.ts`
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: generated access commands and `LibraryAccessStatus`.
- Produces: phases `checking | locked | error | unlocking | restarting` and events `unlock | retry`.

- [x] **Step 1: Write failing screen and app-boundary tests**

Assert the screen:

```ts
expect(screen.getByRole('heading', { name: '本地资料库已锁定' })).toBeVisible()
await user.click(screen.getByRole('button', { name: '使用当前 Windows 账户解锁' }))
expect(emit).toHaveBeenCalledWith('unlock')
```

In `App.profile.test.ts`, return `{ locked: true, trustedWindowsAccount: true }`, assert `profileList` was not called and main navigation is absent, then click unlock and assert `libraryUnlock` is called.

- [x] **Step 2: Implement the standalone access screen**

Render honest checking, locked, credential-error, and restarting copy. The locked state explains that SQLCipher was not opened and that unlock uses the current Windows account. Add one bounded seal entrance animation and reduced-motion overrides.

- [x] **Step 3: Refactor App startup**

Replace direct `onMounted` initialization with:

```ts
const libraryPhase = ref(desktopRuntime ? 'checking' : 'unlocked')

async function loadLibraryAccess() {
  const result = normalizeAppResult(await commands.libraryAccessStatus())
  if (!result.ok) { libraryPhase.value = 'error'; return }
  if (result.data.locked) { libraryPhase.value = 'locked'; return }
  libraryPhase.value = 'unlocked'
  await initializeWorkspace()
}
```

Render `LibraryAccessScreen` unless the phase is `unlocked`; only then render `AppShell`. Unlock success stays in `restarting` while the Rust command restarts the process.

- [x] **Step 4: Run focused Vue tests**

Run:

```powershell
corepack pnpm exec vitest run src/app/LibraryAccessScreen.test.ts src/app/App.profile.test.ts src/app/App.test.ts
```

Expected: PASS.

---

### Task 4: Expose polished lock and sign-out actions in Settings ✅

**Files:**
- Create: `src/app/LibraryLockDialog.vue`
- Create: `src/app/LibraryLockDialog.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `libraryLock`, existing `authDisconnect`, and dialog mode `lock | sign-out`.
- Produces: explicit “立即锁定” and “退出云端并锁定” flows.

- [x] **Step 1: Write failing dialog tests**

Assert dialog semantics, initial cancel focus, Escape cancellation, focus containment, mode-specific copy, and confirm events.

- [x] **Step 2: Implement the lock dialog**

Explain that locking stops phone capture, closes the app, and leaves encrypted data untouched. Sign-out mode additionally explains that cloud credentials are cleared first. Use a 240 ms opacity/transform entrance and no motion under reduced-motion.

- [x] **Step 3: Integrate Settings actions**

Replace the unfinished encryption copy with the real state. Add an “立即锁定” control to the encryption card. Rename the connected cloud action to “退出云端并锁定”; confirmation first calls `authDisconnect`, then `libraryLock`. On failure, keep the dialog open with a stable local message and do not claim the library is locked.

- [x] **Step 4: Run focused settings tests**

Run:

```powershell
corepack pnpm exec vitest run src/app/LibraryLockDialog.test.ts src/app/views/SettingsView.test.ts
```

Expected: PASS.

---

### Task 5: Architecture, quality gate, and local baseline

**Files:**
- Modify: `docs/architecture.md`
- Modify: `src-tauri/tests/runtime_state.rs`
- Modify: this plan.

**Interfaces:**
- Consumes: complete lock lifecycle.
- Produces: documented invariant and clean local Git commit.

- [x] **Step 1: Document the lock lifecycle**

Add that the lock marker is read before SQLCipher opens, locked startup omits `LibraryRuntime`, forged data commands fail for missing state, and current Windows credential access is the offline unlock boundary.

- [x] **Step 2: Run full repository checks**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
corepack pnpm bindings:check
git diff --check
```

Expected: every command exits 0 and initial JavaScript remains below 300 KB gzip.

- [ ] **Step 3: Verify the locked startup**

In automated tests prove no profile command runs while locked. In a disposable Windows run, lock the library, confirm the process restarts to the lock screen, unlock, and confirm the same profiles and encrypted assets return.

- [x] Vue boundary tests prove locked/checking/error/restarting states never mount `AppShell` or request profiles.
- [x] A disposable Windows integration test creates a profile-scoped problem with encrypted question and answer assets, starts through the real startup access boundary while locked without constructing `LibraryRuntime`, unlocks, and proves the same account, profile, problem ID, and asset counts return.
- [x] The native Tauri application opens the existing Windows library and shows the expected profile and two active problems before lock.
- [ ] Final manual process check: click “立即锁定”, observe the native process restart into the lock screen, unlock with the current Windows account, and confirm the same profile and two active problems. This security-setting action is deliberately not automated.

- [x] **Step 4: Commit**

```powershell
git add src-tauri/src/infrastructure/runtime.rs src-tauri/src/lib.rs src-tauri/src/commands/access.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/modules/auth_sync.rs src/shared/api/bindings.ts src/app/LibraryAccessScreen.vue src/app/LibraryAccessScreen.test.ts src/app/LibraryLockDialog.vue src/app/LibraryLockDialog.test.ts src/app/App.vue src/app/App.profile.test.ts src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts docs/architecture.md docs/superpowers/plans/2026-07-23-local-library-lock.md
git commit -m "feat: lock local library after sign-out"
```

---

## Self-Review

- Spec coverage: persistent marker, fail-closed startup, absent database state, LAN shutdown, local-first sign-out, trusted Windows unlock, Vue gating, settings UX, reduced motion, documentation, and restart verification all have explicit implementation and evidence.
- Placeholder scan: no `TBD`, `TODO`, unspecified error handling, or deferred test remains.
- Type consistency: `LibraryAccessStatus`, `libraryAccessStatus`, `libraryLock`, `libraryUnlock`, and every phase name match across Rust, generated bindings, Vue, and tests.
