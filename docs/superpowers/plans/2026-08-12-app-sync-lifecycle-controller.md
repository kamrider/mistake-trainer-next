# App Sync Lifecycle Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Remove cloud-session restoration, sync phase transitions, browser event triggers, cooldown, and sync command policy from `App.vue` into one focused application controller.

**Architecture:** `App.vue` remains the composition root and adapts generated Tauri commands into `AppResult` operations. A new `useApplicationSyncLifecycle` controller owns synchronization workflow state and browser triggers, while the existing `createSyncController` continues to own single-flight and mutation debounce mechanics.

**Tech Stack:** Vue 3.5 Composition API, TypeScript 5.9, Vitest 4, generated Tauri bindings.

## Global Constraints

- Preserve the fresh-start recovery changes currently present in `src/app/App.vue` byte-for-byte outside import adjacency.
- Preserve every sync phase, user-facing status string, retry classification, 15-second visibility cooldown, and local-first behavior.
- `App.vue` may adapt generated commands but must not own sync workflow branching after this plan.
- The new controller may depend on `src/shared` contracts and `src/app/sync-controller`; it must not import feature modules or Vue components.
- Existing architecture and feature public-entry contracts remain green.

---

### Task 1: Sync lifecycle behavior tests

**Files:**
- Create: `src/app/composables/useApplicationSyncLifecycle.test.ts`

**Interfaces:**
- Consumes: the controller interface defined in Task 2.
- Produces: tests for offline/local-only/session states, command failures, single-flight restore, successful refresh, capture deferral, browser triggers, cooldown, and disposal.

- [x] **Step 1: Write a mounted harness with injected operations**

Use a minimal component that calls the composable and exposes its returned values:

```ts
const Harness = defineComponent({
  setup() {
    lifecycle = useApplicationSyncLifecycle(options)
    lifecycle.start()
    onBeforeUnmount(lifecycle.dispose)
    return { phase: lifecycle.phase }
  },
  template: '<output>{{ phase }}</output>',
})
```

The default options use `ref('unlocked')`, `ref(true)`, `ref<AppPage>('dashboard')`, successful connected restore, successful empty sync report, and `vi.fn()` for `onSyncSuccess`.

- [x] **Step 2: Test restore-state mapping and sync execution**

Assert:

```ts
expect(phase.value).toBe('synced')
expect(syncNow).toHaveBeenCalledOnce()
expect(onSyncSuccess).toHaveBeenCalledWith(report, 'startup')
```

Table-test `offline -> offline`, `unconfigured -> local_only`, and `signed_out | verification_required -> signed_out`, with no `syncNow` call.

- [x] **Step 3: Test error and admission semantics**

Assert `AUTH_NETWORK`, `AUTH_TIMEOUT`, `cloud_network`, `cloud_timeout`, and `cloud_unavailable` map to `offline`; other restore failures map to `retry_waiting`. Assert `SYNC_CAPTURE_ACTIVE` maps to `deferred_capture`, `SYNC_ALREADY_RUNNING` stays `syncing`, command exceptions map to `offline`, and returned failures remain unchanged.

- [x] **Step 4: Test browser triggers, single-flight, cooldown, and disposal**

Use fake timers and a `now` option. Verify concurrent startup/online restore calls share one restore operation, an online event after settlement starts a new pass, a visible event inside 15 seconds does nothing, a later visible event restores again, locked/uninitialized state ignores events, and `dispose()` removes listeners and cancels scheduled mutation work.

- [x] **Step 5: Run the test red**

Run:

```powershell
pnpm vitest run src/app/composables/useApplicationSyncLifecycle.test.ts
```

Expected: FAIL because `useApplicationSyncLifecycle` does not exist.

### Task 2: Application sync lifecycle controller

**Files:**
- Create: `src/app/composables/useApplicationSyncLifecycle.ts`

**Interfaces:**
- Consumes:

```ts
interface ApplicationSyncLifecycleOptions {
  desktopRuntime: boolean
  libraryAccessPhase: Readonly<Ref<LibraryAccessPhase>>
  workspaceInitialized: Readonly<Ref<boolean>>
  activePage: Readonly<Ref<AppPage>>
  restoreSession: () => Promise<AppResult<CloudAuthState>>
  syncNow: () => Promise<AppResult<SyncNowReport>>
  onSyncSuccess: (report: SyncNowReport, reason: SyncTrigger) => Promise<void>
  now?: () => number
}
```

- Produces:

```ts
interface ApplicationSyncLifecycle {
  phase: Readonly<Ref<SyncPhase>>
  controller: SyncController
  restoreCloudAndSync: (reason: SyncTrigger) => Promise<void>
  start: () => void
  dispose: () => void
}
```

- [x] **Step 1: Implement phase state and failure classification**

Use `readonly(ref<SyncPhase>('local_only'))` and the exact network code set:

```ts
const networkFailureCodes = new Set([
  'AUTH_NETWORK',
  'AUTH_TIMEOUT',
  'cloud_network',
  'cloud_timeout',
  'cloud_unavailable',
])
```

- [x] **Step 2: Move perform-sync workflow into the controller**

Set `syncing`, await `options.syncNow()`, preserve `SYNC_CAPTURE_ACTIVE` and `SYNC_ALREADY_RUNNING` handling, set `synced` on success, record `lastSuccessfulSyncAtUtcMs = now()`, and await `options.onSyncSuccess(report, reason)`. Exceptions return:

```ts
failure(
  'SYNC_REQUEST_FAILED',
  '暂时无法连接云端，本地内容已经保存并会等待重试。',
  true,
  'sync-request-failed',
)
```

- [x] **Step 3: Move restore single-flight and session-state workflow**

Keep one `restoreTask`. Browser preview sets `local_only`; offline browser state sets `offline`; visible triggers inside `15_000` milliseconds return early. Connected restore sets `idle` then awaits `controller.run(reason)`; the other auth states map exactly as tested.

- [x] **Step 4: Own browser event registration**

`start()` idempotently registers `online` and `visibilitychange`. Both handlers require `libraryAccessPhase.value === 'unlocked'` and `workspaceInitialized.value`; visibility additionally requires `document.visibilityState === 'visible'`. `dispose()` idempotently removes both listeners and disposes the sync controller.

- [x] **Step 5: Run controller tests green**

Run:

```powershell
pnpm vitest run src/app/composables/useApplicationSyncLifecycle.test.ts src/app/sync-controller.test.ts
```

Expected: PASS.

### Task 3: Reduce App.vue to composition

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts` only if a mock needs a stable operation adapter.

**Interfaces:**
- Consumes: `useApplicationSyncLifecycle` from Task 2.
- Produces: the same template bindings and injected `SyncController`, with sync workflow branches removed from `App.vue`.

- [x] **Step 1: Adapt generated commands at the composition root**

Construct the controller with:

```ts
const syncLifecycle = useApplicationSyncLifecycle({
  desktopRuntime,
  libraryAccessPhase,
  workspaceInitialized,
  activePage,
  restoreSession: async () => {
    const invocation = await commands.authRestore()
    if (invocation.status === 'error') {
      return failure(
        'AUTH_COMMAND_UNAVAILABLE',
        '云端会话暂时无法恢复，本地资料仍可使用。',
        true,
        'auth-command-unavailable',
      )
    }
    return normalizeAppResult(invocation.data)
  },
  syncNow: async () => {
    const invocation = await commands.syncNow()
    if (invocation.status === 'error') {
      return failure(
        'SYNC_COMMAND_UNAVAILABLE',
        '同步请求没有启动，本地内容已经保存。',
        true,
        'sync-command-unavailable',
      )
    }
    return normalizeAppResult(invocation.data)
  },
  onSyncSuccess: async (report, reason) => {
    await loadProfiles()
    if (report.pulledChangeCount > 0 && reason !== 'mutation' && activePage.value !== 'review') {
      profileEpoch.value += 1
    }
  },
})
```

Destructure `phase: syncPhase`, use `controller` for `provide`, and retain `syncStatusCopy(syncPhase.value)` for shell copy.

- [x] **Step 2: Replace lifecycle calls**

In `initializeWorkspace`, call `void syncLifecycle.restoreCloudAndSync('startup')`. In `onMounted`, call `syncLifecycle.start()`. In `onUnmounted`, call `syncLifecycle.dispose()`.

- [x] **Step 3: Delete sync workflow implementation from App.vue**

Remove `automaticSyncCooldownMs`, `lastSuccessfulSyncAtUtcMs`, `cloudRestoreTask`, `mutationSyncPhases`, `handleOnline`, `handleVisibilityChange`, `isNetworkFailure`, `restoreCloudAndSync`, `runCloudRestoreAndSync`, and `performSync`. Remove unused `SyncNowReport`, `SyncTrigger`, and `createSyncController` imports.

- [x] **Step 4: Run App integration tests**

Run:

```powershell
pnpm vitest run src/app/App.profile.test.ts src/app/App.test.ts src/app/sync-controller.test.ts src/app/composables/useApplicationSyncLifecycle.test.ts
```

Expected: PASS, including offline startup, capture deferral, restore single-flight, online retrigger, profile refresh, and locked-library tests.

### Task 4: Sync-controller boundary verification

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

**Interfaces:**
- Consumes: the extracted controller and reduced App root.
- Produces: regression evidence that workflow does not return to `App.vue`.

- [x] **Step 1: Add source ownership assertions**

Assert `App.vue` contains `useApplicationSyncLifecycle` and does not contain these tokens:

```ts
for (const token of [
  'function restoreCloudAndSync',
  'function runCloudRestoreAndSync',
  'function performSync',
  "window.addEventListener('online'",
  "document.addEventListener('visibilitychange'",
  'automaticSyncCooldownMs',
]) {
  expect(appSource).not.toContain(token)
}
```

Assert the controller contains `restoreCloudAndSync`, `createSyncController`, both event listeners, and `15_000`.

- [x] **Step 2: Document sync lifecycle ownership**

Add:

```markdown
- `App.vue` composes command adapters and providers. `useApplicationSyncLifecycle` owns cloud restore, sync phase transitions, browser triggers, cooldown, and success callbacks; `createSyncController` owns request single-flight and mutation debounce.
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

- [ ] **Step 4: Prepare the remaining App extraction plans**

Create separate plans for library access/recovery and profile/workspace orchestration. Each plan must remove workflow branching from `App.vue`, keep generated command adaptation at the root, preserve existing integration tests, and add source ownership assertions.
