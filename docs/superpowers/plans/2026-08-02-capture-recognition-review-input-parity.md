# Capture Recognition Review Input Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give pointer and keyboard users the same recognition-review capabilities while decisions save in the background, without allowing crop/apply operations to race the review queue.

**Architecture:** Keep `busy` as the workflow-wide “any recognition work pending” signal used by navigation and exclusive operations, and expose a second reactive `operationBusy` signal for start/cancel/edit/apply/revert operations. Propagate both through `CaptureView` and `CaptureWorkspace`; the review surface allows queueable accept/reject actions when `busy && !operationBusy`, blocks all review decisions during exclusive work, and continues blocking edit/apply until every save finishes.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue.

## Global Constraints

- `busy` remains true while any review decision is queued or any exclusive recognition operation is running.
- `operationBusy` is true only during start, cancel, preview-for-edit, apply, or revert operations; it is false while only the review queue drains.
- Pointer and keyboard accept/reject paths must use the same `operationBusy` admission rule.
- During background review saving, accept/reject and bulk high-confidence acceptance remain available; edit/crop and apply remain disabled because they require an empty review queue.
- During exclusive recognition work, accept/reject, edit/crop, bulk acceptance, and apply are all disabled or ignored.
- Display exact visible feedback while decisions save: `正在后台保存审核决定；你可以继续确认下一条，应用切图需等待保存完成。`
- Preserve review serialization/coalescing, optimistic projection and rollback, job/session continuity, focus handling, navigation guards, close behavior, and event payloads.
- Do not alter recognition algorithms, crop geometry, native/Rust transactions, storage/device migration, updater recovery, account deletion, licensing, privacy, support, or launch gates.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize busy-state input parity

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`
- Modify: `src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**

- Consumes: review props `busy` and new `operationBusy`; workflow refs `busy` and new `operationBusy`; workspace props `recognitionBusy` and new `recognitionOperationBusy`.
- Produces: regressions proving queue-save and exclusive-operation behavior at all three boundaries.

- [x] **Step 1: Add component pointer/keyboard parity regression**

Render two review suggestions with `busy: true, operationBusy: false`. Assert the save message is visible, Accept/Skip remain enabled, Adjust is disabled, pointer Accept emits a decision, and keyboard `s` emits the next decision. Rerender with `operationBusy: true`; assert decision buttons are disabled and Enter/S no longer emit.

- [x] **Step 2: Add workflow state regression**

Hold a review request with a deferred promise and assert:

```ts
expect(controller.busy.value).toBe(true)
expect(controller.operationBusy.value).toBe(false)
```

Hold `start()` with a deferred promise and assert both refs are true until it settles, then both are false.

- [x] **Step 3: Add workspace propagation regression**

Open a recognition review with `recognitionBusy: true, recognitionOperationBusy: false`; assert the background-save message is visible, Accept is enabled, and Adjust is disabled. Rerender `recognitionOperationBusy: true` and assert Accept becomes disabled.

- [x] **Step 4: Run focused suites red**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: the new prop/ref is absent, pointer decisions remain disabled under generic busy, keyboard decisions still fire during exclusive busy, and no saving feedback is rendered.

### Task 2: Expose operation-specific workflow state

**Files:**

- Modify: `src/modules/ocr/composables/useCaptureRecognitionWorkflow.ts`
- Modify: `src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts`

**Interfaces:**

- Produces: reactive `operationBusy: Ref<boolean>` alongside existing `busy`.

- [x] **Step 1: Replace the non-reactive exclusive flag**

Replace `let exclusiveRunning = false` with `const operationBusy = ref(false)`. Set it true/false around start, cancel, edit, apply, and revert; use it in the review admission guard; clear it in `reset()`.

- [x] **Step 2: Preserve aggregate busy semantics**

Keep `busy.value = true` for both queue drains and exclusive operations. In `drainReviews()` restore `busy.value = operationBusy.value`; exclusive operation finalizers clear both refs only for the current lifecycle.

- [x] **Step 3: Return and verify the new state**

Add `operationBusy` to the controller return value and run the workflow suite. Confirm rapid reviews still serialize and roll back exactly as before.

### Task 3: Apply input-specific admission in the review UI

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Consumes: props `busy?: boolean` and `operationBusy?: boolean`.
- Produces: equal decision admission for pointer/keyboard and visible background-save copy.

- [x] **Step 1: Separate queueable and exclusive actions**

Use `operationBusy` for Accept, Skip, Keep Original, bulk acceptance, and keyboard Enter/S. Continue using aggregate `busy` for Adjust/Manual Crop and Apply Accepted. Navigation, filter selection, and close remain available.

- [x] **Step 2: Render save-in-progress feedback**

When `busy && !operationBusy`, render:

```vue
<p class="review-save-state" aria-live="polite">
  正在后台保存审核决定；你可以继续确认下一条，应用切图需等待保存完成。
</p>
```

- [x] **Step 3: Run component tests green**

Run the review component suite and `npm run typecheck`.

### Task 4: Propagate state through workspace and view

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**

- `CaptureView` consumes `recognitionWorkflow.operationBusy`.
- `CaptureWorkspace` adds optional `recognitionOperationBusy` and passes it to `CaptureRecognitionReview.operationBusy`.

- [x] **Step 1: Propagate the reactive ref**

Destructure `operationBusy: recognitionOperationBusy` from the workflow, bind `:recognition-operation-busy` on `CaptureWorkspace`, add the workspace prop, and bind `:operation-busy` on `CaptureRecognitionReview`.

- [x] **Step 2: Update mocked prop contracts**

Add `recognitionOperationBusy` to the Capture workspace mock prop list in `CaptureView.test.ts`, preserving all existing mock behavior.

- [x] **Step 3: Run adjacent integration suites**

Run review session, review component, recognition workflow, workspace, and Capture view suites together.

### Task 5: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review state truthfulness**

Confirm `busy` still protects leaving/applying while saves remain, `operationBusy` covers every exclusive operation, pointer and keyboard decision paths share the same guard, edit/apply remain blocked during queue saves, and no native boundary changed.

- [x] **Step 3: Check workspace hygiene**

Run targeted tracked/untracked whitespace checks, confirm the index is empty, confirm `dist` remains ignored, and verify the pre-existing `recognition_visual_split.rs` modification remains untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Audit: `useCaptureRecognitionWorkflow.review()` intentionally accepts additional decisions while its queue drains (`exclusiveRunning` is false), but `CaptureRecognitionReview` disables pointer buttons with aggregate `busy` and does not guard keyboard decisions with the same state. This proves inconsistent input admission rather than an unsupported queue capability.
- Baseline: review component, recognition workflow, Capture workspace, and Capture view passed, 4 files / 83 tests.
- Red: focused suites failed in three intended boundaries—the workflow lacked `operationBusy`, the review component lacked save feedback and kept pointer decisions disabled by aggregate busy, and Workspace rejected `recognitionOperationBusy` as an extraneous prop.
- Focused green: review component, recognition workflow, Capture workspace, and Capture view passed, 4 files / 86 tests; `npm run typecheck` and `npm run lint` passed.
- Final gates: production `npm run build` passed (2,048 modules); single-worker full Vitest passed, 133 files / 750 tests.
- Local review: aggregate `busy` still protects navigation, edit, apply, and other exclusive work while review saves remain; reactive `operationBusy` is set/cleared around start, cancel, edit preview, apply, and revert, and is cleared by reset. Pointer buttons and keyboard Enter/S share the operation guard; edit/apply also defend against `operationBusy` independently of the aggregate invariant. No native or Rust boundary changed.
- Hygiene: targeted tracked and untracked whitespace checks reported no errors; index is empty; `dist` is absent from status; the pre-existing `recognition_visual_split.rs` modification remains present and was not edited in this batch.
