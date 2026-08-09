# Recognition Crop Return Focus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give both manual crop and recognition-proposal crop dialogs one reusable, race-safe return-focus boundary, then restore recognition users to the exact suggestion edit control after cancel, open failure, or successful save.

**Architecture:** Extract the generation/context/open-state checks currently embedded in `CaptureView.vue` into a small `modal-return-focus.ts` controller. The page supplies workflow-specific target lookup functions; ordinary crop may override the restore target with its first derived result, while recognition crop falls back to the edit control identified by suggestion ID.

**Tech Stack:** Vue 3 Composition API, TypeScript with `exactOptionalPropertyTypes`, Vitest, Testing Library Vue.

## Global Constraints

- Do not change licensing, privacy/legal, support operations, account deletion, device migration, updater recovery, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not edit `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this batch; verification replaces commit checkpoints in this shared dirty worktree.
- Keep `data-crop-item-id`, `data-crop-result-item-id`, and `data-recognition-edit-suggestion-id` semantically separate.

---

### Task 1: Shared modal return-focus controller

**Files:**
- Create: `src/app/modal-return-focus.ts`
- Create: `src/app/modal-return-focus.test.ts`

**Interfaces:**
- Consumes: Vue `nextTick`, current context ID, modal-open state, and a workflow fallback lookup.
- Produces: `createModalReturnFocusController(options)` with `capture`, `clear`, and asynchronous `restore` methods.

- [x] **Step 1: Write failing controller tests**

Cover these exact behaviors with real buttons attached to `document.body` and a deferred `afterRender` promise:

```ts
const controller = createModalReturnFocusController({
  currentContextId: () => contextId,
  isModalOpen: () => modalOpen,
  findFallback: targetId => fallbackTargets.get(targetId),
  afterRender: () => renderGate.promise,
})
```

Assert that a connected enabled original button is restored, a disconnected original uses `findFallback`, an explicit successor resolver wins, and `clear()`, a newer `capture()`, a context switch, or another open modal cancels an in-flight restore.

- [x] **Step 2: Run the focused test and verify it fails**

Run: `pnpm vitest run src/app/modal-return-focus.test.ts`

Expected: FAIL because `modal-return-focus.ts` does not exist.

- [x] **Step 3: Implement the controller**

Use this public shape:

```ts
export interface ModalReturnFocusCapture {
  contextId: string
  targetId: string
  element: HTMLButtonElement | undefined
}

export interface ModalReturnFocusController {
  capture: (input: ModalReturnFocusCapture) => void
  clear: () => void
  restore: (findSuccessor?: () => HTMLButtonElement | undefined) => Promise<boolean>
}
```

Each capture stores a monotonically increasing generation. `restore()` must wait for render, then recheck request identity, generation, context ID, and modal-open state before consuming the request and focusing an enabled connected original, successor, or fallback.

- [x] **Step 4: Run the focused controller test**

Run: `pnpm vitest run src/app/modal-return-focus.test.ts`

Expected: PASS.

### Task 2: Migrate ordinary crop focus return

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Test: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Consumes: `createModalReturnFocusController`, `visibleCropEditor`, `detail`, `cropLauncherFor`, and `cropResultControlFor`.
- Produces: behavior equivalent to the already verified crop return-focus paths without page-local generation state.

- [x] **Step 1: Add or retain regression assertions**

Keep coverage proving cancel returns to the original crop launcher, preview failure uses a replacement launcher, successful crop focuses the first derived result control, save failure stays inside the editor, and leaving the detail cancels pending restoration.

- [x] **Step 2: Replace page-local crop generation logic**

Instantiate one controller:

```ts
const cropReturnFocus = createModalReturnFocusController({
  currentContextId: () => detail.value?.batch.id,
  isModalOpen: () => Boolean(visibleCropEditor.value),
  findFallback: cropLauncherFor,
})
```

Call `capture()` before `openCropEditor`, `restore()` on cancel/open failure, pass `() => cropResultControlFor(report.derivedItemIds[0])` after successful apply, and call `clear()` on detail clear and unmount.

- [x] **Step 3: Run ordinary crop integration tests**

Run: `pnpm vitest run src/app/views/CaptureView.test.ts src/modules/capture/composables/useCaptureItemEditing.test.ts`

Expected: PASS.

### Task 3: Recognition proposal focus continuity

**Files:**
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Consumes: recognition suggestion IDs, `editRecognition`, `closeRecognitionProposal`, `saveRecognitionProposal`, and the shared controller.
- Produces: `data-recognition-edit-suggestion-id` on both `手工裁剪` and `调整边界` controls plus page handlers `openRecognitionCropEditor`, `closeRecognitionCropEditor`, and `saveRecognitionCropEditor`.

- [x] **Step 1: Write failing component and integration tests**

Assert both recognition edit-button variants expose their current suggestion ID. In `CaptureView.test.ts`, cover cancel, preview failure with a replaced launcher, successful save with a replaced accepted-state launcher, save failure retaining focus inside the proposal editor, and detail exit cancelling pending restoration.

- [x] **Step 2: Run the recognition-focused tests and verify failure**

Run: `pnpm vitest run src/modules/ocr/components/CaptureRecognitionReview.test.ts src/app/views/CaptureView.test.ts`

Expected: FAIL because the stable recognition target and page handlers do not exist.

- [x] **Step 3: Implement recognition handlers**

Create a second controller whose fallback queries enabled buttons by `data-recognition-edit-suggestion-id`. Capture the batch ID, suggestion ID, and active edit button before awaiting `editRecognition`. Restore only when `recognitionCropEditor` is absent after open/save; keep focus inside on save failure. Clear the controller before recognition reset and on unmount.

- [x] **Step 4: Run focused tests**

Run: `pnpm vitest run src/app/modal-return-focus.test.ts src/app/views/CaptureView.test.ts src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts`

Expected: PASS.

### Task 4: Verification and review

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-recognition-crop-return-focus.md`

**Interfaces:**
- Consumes: all completed changes.
- Produces: verified, reviewed batch with every checkbox completed.

- [x] **Step 1: Run static and production checks**

Run `pnpm lint`, `pnpm typecheck`, and `pnpm build`.

Expected: all exit 0.

- [x] **Step 2: Run the full frontend suite**

Run: `pnpm test`

Expected: all test files pass.

- [x] **Step 3: Request code review**

Review shared-controller cancellation, nested modal focus behavior, recognition save failure, and target-ID separation. Fix every Critical or Important finding and rerun affected checks.

- [x] **Step 4: Mark this plan complete**

Only check the remaining boxes after focused tests, static checks, production build, full tests, and follow-up review pass.
