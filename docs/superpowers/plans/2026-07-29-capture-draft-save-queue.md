# Capture Draft Save Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract capture draft autosave scheduling from the oversized route view and guarantee that a revision-conflict retry can never overwrite a newer queued edit.

**Architecture:** A framework-light queue controller owns per-draft coalescing, one-at-a-time execution, one revision-conflict retry, active-batch cleanup, and disposal. `CaptureView.vue` remains responsible for generated command invocation, current revision lookup, batch reload, and user-facing state/error copy. Newer queued content always wins over an older request awaiting conflict refresh.

**Tech Stack:** TypeScript, Vue 3 integration, Vitest.

## Global Constraints

- Preserve the generated `captureDraftUpdate` command and payload.
- Preserve latest-write coalescing while one save is in flight.
- Retry one revision conflict only; never loop indefinitely.
- Never reinsert an older conflicted update when the same draft already has newer queued content.
- Drop queued edits belonging to an inactive batch and clear everything on unmount.
- Ignore in-flight completions after leaving a batch, including a conflict refresh that finishes late.
- Preserve `saving`, `saved`, and `error` UI states and the existing unexpected-error copy.
- Do not edit recognition/OCR, Rust, generated bindings, or excluded pre-launch behavior.
- Do not stage or commit the dirty worktree.

---

### Task 1: Build and prove the draft save queue

**Files:**
- Create: `src/modules/capture/composables/useCaptureDraftSaveQueue.ts`
- Create: `src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts`

**Interfaces:**

```ts
export interface CaptureDraftSaveUpdate {
  batchId: string
  draftId: string
  subject: string
  tags: string[]
  note: string
}

export type CaptureDraftSaveOutcome =
  | { kind: 'saved' }
  | { kind: 'revision_conflict', message: string }
  | { kind: 'failed', message: string }

export interface CaptureDraftSaveQueue {
  enqueue: (update: CaptureDraftSaveUpdate) => void
  flush: () => Promise<void>
  retainBatch: (batchId: string) => void
  clear: () => void
  dispose: () => void
}

export function useCaptureDraftSaveQueue(options: {
  activeBatchId: () => string | undefined
  isBlocked: () => boolean
  perform: (update: CaptureDraftSaveUpdate) => Promise<CaptureDraftSaveOutcome>
  refresh: (batchId: string) => Promise<void>
  onSaving: () => void
  onSaved: () => void
  onFailed: (message: string) => void
  onBusyChange: (busy: boolean) => void
}): CaptureDraftSaveQueue
```

- [x] **Step 1: Write failing unit tests**

Prove:

```ts
it('keeps a newer edit queued while the previous save is in flight', async () => {
  queue.enqueue(update('first'))
  await started
  queue.enqueue(update('latest'))
  finishFirst({ kind: 'saved' })
  await waitForSecondSave()
  expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'latest' }))
})

it('does not let an older conflict retry overwrite a newer queued edit', async () => {
  perform.mockResolvedValueOnce({ kind: 'revision_conflict' })
  queue.enqueue(update('old'))
  await refreshStarted
  queue.enqueue(update('new'))
  finishRefresh()
  await waitForSecondSave()
  expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'new' }))
})
```

Also cover one conflict retry without a newer edit, failed outcome, blocked/resumed flush,
active-batch cleanup, late in-flight completion, leaving during conflict refresh, and disposal.

- [x] **Step 2: Run tests and verify they fail**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\composables\useCaptureDraftSaveQueue.test.ts
```

Expected: FAIL because the queue module does not exist.

- [x] **Step 3: Implement the queue**

Use a `Map<batchId:draftId, PendingUpdate>` and monotonically increasing `generation`. Remove an entry before running it. When an outcome is `revision_conflict`, refresh once and reinsert the old entry only when:

```ts
!pending.has(key) && entry.attempts === 0
```

If a newer generation is already pending, leave it untouched. After each run, release busy state and start another flush when eligible. `retainBatch` removes every other batch; `dispose` prevents future work and clears the map.

- [x] **Step 4: Run queue tests**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\composables\useCaptureDraftSaveQueue.test.ts
```

Expected: all queue tests PASS.

### Task 2: Integrate CaptureView without changing command behavior

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] **Step 1: Add a route-level regression for the conflict/newer-edit race**

Hold the batch reload promise after the first `capture_revision_conflict`, emit a second draft update while reload is pending, then release reload. Assert the second `captureDraftUpdate` call contains the newer note and the refreshed revision.

- [x] **Step 2: Replace inline queue state**

Remove `PendingDraftUpdate`, `pendingDraftUpdates`, `draftSaveRunning`, `draftUpdateKey`, and `flushDraftUpdates`. Create the queue with:

```ts
const draftSaveQueue = useCaptureDraftSaveQueue({
  activeBatchId: () => detail.value?.batch.id,
  isBlocked: () => busy.value,
  perform: persistDraftUpdate,
  refresh: loadDetail,
  onSaving: () => { saveState.value = 'saving' },
  onSaved: () => { saveState.value = 'saved' },
  onFailed: (message) => {
    saveState.value = 'error'
    showError(message)
  },
  onBusyChange: (saving) => { busy.value = saving },
})
```

`persistDraftUpdate` calls the same generated command with `detail.value.batch.revision` and returns the typed outcome. `updateDraft` only enqueues. The existing `watch(busy)` calls `draftSaveQueue.flush()`, the batch watch calls `retainBatch`, and unmount calls `dispose`.

- [x] **Step 3: Run focused tests**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\composables\useCaptureDraftSaveQueue.test.ts src\app\views\CaptureView.test.ts
```

Expected: all focused tests PASS.

### Task 3: Run gates and review

- [x] **Step 1: Run frontend gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test:coverage
pnpm build
```

- [x] **Step 2: Review**

Confirm the command/payload and visible save states are unchanged, the new race test proves newer content wins, only one retry is possible, inactive batches and unmount clear pending work, and no recognition/OCR source changed.

- [x] **Step 3: Mark the plan complete**

Do not stage or commit.
