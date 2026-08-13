# Library Recovery Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move reconnect, recovery-backup restore, fresh-start, dialog state, error reporting, and recovery single-flight ownership out of `App.vue` into one focused application controller.

**Architecture:** `useLibraryAccessLifecycle` remains the access-state machine for checking, locked, unlocked, recovery, and restarting phases. A new `useLibraryRecoveryController` consumes already-normalized operations and owns only user-triggered recovery workflows; `App.vue` adapts generated Tauri commands and binds returned readonly state/actions to the existing screens and dialogs.

**Tech Stack:** Vue 3.5 Composition API, TypeScript 5.9, Vitest 4, generated Tauri command bindings.

## Global Constraints

- Preserve the current fresh-start behavior, including clearing stale messages on open, showing backend rejection inside the active dialog, and keeping the dialog open after failure.
- Preserve the generic exception copy exactly: `恢复操作没有完成，原资料库状态没有被覆盖，请稍后重试。`
- Preserve recovery single-flight across different actions; a competing recovery action returns the active promise and does not invoke its operation.
- Successful reconnect, restore, and fresh-start call `enterRestarting`; failed or cancelled operations never do.
- `App.vue` remains the generated-command adapter and must not retain recovery workflow branching after this plan.
- Do not modify the existing Windows installer, storage-location, or fresh-start Rust changes already present in the dirty worktree.

---

### Task 1: Recovery controller behavior contract

**Files:**
- Create: `src/app/composables/useLibraryRecoveryController.test.ts`

**Interfaces:**
- Consumes: the controller interface defined in Task 2.
- Produces: tests for dialog state, stale-message clearing, backend failures, generic exceptions, single-flight, candidate lifecycle, and restarting transitions.

- [x] **Step 1: Create default operations and candidate fixtures**

```ts
const candidate: BackupRestoreCandidate = {
  id: 'candidate-1',
  summary: {
    formatVersion: 1,
    createdAtUtcMs: 1,
    assetCount: 2,
    encryptedBytes: 3,
    label: '恢复备份',
    readyForRestore: true,
  },
  expiresAtUtcMs: 100,
}

function createOptions(overrides = {}): LibraryRecoveryControllerOptions {
  return {
    reconnect: vi.fn().mockResolvedValue(success(true)),
    prepareRestore: vi.fn().mockResolvedValue(success(candidate)),
    restore: vi.fn().mockResolvedValue(success(true)),
    startFresh: vi.fn().mockResolvedValue(success(true)),
    enterRestarting: vi.fn(),
    ...overrides,
  }
}
```

- [x] **Step 2: Test open/close state and backend rejection**

Open fresh-start, assert `freshStartDialogOpen.value === true` and stale `message` is cleared. Return `failure('LIBRARY_CHANGED', '资料库状态已经变化；没有删除任何凭据。', false, 'changed')` from `startFresh`; assert the dialog remains open, the exact backend message is exposed, `busy` returns false, and `enterRestarting` is not called.

- [x] **Step 3: Test successful candidate and restart lifecycles**

Assert prepare stores the candidate and opens restore; confirmation calls `restore('candidate-1')`, closes restore, and enters restarting. Assert reconnect success enters restarting. Assert fresh-start success closes its dialog and enters restarting.

- [x] **Step 4: Test exception safety and cross-action single-flight**

Use a deferred reconnect operation, invoke reconnect and prepare concurrently, and assert both calls return the same promise while `prepareRestore` is untouched. Reject an operation and assert the exact generic copy, `busy === false`, and a later operation can run.

- [x] **Step 5: Run the new test red**

Run:

```powershell
pnpm vitest run src/app/composables/useLibraryRecoveryController.test.ts
```

Expected: FAIL because `useLibraryRecoveryController` does not exist.

### Task 2: Focused recovery workflow controller

**Files:**
- Create: `src/app/composables/useLibraryRecoveryController.ts`
- Retain: `src/app/recovery-single-flight.ts`

**Interfaces:**
- Consumes:

```ts
export interface LibraryRecoveryControllerOptions {
  reconnect: () => Promise<AppResult<boolean>>
  prepareRestore: () => Promise<AppResult<BackupRestoreCandidate | null>>
  restore: (candidateId: string) => Promise<AppResult<boolean>>
  startFresh: (confirmation: string) => Promise<AppResult<boolean>>
  enterRestarting: () => void
}
```

- Produces:

```ts
export interface LibraryRecoveryController {
  busy: Readonly<Ref<boolean>>
  message: Readonly<Ref<string>>
  candidate: Readonly<Ref<BackupRestoreCandidate | undefined>>
  restoreDialogOpen: Readonly<Ref<boolean>>
  freshStartDialogOpen: Readonly<Ref<boolean>>
  openFreshStartDialog: () => void
  closeFreshStartDialog: () => void
  closeRestoreDialog: () => void
  reconnectLibrary: () => Promise<boolean>
  prepareRecoveryBackup: () => Promise<boolean>
  confirmRecoveryBackup: () => Promise<boolean>
  confirmFreshStart: (confirmation: string) => Promise<boolean>
}
```

- [x] **Step 1: Own readonly state and one shared recovery runner**

Create refs for `busy`, `message`, `candidate`, and both dialog flags. Wrap every async action with `createRecoverySingleFlight()` and this exact runner:

```ts
function runRecovery(operation: () => Promise<boolean>): Promise<boolean> {
  return runSingleFlight(async () => {
    busy.value = true
    message.value = ''
    try {
      return await operation()
    }
    catch {
      message.value = '恢复操作没有完成，原资料库状态没有被覆盖，请稍后重试。'
      return false
    }
    finally {
      busy.value = false
    }
  })
}
```

- [x] **Step 2: Implement reconnect and prepare workflows**

Reconnect exposes `result.error.userMessage` on failure and calls `enterRestarting` only when `result.data` is true. Prepare exposes backend failure, returns false for `null`, and stores/opens only a real candidate.

- [x] **Step 3: Implement restore and fresh-start workflows**

Confirmation without a candidate returns `Promise.resolve(false)` without entering busy state. Successful restore closes the restore dialog and enters restarting. Fresh-start opens through a method that clears stale messages; failure keeps the dialog open, success closes it and enters restarting.

- [x] **Step 4: Return readonly state and explicit close methods**

Use Vue `readonly` for every exposed ref. Close methods only change their own dialog flag and never clear backend errors implicitly; the next open or operation clears the message according to the existing behavior.

- [x] **Step 5: Run controller and single-flight tests green**

Run:

```powershell
pnpm vitest run src/app/composables/useLibraryRecoveryController.test.ts src/app/recovery-single-flight.test.ts
```

Expected: PASS.

### Task 3: Compose recovery operations in App.vue

**Files:**
- Modify: `src/app/App.vue`
- Test: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: `useLibraryRecoveryController` from Task 2 and existing generated commands.
- Produces: unchanged template behavior with no recovery workflow implementation in `App.vue`.

- [x] **Step 1: Adapt generated commands to normalized operations**

Create the controller after `enterRestarting` is available:

```ts
const libraryRecovery = useLibraryRecoveryController({
  reconnect: async () => {
    const invocation = await commands.storageReconnectSelect()
    if (invocation.status === 'error') throw new Error('reconnect command rejected')
    return normalizeAppResult(invocation.data)
  },
  prepareRestore: async () => {
    const invocation = await commands.backupRecoveryPrepare()
    if (invocation.status === 'error') throw new Error('backup recovery command rejected')
    return normalizeAppResult(invocation.data)
  },
  restore: async (candidateId) => {
    const invocation = await commands.backupRecoveryRestore(candidateId)
    if (invocation.status === 'error') throw new Error('backup recovery restore rejected')
    return normalizeAppResult(invocation.data)
  },
  startFresh: async confirmation => normalizeAppResult(
    await commands.libraryRecoveryStartFresh(confirmation),
  ),
  enterRestarting,
})
```

- [x] **Step 2: Destructure template bindings from the controller**

```ts
const {
  busy: libraryRecoveryBusy,
  message: libraryRecoveryMessage,
  candidate: recoveryCandidate,
  restoreDialogOpen: recoveryRestoreDialogOpen,
  freshStartDialogOpen,
  openFreshStartDialog,
  closeFreshStartDialog,
  closeRestoreDialog,
  reconnectLibrary,
  prepareRecoveryBackup,
  confirmRecoveryBackup,
  confirmFreshStart,
} = libraryRecovery
```

- [x] **Step 3: Delete App-owned recovery state and functions**

Remove `createRecoverySingleFlight`, all five recovery refs, `runSingleLibraryRecovery`, and the functions `runLibraryRecovery`, `reconnectLibrary`, `prepareRecoveryBackup`, `confirmRecoveryBackup`, and `confirmFreshStart` from `App.vue`.

- [x] **Step 4: Bind explicit close actions in the template**

Replace direct readonly-ref assignments with:

```vue
@cancel="closeRestoreDialog"
@cancel="closeFreshStartDialog"
```

Keep `:message="libraryRecoveryMessage || libraryAccessError"` on the access screen and `:message="libraryRecoveryMessage"` on the fresh-start dialog.

- [x] **Step 5: Add one App integration test for backend fresh-start rejection**

Mock `libraryAccessStatus` as `recovery_required`, mock `libraryRecoveryStartFresh` with an `AppResult` failure, open the fresh-start dialog, type `永久放弃原资料库`, confirm, and assert the dialog remains visible with the backend message and no workspace command starts.

- [x] **Step 6: Run App recovery integration tests**

Run:

```powershell
pnpm vitest run src/app/App.profile.test.ts src/app/LibraryFreshStartDialog.test.ts src/app/composables/useLibraryRecoveryController.test.ts
```

Expected: PASS.

### Task 4: Recovery ownership contract and full verification

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

**Interfaces:**
- Consumes: the extracted recovery controller.
- Produces: regression protection against workflow returning to `App.vue`.

- [x] **Step 1: Add source ownership assertions**

Assert `App.vue` contains `useLibraryRecoveryController` and does not contain:

```ts
for (const token of [
  'createRecoverySingleFlight',
  'function runLibraryRecovery',
  'function reconnectLibrary',
  'function prepareRecoveryBackup',
  'function confirmRecoveryBackup',
  'function confirmFreshStart',
]) {
  expect(appSource).not.toContain(token)
}
```

Assert the controller contains all five operation names, `createRecoverySingleFlight`, the generic exception copy, and `readonly` state exposure.

- [x] **Step 2: Document recovery workflow ownership**

Add:

```markdown
- `useLibraryAccessLifecycle` owns access phases and initialization admission. `useLibraryRecoveryController` owns reconnect, recovery restore, fresh-start, recovery dialog state, and cross-action single-flight; `App.vue` only adapts generated commands.
```

- [x] **Step 3: Run complete verification**

Run:

```powershell
pnpm contract:architecture
pnpm typecheck
pnpm lint
pnpm test
git diff --check
```

Expected: all commands PASS.

- [x] **Step 4: Prepare the profile/workspace orchestration plan**

The next plan must move profile preview projection, create/rename/delete/select policies, workspace transition guarding, dashboard refresh, and profile-sync scheduling out of `App.vue`, while keeping command adaptation and provider wiring at the root.
