# Capture LAN Session Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Windows phone-capture LAN lifecycle orchestration out of `CaptureView.vue` and prevent late status responses from restoring a session after the user has stopped it or left the page.

**Architecture:** A Vue composable owns LAN addresses, preflight state, the active session, request coalescing, polling, and lifecycle invalidation. `CaptureView.vue` supplies normalized command adapters plus the current batch/busy callbacks; the existing dialog props, generated bindings, permission flow, and user-facing copy remain unchanged.

**Tech Stack:** TypeScript, Vue 3 refs, generated Tauri command bindings, Vitest.

## Global Constraints

- Preserve every pre-existing worktree change.
- Do not edit recognition/OCR, Rust, generated bindings, installer/release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Keep `CaptureWorkspace` and `CaptureLanDialog` props/events unchanged.
- Preserve the one-time firewall repair flow and all existing Chinese error copy.
- Coalesce concurrent preflight and status reads.
- A status response started before `stop()` or `dispose()` must never restore a stale session.
- Do not stop an active backend service merely because the Vue route unmounts; preserve existing service lifetime.
- Do not stage or commit.

---

### Task 1: Prove the LAN lifecycle controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureLanSession.ts`
- Create: `src/modules/capture/composables/useCaptureLanSession.test.ts`

**Interfaces:**

```ts
export interface CaptureLanOperations {
  addresses: () => Promise<AppResult<CaptureLanAddress[]>>
  preflight: () => Promise<AppResult<CaptureLanPreflight>>
  repair: () => Promise<AppResult<CaptureLanPreflight>>
  status: () => Promise<AppResult<CaptureLanSession | null>>
  start: (input: CaptureLanStartInput) => Promise<AppResult<CaptureLanSession>>
  stop: () => Promise<AppResult<boolean>>
}

export function useCaptureLanSession(options: {
  desktopAvailable: boolean
  activeBatchId: () => string | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onError: (message: string) => void
  operations: CaptureLanOperations
}): {
  addresses: Ref<CaptureLanAddress[]>
  preflight: Ref<CaptureLanPreflight | undefined>
  preflightBusy: Ref<boolean>
  session: Ref<CaptureLanSession | undefined>
  loadAddresses: () => Promise<CaptureLanAddress[]>
  loadPreflight: () => Promise<CaptureLanPreflight | undefined>
  loadStatus: () => Promise<void>
  start: (selectedAddress: string | null) => Promise<void>
  stop: (silent?: boolean) => Promise<void>
  startPolling: (intervalMs?: number) => void
  dispose: () => void
}
```

- [x] **Step 1: Write failing controller tests**

Add tests that:

```ts
it('coalesces concurrent preflight reads', async () => {
  const first = controller.loadPreflight()
  const second = controller.loadPreflight()
  expect(operations.preflight).toHaveBeenCalledOnce()
  resolvePreflight(success(readyRule))
  await expect(Promise.all([first, second])).resolves.toEqual([readyRule, readyRule])
})

it('repairs once and starts with a validated address', async () => {
  operations.preflight.mockResolvedValue(success(missingRule))
  operations.repair.mockResolvedValue(success(readyRule))
  await controller.start('192.168.1.20')
  expect(operations.start).toHaveBeenCalledWith({
    batchId: 'batch-1',
    selectedAddress: '192.168.1.20',
  })
})

it('does not restore a stopped session from an older status response', async () => {
  await controller.loadStatus()
  const stalePoll = controller.loadStatus()
  await controller.stop()
  resolveStaleStatus(success(session))
  await stalePoll
  expect(controller.session.value).toBeUndefined()
})
```

Also prove that status requests coalesce, permission cancellation does not start a session and can be retried, and late work after `dispose()` does not mutate refs or surface errors.

- [x] **Step 2: Run the new test file and verify failure**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureLanSession.test.ts
```

Expected: FAIL because `useCaptureLanSession.ts` does not exist.

- [x] **Step 3: Implement request coalescing and lifecycle invalidation**

Use:

```ts
let disposed = false
let sessionEpoch = 0
let preflightRequest: Promise<AppResult<CaptureLanPreflight>> | undefined
let statusRequest:
  | { epoch: number, promise: Promise<AppResult<CaptureLanSession | null>> }
  | undefined
let pollTimer: ReturnType<typeof setInterval> | undefined
```

`start()` and `stop()` increment `sessionEpoch` before invoking mutating commands.
The previous `statusRequest` remains the coalescing anchor until it settles so a slow
request cannot overlap a new poll, while `loadStatus()` applies its response only when
the request epoch still equals `sessionEpoch`. `dispose()` increments the epoch, clears
polling, and prevents every later ref/error callback.

- [x] **Step 4: Run controller tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureLanSession.test.ts
```

Expected: all LAN controller tests PASS.

### Task 2: Integrate the controller into CaptureView

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] **Step 1: Add a route-level stale-poll regression**

Expose a `stop mobile` action in the existing workspace stub. Start with a visible session, hold a later `captureLanStatus()` response, stop the service successfully, release the old status response, and assert the session remains `none`.

- [x] **Step 2: Add normalized command adapters**

Instantiate the composable with:

```ts
const captureLan = useCaptureLanSession({
  desktopAvailable,
  activeBatchId: () => detail.value?.batch.id,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onError: showError,
  operations: {
    addresses: async () => normalizeAppResult(await commands.captureLanAddresses()),
    preflight: async () => normalizeAppResult(await commands.captureLanPreflight()),
    repair: async () => normalizeAppResult(await commands.captureLanFirewallRepair()),
    status: async () => normalizeAppResult(await commands.captureLanStatus()),
    start: async input => normalizeAppResult(await commands.captureLanStart(input)),
    stop: async () => normalizeAppResult(await commands.captureLanStop()),
  },
})
```

Alias `addresses`, `preflight`, `preflightBusy`, and `session` for the existing template. Replace the inline LAN functions with controller methods. On mount call the three load methods and `startPolling(5_000)`; on unmount call `dispose()`.

- [x] **Step 3: Run focused integration tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureLanSession.test.ts src/app/views/CaptureView.test.ts src/modules/capture/components/CaptureLanDialog.test.ts
```

Expected: all focused tests PASS and existing firewall repair/cancellation behavior remains unchanged.

### Task 3: Verify and review

- [x] **Step 1: Run frontend gates**

```powershell
npm run lint
npm run typecheck
npm run test:coverage
npm run build
```

- [x] **Step 2: Review the scoped diff**

Confirm the generated commands and dialog contract are unchanged, stale status cannot win over stop/dispose, polling is non-overlapping, all existing Chinese copy remains, and no recognition/OCR or Rust source was changed.

- [x] **Step 3: Mark this plan complete**

Do not stage or commit.
