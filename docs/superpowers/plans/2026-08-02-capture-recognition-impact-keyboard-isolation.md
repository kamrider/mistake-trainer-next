# Capture Recognition Impact Keyboard Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent keyboard input inside the apply-impact confirmation from triggering shortcuts or close actions on the recognition review dialog underneath it.

**Architecture:** Keep the existing nested alertdialog and focus trap, but make it the exclusive keydown boundary while open. Stop keydown propagation at the impact dialog before the outer review surface sees J/K, Enter, S, E, Tab, or Escape; retain `handleImpactKeydown` as the sole owner of inner Escape and Tab behavior.

**Tech Stack:** Vue 3 templates, TypeScript, Vitest, Testing Library Vue.

## Global Constraints

- While the impact alertdialog is open, J/K must not change the underlying suggestion.
- Enter/S must not accept or reject an underlying suggestion.
- E must not emit crop/edit for an underlying suggestion.
- Escape must close only the impact alertdialog, restore focus to the Apply button, and must not emit the outer review `close` event.
- Tab and Shift+Tab must remain trapped inside the impact alertdialog.
- Clicking Confirm and Cancel must preserve existing payloads and focus behavior.
- Preserve review queue semantics, operationBusy admission, review-session continuity, screen-reader announcements, and outer-dialog shortcuts when the impact dialog is closed.
- Do not alter recognition algorithms, crop geometry, native/Rust transactions, storage/device migration, updater recovery, account deletion, licensing, privacy, support, or launch gates.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize nested-dialog keyboard leakage

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Consumes: outer review shortcuts and inner impact alertdialog.
- Produces: one regression covering navigation, decision, edit, Escape, and focus isolation.

- [x] **Step 1: Build a two-suggestion impact scenario**

Accept the first of two review suggestions so the review advances to `2 / 2`, then open the Apply impact alertdialog.

- [x] **Step 2: Exercise leaked shortcuts**

Dispatch `k`, `s`, `Enter`, and `e` keydown events on the alertdialog. Assert the position remains `2 / 2`, the initial Accept remains the only `review` emission, and no `edit` event is emitted.

- [x] **Step 3: Assert Escape ownership**

Dispatch Escape on the alertdialog. Assert it disappears, focus returns to the Apply button, the outer review dialog remains visible, and `close` was not emitted.

- [x] **Step 4: Run the component suite red**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts
```

Expected: underlying cursor/decision/edit assertions fail and Escape emits outer close because keydown bubbles to the review surface.

### Task 2: Make the impact dialog the keyboard boundary

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Preserves: `handleImpactKeydown(event: KeyboardEvent)` for Escape and focus trapping.
- Produces: stopped keydown propagation via the inner alertdialog template binding.

- [x] **Step 1: Stop propagation at the alertdialog**

Change the inner binding to:

```vue
@keydown.stop="handleImpactKeydown"
```

Do not add key-specific duplication to the outer handler.

- [x] **Step 2: Run component and review-session suites green**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/ocr/composables/useCaptureRecognitionReviewSession.test.ts
npm run typecheck
```

- [x] **Step 3: Run adjacent workspace and Capture view suites**

Run review workflow, Capture workspace, and Capture view tests to confirm nested close/focus and parent event behavior remain intact.

### Task 3: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review keyboard ownership**

Confirm only the impact dialog handles keys while open, outer shortcuts still work when closed, focus trap selectors remain unchanged, Escape has exactly one owner, and Confirm/Cancel payloads remain unchanged.

- [x] **Step 3: Check workspace hygiene**

Run targeted tracked/untracked whitespace checks, confirm the index is empty, confirm `dist` remains ignored, and verify the pre-existing `recognition_visual_split.rs` modification remains untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Audit: the inner alertdialog uses `@keydown="handleImpactKeydown"` and is nested inside the outer `@keydown="handleKeydown"` review surface. The inner handler prevents defaults but never stops propagation, while the outer handler treats the alertdialog element as a shortcut-capable target.
- Baseline: `CaptureRecognitionReview` passed, 1 file / 13 tests.
- Red: the new nested-dialog regression observed three total review emissions instead of the single intentional acceptance after K/S/Enter/E were dispatched inside the alertdialog, proving shortcut leakage before it could reach the remaining edit/Escape assertions.
- Focused green: review component and session passed, 2 files / 17 tests; review component, session, workflow, Workspace, and Capture view passed together, 5 files / 90 tests; typecheck passed. The isolation regression also directly verifies Tab/Shift+Tab focus cycling.
- Final gates: `npm run lint`, `npm run typecheck`, production `npm run build` (2,048 modules), and single-worker full Vitest (133 files / 751 tests) passed.
- Local review: the outer review retains `@keydown="handleKeydown"`; the nested impact alertdialog alone uses `@keydown.stop="handleImpactKeydown"`. Its Escape and Tab logic, Confirm payload, Cancel focus restoration, and outer shortcuts when closed are unchanged; the new regression proves K/S/Enter/E cannot reach the outer handler and Escape does not emit outer close.
- Hygiene: targeted tracked and untracked whitespace checks reported no errors; index is empty; `dist` is absent from status; the pre-existing `recognition_visual_split.rs` modification remains present and was not edited in this batch.
