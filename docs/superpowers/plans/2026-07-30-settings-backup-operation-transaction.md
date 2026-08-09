# Settings Backup Operation Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make backup creation, restore-package preparation, and restore startup one mutually exclusive settings transaction that cannot be duplicated or abandoned silently during navigation.

**Architecture:** Extract the backup workflow state from `SettingsView.vue` into `useSettingsBackupOperations`. The composable owns one phase, receipts, the currently validated restore candidate, stable error copy, and command-result semantics. `SettingsView` adapts Tauri commands into `AppResult` operations and includes the composable's busy state in its existing route, workspace, and `beforeunload` guard.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, Tauri-generated bindings.

## Global Constraints

- Do not modify Rust backup package, schema-validation, or restore repositories in this batch.
- Do not implement excluded launch work: licensing, privacy/legal copy, support operations, account deletion, device migration, update-failure recovery, or SLA.
- Preserve existing cancellation semantics: `ok(null)` is neutral and must not be reported as failure.
- A failed restore-package validation must invalidate the previously validated candidate.
- A failed restore start keeps the candidate available for an explicit retry.
- Successful restore startup remains busy until the application restarts.
- Do not stage or commit the shared dirty worktree.

---

### Task 1: Add a single backup-operation transaction

**Files:**
- Create: `src/app/composables/useSettingsBackupOperations.ts`
- Test: `src/app/composables/useSettingsBackupOperations.test.ts`

**Interfaces:**
- Consumes:

```ts
interface SettingsBackupOperations {
  create: () => Promise<AppResult<BackupSummary | null>>
  prepareRestore: () => Promise<AppResult<BackupRestoreCandidate | null>>
  restore: (candidateId: string) => Promise<AppResult<boolean>>
}
```

- Produces:

```ts
type SettingsBackupPhase = 'idle' | 'creating' | 'preparing' | 'restoring'

function useSettingsBackupOperations(operations: SettingsBackupOperations): {
  phase: Ref<SettingsBackupPhase>
  busy: ComputedRef<boolean>
  created: Ref<BackupSummary | undefined>
  candidate: Ref<BackupRestoreCandidate | undefined>
  message: Ref<string>
  createBackup(): Promise<boolean>
  prepareRestore(): Promise<boolean>
  restoreBackup(): Promise<boolean>
}
```

- [x] **Step 1: Write failing mutual-exclusion tests**

Create deferred command promises and assert:

```ts
const creating = controller.createBackup()
await controller.prepareRestore()
await controller.restoreBackup()
expect(operations.create).toHaveBeenCalledOnce()
expect(operations.prepareRestore).not.toHaveBeenCalled()
expect(operations.restore).not.toHaveBeenCalled()
```

Repeat for `preparing` and `restoring`, and call the same action twice before its first promise settles.

- [x] **Step 2: Write result-semantics tests**

Cover all of these exact cases:

```ts
success(null) // neutral cancellation, no error
failure(...) // userMessage is exposed
throw new Error('offline') // stable fallback copy
prepare failure // candidate becomes undefined
restore failure // candidate remains available and phase returns to idle
restore success // phase remains "restoring"
create failure // the last successful created receipt remains visible
```

- [x] **Step 3: Run the composable test and verify red**

Run:

```powershell
pnpm exec vitest run src/app/composables/useSettingsBackupOperations.test.ts
```

Expected: FAIL because `useSettingsBackupOperations.ts` does not exist.

- [x] **Step 4: Implement the transaction**

Use one synchronous phase gate before every `await`:

```ts
if (phase.value !== 'idle') return false
phase.value = 'creating'
```

For creation and preparation, always restore `phase` to `idle` in `finally`. For restore, restore it only on failure or thrown error; leave it as `restoring` after `success(true)` so the application restart remains the terminal transition.

- [x] **Step 5: Run the composable test and verify green**

Run the same focused command. Expected: all controller tests pass.

### Task 2: Integrate the transaction with settings navigation safety

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `useSettingsBackupOperations` from Task 1.
- Produces: no new public component API; existing `SettingsBackupPanel` props and events remain unchanged.

- [x] **Step 1: Write failing duplicate-event integration tests**

Render `SettingsView`, defer `backupCreate`, click “创建加密备份” twice before resolving, and assert:

```ts
expect(api.backupCreate).toHaveBeenCalledOnce()
expect(api.backupPrepareRestore).not.toHaveBeenCalled()
```

Do the symmetric test for restore-package preparation.

- [x] **Step 2: Write failing transition-guard integration test**

Render through `RouterView` with `workspaceTransitionGuardKey`. While `backupCreate` is pending:

```ts
await router.push({ name: 'dashboard' })
expect(router.currentRoute.value.name).toBe('settings')
await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
const event = new Event('beforeunload', { cancelable: true })
window.dispatchEvent(event)
expect(event.defaultPrevented).toBe(true)
```

Assert the page shows `备份操作正在完成，请等待完成后再离开设置。`. Resolve the command and verify route, workspace, and unload transitions become available.

- [x] **Step 3: Replace view-local backup refs and methods**

Adapt Tauri invocations at the view boundary:

```ts
create: async () => {
  const invocation = await commands.backupCreate()
  if (invocation.status === 'error') throw new Error('backup command rejected')
  return normalizeAppResult(invocation.data)
}
```

Use the same adapter shape for `backupPrepareRestore` and `backupRestore`. Bind the returned `created`, `candidate`, and phase-derived booleans to the existing panel and dialog.

- [x] **Step 4: Extend the existing unsaved-changes guard**

Define:

```ts
const backupOperationBusy = computed(() => backupBusy.value)
```

Include it in the existing `busy` callback. In `onBusy`, choose backup-specific copy when `backupOperationBusy.value` is true; otherwise preserve the existing preference-save copy. This reuses the already registered route, workspace, and `beforeunload` boundary instead of adding a second competing guard.

- [x] **Step 5: Preserve restore-dialog behavior**

On `restoreBackup()` failure, close the dialog and restore focus to “查看风险并确认恢复”; keep the candidate card visible. On success, leave the dialog in its existing busy “正在准备重启…” state.

- [x] **Step 6: Run affected settings tests**

Run:

```powershell
pnpm exec vitest run src/app/composables/useSettingsBackupOperations.test.ts src/app/components/SettingsBackupPanel.test.ts src/app/views/SettingsView.test.ts
```

Expected: all tests pass.

### Task 3: Verify and review

**Files:**
- Modify: `docs/superpowers/plans/2026-07-30-settings-backup-operation-transaction.md`

- [x] **Step 1: Run full frontend gates**

Run:

```powershell
pnpm test
pnpm typecheck
pnpm lint
```

Expected: all commands exit `0`.

- [x] **Step 2: Run worktree hygiene checks**

Run `git diff --check`, scan the files from this plan for trailing whitespace, scan this plan for unchecked boxes, and confirm `git diff --cached --name-only` is empty.

- [x] **Step 3: Perform local code review**

Review phase transitions, cancellation semantics, retained receipts, failed-candidate invalidation, late command settlement, navigation ownership, accessibility copy, and unchanged Tauri/Rust boundaries. Fix every Critical or Important issue and rerun affected plus full gates.

### Verification record

- [x] Focused controller tests: 1 file, 9 tests passed.
- [x] Affected settings tests: 3 files, 63 tests passed.
- [x] Full frontend tests: 98 files, 608 tests passed in an exclusive run.
- [x] Typecheck: `vue-tsc --noEmit` passed.
- [x] Lint: `eslint . --max-warnings 0` passed.
- [x] Diff/worktree checks: passed; Git index remains empty.
