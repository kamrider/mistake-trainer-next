# Capture Preview Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `CaptureView.vue`’s inline thumbnail cache with a bounded LRU controller that cannot resurrect previews after item invalidation, batch changes, or view disposal.

**Architecture:** A Vue composable owns the reactive preview map, true least-recently-used ordering, per-request identity, and a global lifecycle epoch. `CaptureView.vue` supplies the active batch and normalized preview command, invalidates entries after destructive/crop mutations, clears the cache when the active batch changes, and disposes it on unmount.

**Tech Stack:** TypeScript, Vue 3 reactive state, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve every pre-existing worktree change.
- Do not edit recognition/OCR, Rust, generated bindings, installer/release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Keep the `CaptureWorkspace` `previews` prop and `preview` event unchanged.
- Keep preview failures silent so one thumbnail never interrupts batch organization.
- Keep the cache limit at exactly 40 entries in `CaptureView`.
- Coalesce duplicate requests for the same item and active batch.
- A response started before item invalidation, batch change, or disposal must never enter the cache.
- Validate that the returned `CaptureItemPreview.itemId` matches the requested item.
- Do not stage or commit.

---

### Task 1: Build and prove the preview cache

**Files:**

- Create: `src/modules/capture/composables/useCapturePreviewCache.ts`
- Create: `src/modules/capture/composables/useCapturePreviewCache.test.ts`

**Interfaces:**

```ts
export interface CapturePreviewCache {
  previews: Record<string, string>
  load: (itemId: string) => Promise<void>
  invalidate: (itemId: string) => void
  clear: () => void
  dispose: () => void
}

export function useCapturePreviewCache(options: {
  activeBatchId: () => string | undefined
  fetchPreview: (
    batchId: string,
    itemId: string,
  ) => Promise<AppResult<CaptureItemPreview>>
  maxEntries?: number
}): CapturePreviewCache
```

- [x] **Step 1: Write failing unit tests**

Add tests that prove:

```ts
it('coalesces duplicate requests', async () => {
  const first = cache.load('item-1')
  const second = cache.load('item-1')
  expect(fetchPreview).toHaveBeenCalledOnce()
  resolvePreview(success(preview('item-1')))
  await Promise.all([first, second])
})

it('uses true LRU order on cache hits', async () => {
  await cache.load('item-1')
  await cache.load('item-2')
  await cache.load('item-1')
  await cache.load('item-3')
  expect(cache.previews['item-1']).toBeDefined()
  expect(cache.previews['item-2']).toBeUndefined()
})

it('does not reinsert an invalidated in-flight item', async () => {
  const loading = cache.load('item-1')
  cache.invalidate('item-1')
  resolvePreview(success(preview('item-1')))
  await loading
  expect(cache.previews['item-1']).toBeUndefined()
})
```

Also cover batch-change `clear()`, mismatched response IDs, silent failures, and `dispose()`.

- [x] **Step 2: Run the new test and verify failure**

```powershell
npm run test -- --run src/modules/capture/composables/useCapturePreviewCache.test.ts
```

Expected: FAIL because `useCapturePreviewCache.ts` does not exist.

- [x] **Step 3: Implement true LRU and request identity**

Use:

```ts
interface PreviewRequest {
  batchId: string
  epoch: number
  promise: Promise<void>
}

const previews = reactive<Record<string, string>>({})
const order: string[] = []
const requests = new Map<string, PreviewRequest>()
let epoch = 0
let disposed = false
```

On a cache hit, remove the item from `order` and append it. On a successful response, apply only when the request remains the current map entry, its epoch matches, the active batch is unchanged, and `result.data.itemId === itemId`. `invalidate()` removes the request identity and cached entry. `clear()` increments the epoch, clears every request and preview, and `dispose()` permanently rejects future work.

- [x] **Step 4: Run cache tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCapturePreviewCache.test.ts
```

Expected: all cache tests PASS.

### Task 2: Integrate CaptureView

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] **Step 1: Add a route-level late-response regression**

Start a preview request, close the active batch, resolve the old request, and assert `preview-cache` remains `0`. Run this test against the inline implementation first and verify it fails with cache size `1`.

- [x] **Step 2: Replace inline cache state**

Instantiate:

```ts
const previewCache = useCapturePreviewCache({
  activeBatchId: () => detail.value?.batch.id,
  fetchPreview: async (batchId, itemId) =>
    normalizeAppResult(await commands.captureItemPreview(batchId, itemId)),
  maxEntries: 40,
})
const previews = previewCache.previews
const loadPreview = previewCache.load
const removeCachedPreview = previewCache.invalidate
```

Remove `reactive`, `previewOrder`, `previewRequests`, the inline `removeCachedPreview`, and inline `loadPreview`. In the existing active-batch watcher call `previewCache.clear()` whenever the batch ID changes. On unmount call `previewCache.dispose()`.

- [x] **Step 3: Run focused tests**

```powershell
npm run test -- --run src/modules/capture/composables/useCapturePreviewCache.test.ts src/app/views/CaptureView.test.ts src/modules/capture/components/CaptureThumbnail.test.ts
```

Expected: all focused tests PASS; the existing duplicate-request and 40-entry tests remain green.

### Task 3: Verify and review

- [x] **Step 1: Run frontend gates**

```powershell
npm run lint
npm run typecheck
npm run test:coverage
npm run build
```

- [x] **Step 2: Review the scoped diff**

Confirm true LRU behavior, request coalescing, item/batch/lifecycle invalidation, exact limit 40, silent failure semantics, unchanged workspace contract, and no recognition/OCR or Rust edits.

- [x] **Step 3: Mark the plan complete**

Do not stage or commit.
