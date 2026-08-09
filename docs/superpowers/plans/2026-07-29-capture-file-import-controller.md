# Capture File Import Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move browser-provided image import orchestration out of `CaptureView.vue`, enforce the batch’s remaining capacity before reading files, expose accessible progress, and prevent late completion from reopening a batch the user left.

**Architecture:** A Vue composable owns bounded two-worker file processing, upload IDs, source sequencing, remaining-capacity calculation, failure collection, and reactive progress. `CaptureView.vue` owns command adaptation, messages, and post-import refresh routing; it reloads batch detail only when that batch is still active, otherwise it refreshes the inbox list. `CaptureWorkspace.vue` renders a polite live progress region without changing import events.

**Tech Stack:** TypeScript, Vue 3 refs, browser `File` APIs, generated Tauri bindings, Vitest, Testing Library.

## Global Constraints

- Preserve every pre-existing worktree change.
- Do not edit recognition/OCR, Rust, generated bindings, installer/release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Keep `captureImportBytes` and `CaptureImportBytesInput` unchanged.
- Keep exactly two import workers and the exact batch cap of 150 images.
- Compute accepted files as `max(0, 150 - currentItemCount)`, not 150 per invocation.
- Preserve source order through `sourceSequence = currentItemCount + fileIndex`.
- Continue importing after individual read/command failures and preserve the existing failure/limit Chinese copy.
- A completed import may refresh detail only when its batch remains active; it must never navigate back to a batch the user left.
- Keep paste, drag/drop, and file-picker events unchanged.
- Do not stage or commit.

---

### Task 1: Build and prove the import controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureFileImport.ts`
- Create: `src/modules/capture/composables/useCaptureFileImport.test.ts`

**Interfaces:**

```ts
export interface CaptureFileImportProgress {
  completed: number
  total: number
  failed: number
}

export interface CaptureFileImportOutcome {
  batchId: string
  failedNames: string[]
  skippedCount: number
}

export function useCaptureFileImport(options: {
  activeBatchId: () => string | undefined
  currentItemCount: () => number
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  importBytes: (
    input: CaptureImportBytesInput,
  ) => Promise<AppResult<CaptureItemSummary>>
  createUploadId?: () => string
  maxBatchItems?: number
  concurrency?: number
}): {
  progress: Ref<CaptureFileImportProgress | undefined>
  importFiles: (files: File[]) => Promise<CaptureFileImportOutcome | undefined>
  clearProgress: () => void
  dispose: () => void
}
```

- [x] **Step 1: Write failing unit tests**

Add tests that prove:

```ts
it('uses two workers while preserving source sequence', async () => {
  const importing = controller.importFiles(files('a.png', 'b.png', 'c.png'))
  expect(importBytes).toHaveBeenCalledTimes(2)
  resolveFirst(success(item))
  await waitForThirdCall()
  expect(importBytes.mock.calls.map(call => call[0].sourceSequence))
    .toEqual([4, 5, 6])
  await importing
})

it('uses only remaining batch capacity', async () => {
  currentItemCount = 149
  const result = await controller.importFiles(files('a.png', 'b.png', 'c.png'))
  expect(importBytes).toHaveBeenCalledOnce()
  expect(result?.skippedCount).toBe(2)
})

it('reports live completed and failed counts', async () => {
  const importing = controller.importFiles(files('bad.png', 'good.png'))
  resolveBad(failure('invalid', '损坏', false, 'diag'))
  await waitForProgress({ completed: 1, total: 2, failed: 1 })
  resolveGood(success(item))
  await importing
})
```

Also cover file-read rejection, generated upload IDs, blocked/empty input, progress clearing, and disposal ignoring later progress callbacks.

- [x] **Step 2: Run the new test and verify failure**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureFileImport.test.ts
```

Expected: FAIL because `useCaptureFileImport.ts` does not exist.

- [x] **Step 3: Implement bounded workers and outcomes**

Slice files to the remaining capacity before `arrayBuffer()`. Use one shared monotonic `nextFileIndex` and:

```ts
const workerCount = Math.min(concurrency, filesToImport.length)
await Promise.all(Array.from({ length: workerCount }, async () => {
  while (nextFileIndex < filesToImport.length) {
    const index = nextFileIndex
    nextFileIndex += 1
    await importOne(filesToImport[index]!, currentItemCount + index)
  }
}))
```

Each worker records a failure by source name, increments progress in `finally`, and never aborts sibling work. `dispose()` prevents later progress/busy callbacks but does not pretend to cancel already-issued backend commands.

- [x] **Step 4: Run controller tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureFileImport.test.ts
```

Expected: all import controller tests PASS.

### Task 2: Integrate lifecycle-safe refresh

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] **Step 1: Add a route-level late-completion regression**

Hold both `captureImportBytes` calls, start importing two files, close the batch, release both calls, and assert the active batch remains `none`. Run against the inline implementation first and verify it fails by reopening `batch-1`.

- [x] **Step 2: Replace inline worker orchestration**

Instantiate the controller with normalized `captureImportBytes`, current batch/item getters, global busy callbacks, and `crypto.randomUUID`. Keep notice construction in `CaptureView.vue`. After an outcome:

```ts
if (detail.value?.batch.id === outcome.batchId) {
  await loadDetail(outcome.batchId)
}
else {
  await loadBatches()
}
```

Clear progress in the synchronous batch watcher and dispose it on unmount.

- [x] **Step 3: Run focused view/controller tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCaptureFileImport.test.ts src/app/views/CaptureView.test.ts
```

Expected: all focused tests PASS, including partial failure, 150-cap, and leave-during-import cases.

### Task 3: Add accessible import progress

**Files:**

- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] **Step 1: Add a failing component test**

Render with:

```ts
importProgress: { completed: 3, total: 10, failed: 1 }
```

Assert a `role="status"` region says `正在导入 3/10 张，1 张失败` and a `<progress max="10" value="3">` element is present.

- [x] **Step 2: Add the optional prop and status UI**

Add `importProgress?: CaptureFileImportProgress` to props. Under the capture toolbar render:

```vue
<div
  v-if="importProgress"
  class="import-progress"
  role="status"
  aria-live="polite"
>
  <span>
    正在导入 {{ importProgress.completed }}/{{ importProgress.total }} 张
    <template v-if="importProgress.failed">，{{ importProgress.failed }} 张失败</template>
  </span>
  <progress :max="importProgress.total" :value="importProgress.completed" />
</div>
```

Pass `:import-progress="importProgress"` from `CaptureView`.

- [x] **Step 3: Run component and integration tests**

```powershell
npm run test -- --run src/modules/capture/components/CaptureWorkspace.test.ts src/modules/capture/composables/useCaptureFileImport.test.ts src/app/views/CaptureView.test.ts
```

Expected: all focused tests PASS.

### Task 4: Verify and review

- [x] **Step 1: Run frontend gates**

```powershell
npm run lint
npm run typecheck
npm run test:coverage
npm run build
```

- [x] **Step 2: Review the scoped diff**

Confirm remaining capacity, two-worker bound, ordering, partial failure, progress accessibility, leave-during-import behavior, unchanged events/commands, and no recognition/OCR or Rust edits.

- [x] **Step 3: Mark the plan complete**

Do not stage or commit.
