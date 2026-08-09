# Recognition Review Modal Focus Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep keyboard focus and page scrolling contained while the recognition review modal is open, using a shared focus-ring utility instead of another component-local implementation.

**Architecture:** Create a pure `trapDialogFocus(event, container)` utility under `src/app` with the product-wide focusable selector and wrap behavior. Use it in the existing high-coverage `ActionConfirmDialog` and both focus boundaries inside `CaptureRecognitionReview`; the outer review owns body overflow for its entire mounted lifetime, while `CaptureWorkspace` remains the sole owner of returning focus to the review launcher after close.

**Tech Stack:** Vue 3 Composition API, TypeScript, DOM KeyboardEvent APIs, Vitest, Testing Library Vue.

## Global Constraints

- The recognition review `aria-modal="true"` surface must cycle Shift+Tab from its first enabled control to its last and Tab from its last to its first.
- Disabled controls must not participate in the focus ring.
- While recognition review is mounted, `document.body.style.overflow` must be `hidden`; unmount must restore the exact prior inline value.
- Mount must focus the review root; `CaptureWorkspace` must remain the sole owner of returning focus to the review launcher after close.
- The nested impact alertdialog must continue using its own smaller focus boundary and must not leak keys to the outer review.
- `trapDialogFocus` must focus the container when no enabled focusable descendants exist.
- Preserve Escape semantics, review shortcuts, background-save/operationBusy admission, impact Confirm/Cancel payloads, ActionConfirmDialog behavior, and Workspace launcher focus restoration.
- Do not migrate unrelated dialogs in this batch; establish the reusable boundary with two high-coverage consumers only.
- Do not alter recognition algorithms, crop geometry, native/Rust transactions, storage/device migration, updater recovery, account deletion, licensing, privacy, support, or launch gates.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize outer modal containment

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Consumes: outer review dialog mount/unmount and enabled button order.
- Produces: regression evidence for focus cycling and scroll-lock restoration; the existing Workspace suite remains authoritative for launcher focus restoration.

- [x] **Step 1: Add focus and body ownership setup**

Create and focus an external element, set `document.body.style.overflow = 'auto'`, render the review component, then assert the review dialog receives focus and body overflow becomes `hidden`.

- [x] **Step 2: Add outer focus-cycle assertions**

Collect enabled buttons inside the review dialog. Focus the first and dispatch Shift+Tab; assert the last receives focus. Focus the last and dispatch Tab; assert the first receives focus.

- [x] **Step 3: Add scroll restoration assertions**

Unmount and assert body overflow returns to `auto`; remove the fixture element and inline style in test cleanup. Keep launcher focus restoration under the existing `CaptureWorkspace` integration test so the parent has sole ownership.

- [x] **Step 4: Run the review suite red**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts
```

Expected: body overflow remains `auto` and outer focus does not wrap.

### Task 2: Build the shared focus-ring utility

**Files:**

- Create: `src/app/dialog-focus.ts`
- Create: `src/app/dialog-focus.test.ts`

**Interfaces:**

- Produces: `trapDialogFocus(event: KeyboardEvent, container: HTMLElement | undefined): void`.

- [x] **Step 1: Test wrap and empty-container behavior**

Build a container with first/disabled/last buttons. Assert disabled controls are excluded, Shift+Tab wraps first→last, Tab wraps last→first, non-Tab keys do nothing, and an empty container focuses itself after Tab.

- [x] **Step 2: Implement the product focusable selector**

Use:

```ts
const dialogFocusableSelector = [
  'button:not([disabled])', '[href]', 'input:not([disabled])',
  'select:not([disabled])', 'textarea:not([disabled])', 'summary',
  '[tabindex]:not([tabindex="-1"])',
].join(', ')
```

Filter descendants with the `hidden` attribute, prevent default only when wrapping or focusing the empty container, and use `container.contains(document.activeElement)` to recover an invalid active element to the correct edge.

- [x] **Step 3: Run utility tests green**

Run the utility test and `npm run typecheck`.

### Task 3: Migrate reviewed modal boundaries

**Files:**

- Modify: `src/app/components/ActionConfirmDialog.vue`
- Verify unchanged: `src/app/components/ActionConfirmDialog.test.ts`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Consumes: `trapDialogFocus` from `src/app/dialog-focus.ts`.
- Preserves: all component props/emits and current template roles/labels.

- [x] **Step 1: Migrate ActionConfirmDialog**

Remove its local focusable query/wrap code and call `trapDialogFocus(event, panel.value)` after its Escape branch. Keep Teleport, safe-action initial focus, body lock, previous-focus restoration, and `@keydown.stop` unchanged.

- [x] **Step 2: Add outer review scroll ownership**

Import `onBeforeUnmount` and the shared utility. On mount, remember body overflow, set overflow hidden, then focus the review root. On unmount, restore overflow; do not capture or restore an external focus target because `CaptureWorkspace` owns that transition.

- [x] **Step 3: Apply shared wrapping to both review boundaries**

In outer `handleKeydown`, route Tab to `trapDialogFocus(event, reviewRoot.value)` and return before shortcuts. In `handleImpactKeydown`, retain Escape then call `trapDialogFocus(event, impactDialog.value)`; remove duplicated query/first/last code. Keep the inner `.stop` modifier.

- [x] **Step 4: Run modal suites green**

Run dialog-focus, ActionConfirmDialog, review component, and review-session suites together.

### Task 4: Commercial-quality gates and local review

- [x] **Step 1: Run adjacent integration suites**

Run recognition workflow, Capture workspace, and Capture view tests to confirm launcher focus and parent close behavior remain intact.

- [x] **Step 2: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 3: Review ownership and scope**

Confirm outer and inner review boundaries call the shared utility, inner events still stop, ActionConfirm behavior is unchanged, body restoration occurs exactly once, `CaptureWorkspace` remains the sole focus-return owner, and no unrelated dialog/native boundary changed.

- [x] **Step 4: Check workspace hygiene**

Run targeted tracked/untracked whitespace checks, confirm the index is empty, confirm `dist` remains ignored, and verify the pre-existing `recognition_visual_split.rs` modification remains untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Audit: `CaptureRecognitionReview` is fixed-position with `aria-modal="true"` and focuses its root on mount, but outer `handleKeydown` has no Tab branch, body scrolling is not locked, and unmount does not restore focus. At least nine other components contain local focus-ring algorithms, while `ActionConfirmDialog` provides an existing tested behavior model but no shared utility.
- Baseline: recognition review and ActionConfirmDialog passed, 2 files / 17 tests.
- Red: the outer review regression failed because body overflow remained `auto`; the shared utility suite first failed before its module existed, then an added reviewer edge case proved `[href][tabindex="-1"]` would incorrectly become the last focus target without explicit filtering.
- Focused green: shared utility, ActionConfirmDialog, review component/session/workflow, Capture workspace, and Capture view passed, 7 files / 96 tests; earlier modal-only run passed 4 files / 23 tests and typecheck passed.
- Final gates: `npm run lint` and `npm run typecheck` passed; production build transformed 2049 modules; the single-worker full run passed 134 files / 754 tests.
- Local review: outer and nested review boundaries share `trapDialogFocus`; nested key events still stop at the alertdialog; review unmount restores only body overflow; `CaptureWorkspace.closeRecognitionReview` is the sole launcher-focus owner; ActionConfirmDialog retains its prior lifecycle behavior.
- Hygiene: targeted tracked and untracked whitespace checks reported no errors (only Git LF→CRLF notices); the index is empty; generated `dist/` remains ignored; the pre-existing `src-tauri/src/infrastructure/recognition_visual_split.rs` modification remains present and was not touched by this batch.
