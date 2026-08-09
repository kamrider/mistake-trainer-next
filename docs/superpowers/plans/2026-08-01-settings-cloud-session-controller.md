# Settings Cloud Session Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make settings authentication and local-library locking one race-safe workflow so stale restore/status responses cannot overwrite a newer login or sign-out result.

**Architecture:** Add a focused Vue composable that owns cloud-auth form state, authentication mutations, auth-load revision guards, and the lock/sign-out transaction. `SettingsView` remains the page coordinator: it supplies normalized Tauri adapters, global-sync and restart callbacks, and DOM focus restoration after the controller closes the dialog. Synchronization, backend selection, storage migration, updater, and account deletion remain outside this controller.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, generated Tauri bindings.

## Global Constraints

- Preserve all existing Chinese copy, command payloads, password clearing, first-sync scheduling, sign-out-before-lock ordering, dialog focus behavior, and restart-boundary behavior.
- A restore/status request may update auth state only if no newer authentication mutation has started since that request began.
- Admit only one authentication mutation and one lock transaction at a time.
- Disable settings refresh while authentication or locking is active; ordinary local settings remain usable.
- Do not modify sync semantics, backend selection, storage/device migration, updater recovery, account deletion, Rust, generated bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize the cloud-session transaction

**Files:**

- Create: `src/app/composables/useSettingsCloudSession.test.ts`

**Interfaces:**

- Consumes: generated `AuthCredentials`, `CloudAuthState`, and internal `AppResult<T>`.
- Produces: executable requirements for `useSettingsCloudSession(options)`.

- [x] **Step 1: Add typed fixtures and deferred-operation harness**

Create signed-out, verification-required, and connected `CloudAuthState` fixtures. Supply mocked `restore`, `status`, `signIn`, `signUp`, `disconnect`, and `lockLibrary` operations plus `onConnected` and `onRestarting` callbacks.

- [x] **Step 2: Add the stale-load regression**

Start `restoreSession()` with a deferred signed-out result, submit valid credentials, resolve sign-in as connected, then resolve the old restore response. Assert final auth remains connected, password is empty, and `onConnected` ran exactly once.

- [x] **Step 3: Add mutation and lock transaction coverage**

Assert duplicate submits are rejected while pending; sign-up uses the exact trimmed email/password and verification copy; sign-out calls `disconnect` before `lockLibrary`; a disconnect failure never invokes `lockLibrary`; a successful lock calls `onRestarting` and deliberately remains busy.

- [x] **Step 4: Run the new test red**

Run:

```powershell
npm test -- --run src/app/composables/useSettingsCloudSession.test.ts
```

Expected: import-resolution failure because `useSettingsCloudSession.ts` does not exist.

### Task 2: Implement the controller

**Files:**

- Create: `src/app/composables/useSettingsCloudSession.ts`
- Test: `src/app/composables/useSettingsCloudSession.test.ts`

**Interfaces:**

- Consumes operations:

```ts
restore?: () => Promise<AppResult<CloudAuthState>>
status?: () => Promise<AppResult<CloudAuthState>>
signIn: (credentials: AuthCredentials) => Promise<AppResult<CloudAuthState>>
signUp: (credentials: AuthCredentials) => Promise<AppResult<CloudAuthState>>
disconnect: () => Promise<AppResult<CloudAuthState>>
lockLibrary: () => Promise<AppResult<LibraryAccessStatus>>
```

- Produces refs `auth`, `email`, `password`, `mode`, `authBusy`, `authMessage`, `lockDialogOpen`, `lockDialogMode`, `lockingLibrary`, `lockErrorMessage`; actions `restoreSession`, `submit`, `disconnectCloud`, `openLibraryLock`, `closeLibraryLock`, and `confirmLibraryLock`.

- [x] **Step 1: Implement revision-guarded auth loading**

Capture an auth revision before restore/status. Apply its result only when that revision still matches; silently ignore optional load errors so the rest of Settings remains usable.

- [x] **Step 2: Implement single-admission auth mutations**

Increment the revision before each accepted submit/disconnect, clear only the password after successful submit, preserve exact product messages, and invoke `onConnected` best-effort only for a connected result.

- [x] **Step 3: Implement sign-out-and-lock ordering**

Keep the dialog retryable after disconnect/lock failure. On success invoke `onRestarting` and retain `lockingLibrary=true` because the native process is crossing the restart boundary.

- [x] **Step 4: Run controller tests green**

Run the Task 1 command and expect all controller cases to pass.

### Task 3: Integrate SettingsView without changing presentation contracts

**Files:**

- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Verify: `src/app/components/SettingsCloudAuthPanel.vue`
- Verify: `src/app/LibraryLockDialog.vue`

**Interfaces:**

- Consumes: the Task 2 controller and existing generated commands.
- Produces: unchanged `SettingsCloudAuthPanel` and `LibraryLockDialog` props/emits.

- [x] **Step 1: Add the pending-auth refresh regression**

Defer `authSignIn`, submit valid credentials, and assert the top-level `刷新状态` button is disabled until the request resolves. Resolve connected success and assert the button re-enables, password is cleared, and the connected actions appear.

- [x] **Step 2: Replace view-local auth and lock state/functions**

Normalize all typed command results in the page adapters. Replace `load()`'s inline auth restore/status code with `restoreSession()`. Keep sync-now and backend-selection orchestration in `SettingsView`.

- [x] **Step 3: Preserve focus restoration in the page**

After `closeLibraryLock()` returns the closed mode, wait for Vue DOM removal and restore focus to the sign-out or local-lock trigger exactly as before.

- [x] **Step 4: Run focused and adjacent tests**

Run:

```powershell
npm test -- --run src/app/composables/useSettingsCloudSession.test.ts src/app/views/SettingsView.test.ts src/app/components/SettingsCloudAuthPanel.test.ts src/app/LibraryLockDialog.test.ts
```

Expected: all files pass with existing and new interaction semantics.

### Task 4: Commercial-quality gates and local review

**Files:**

- Verify: all files in Tasks 1–3
- Modify: `docs/superpowers/plans/2026-08-01-settings-cloud-session-controller.md`

- [x] **Step 1: Run static and full regression gates**

Run `npm run lint`, `npm run typecheck`, then `npm test -- --run --maxWorkers=1`. Expect zero lint warnings, no TypeScript errors, and the complete frontend suite to pass.

- [x] **Step 2: Review identity and lifecycle safety**

Confirm stale loads cannot overwrite mutations, duplicate requests are rejected, sign-out precedes lock, lock failure stays retryable, success remains in the restart boundary, refresh disables during auth/lock, and no excluded subsystem changed.

- [x] **Step 3: Check workspace hygiene**

Run target/global whitespace checks, confirm the index is empty, confirm no generated artifacts appeared, and confirm `recognition_visual_split.rs` remains an untouched pre-existing modification.

- [x] **Step 4: Record evidence below**

Record baseline, red, controller green, focused integration, full gates, review, and hygiene results.

## Verification record

- Baseline: Settings view, cloud panel, and lock dialog passed, 3 files / 57 tests.
- Controller red: the new suite failed at import resolution because `useSettingsCloudSession.ts` did not exist.
- Interaction red: the pending-auth page test found `刷新` remained enabled while `authSignIn` was unresolved.
- Controller green: 1 file / 6 tests passed; TypeScript passed before page integration.
- Focused integration: controller, Settings view, cloud panel, and lock dialog passed, 4 files / 64 tests.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed; the production Vite build passed; serial full Vitest passed, 128 files / 707 tests.
- Review: `SettingsView.vue` decreased from the audited 1,082 lines to 1,033; auth/lock commands are confined to one adapter block; the 208-line controller owns auth form state, two-dimensional load/auth revisions, single-admission mutations, sign-out-before-lock ordering, retryable failure, and the retained restart boundary. The page owns only command adaptation, first-sync/restart callbacks, and post-dialog focus restoration.
- Preserved behavior: exact credentials and user copy, password clearing, connected first-sync scheduling, local lock, sign-out then lock, dialog focus return, backend selection, manual synchronization, storage/device migration, updater behavior, panel props/emits, and all generated command contracts remain unchanged.
- Hygiene: whitespace checks passed; nothing was staged or committed; the build produced only ignored `dist` output; no generated source artifact entered status; `recognition_visual_split.rs` remains an untouched pre-existing modification; excluded launch-gate work was not changed.
