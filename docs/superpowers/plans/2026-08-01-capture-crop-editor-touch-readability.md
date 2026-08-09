# Capture Crop Editor Touch Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the capture crop editor commercially readable and operable with 44px controls while preserving a precise 18px visual resize marker, and provide a safe browser route for repeatable desktop and narrow-screen verification.

**Architecture:** Extend the existing pure development fixture with a typed crop-editor state, then let `CaptureView` recognize only the exact `preview=crop-editor` development/browser mode and open that state without native calls. Keep crop geometry and component markup intact; enforce typography and hit-target rules through a source contract and CSS-only changes.

**Tech Stack:** Vue 3, TypeScript, Vue Router, Vitest, Testing Library Vue, Vite browser preview, scoped CSS

## Global Constraints

- Do not modify Rust, native commands, crop geometry math, crop recipe serialization, persistence, recognition proposal behavior, or desktop initialization.
- The crop preview must run only when `import.meta.env.DEV`, `!isTauri()`, and `route.query.preview === 'crop-editor'` are all true.
- Reuse the generated SVG data URLs from `createCaptureDevelopmentPreview`; do not read files, call native APIs, or persist preview edits.
- Every explicit visible pixel font size in `CaptureCropEditor.vue` must be at least 12px.
- Close, toolbar, footer, region-select, reorder, delete, and resize actions must expose a 44px target.
- Resize handles must retain an 18px visible marker centered inside the 44px button so precision feedback is not visually oversized.
- Preserve keyboard nudging, focus trapping, Escape close, undo/redo, zoom, rotation, region ordering, reduced motion, and forced-colors behavior.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.
- Preserve the dirty worktree; do not stage or commit.

---

### Task 1: Safe Crop Editor Development Preview

**Files:**
- Modify: `src/app/views/capture-development-preview.ts`
- Modify: `src/app/views/capture-development-preview.test.ts`
- Modify: `src/app/views/CaptureView.vue`

**Interfaces:**
- Produces: `createCaptureDevelopmentCropEditor(preview: CaptureDevelopmentPreview, itemId?: string): CaptureCropEditorState`.
- Consumes: `CaptureView` loads the existing capture fixture, assigns the returned editor state, and bypasses native initialization only for the exact development mode.

- [x] **Step 1: Add the failing typed fixture test**

Import `createCaptureDevelopmentCropEditor`, create the existing fixture, and assert the returned state references `preview-q1`, `圆锥曲线题面.png`, and exactly `preview.previews['preview-q1']`.

- [x] **Step 2: Run the fixture test and verify red**

Run: `npm test -- --run src/app/views/capture-development-preview.test.ts`

Expected: FAIL because `createCaptureDevelopmentCropEditor` is not exported.

- [x] **Step 3: Implement the pure crop state and exact route gate**

Add:

```ts
export function createCaptureDevelopmentCropEditor(
  preview: CaptureDevelopmentPreview,
  itemId = 'preview-q1',
): CaptureCropEditorState {
  const item = preview.detail.items.find(value => value.id === itemId)
  const dataUrl = preview.previews[itemId]
  if (!item || !dataUrl) throw new Error(`Missing capture preview item: ${itemId}`)
  return { itemId, itemName: item.sourceName, dataUrl }
}
```

In `CaptureView`, derive a mode only for `capture-card` or `crop-editor`; load the shared preview for either mode and assign the typed crop state only for `crop-editor`. Keep the existing non-desktop warning for every other route.

- [x] **Step 4: Run fixture and CaptureView regression tests**

Run: `npm test -- --run src/app/views/capture-development-preview.test.ts src/app/views/CaptureView.test.ts`

Expected: both files pass.

### Task 2: Crop Editor Readability And Hit-Target Contract

**Files:**
- Create: `src/modules/capture/components/CaptureCropEditorReadability.test.ts`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`

**Interfaces:**
- Consumes: the crop-editor SFC source from the repository root.
- Produces: a contract rejecting explicit visible font sizes below 12px and asserting 44px controls with an 18px pseudo-element resize marker.

- [x] **Step 1: Add the failing source contract**

Read and compact `CaptureCropEditor.vue`. Assert no `font-size` below 12px; assert `.icon-button` is 44×44; toolbar/footer actions have `min-height:44px`; `.region-select` has `min-height:44px`; region action columns and buttons are 44px; `.resize-handle` is 44×44; and `.resize-handle::after` is 18×18.

- [x] **Step 2: Run the contract and verify red**

Run: `npm test -- --run src/modules/capture/components/CaptureCropEditorReadability.test.ts`

Expected: FAIL on the 10px region badge and the current 26–40px controls / 18px resize target.

- [x] **Step 3: Raise typography and interaction geometry**

Change the region badge to 12px; close, toolbar, footer, region-select, and region actions to 44px. Convert resize buttons to transparent 44px boxes with a centered `::after` marker sized 18px, and reposition each handle around the crop boundary using ±22px offsets. Include `.resize-handle::after` in forced-colors rules.

- [x] **Step 4: Run focused behavior and contract tests**

Run: `npm test -- --run src/modules/capture/components/CaptureCropEditorReadability.test.ts src/modules/capture/components/CaptureCropEditor.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all focused tests pass; existing geometry and emitted recipe tests remain green.

### Task 3: Desktop And Narrow Browser Verification

**Files:**
- Modify only Task 1 or Task 2 files if visual verification reveals clipping, overlap, or page overflow.

**Interfaces:**
- Consumes: `/#/inbox?preview=crop-editor`.
- Produces: computed-layout and interaction evidence for the dialog, toolbar, crop canvas, resize handles, region strip, and footer.

- [x] **Step 1: Start and record a temporary Vite server**

Confirm port 1420 is initially empty, start `npm run dev -- --host 127.0.0.1`, and record the exact listener PID.

- [x] **Step 2: Verify desktop interactions at 1280×900**

Confirm no generic browser warning or page overflow; sampled controls compute to 44px; resize buttons compute to 44px while their pseudo-elements compute to 18px; add a second region, reorder it, zoom to 125%, undo/redo, and close/reopen the dialog.

- [x] **Step 3: Verify narrow interactions at 390×844**

Confirm the dialog fits without page overflow; the toolbar and region list scroll inside their own containers; canvas and footer remain reachable; sampled controls and resize buttons remain 44px; add a region, select it, reorder it, and close via the exact accessible control.

- [x] **Step 4: Restore browser and process state**

Reset the viewport, finalize only created tabs, stop the exact Vite execution and listener PID, and confirm port 1420 returns to its initial empty state.

### Task 4: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-capture-crop-editor-touch-readability.md`

**Interfaces:**
- Consumes: red/green tests, browser measurements, Git hygiene, and final code review.
- Produces: checked steps and exact verification evidence.

- [x] **Step 1: Run static and full regression checks**

Run: `npm run typecheck`

Run: `npm run lint`

Run: `npm test -- --run`

Expected: every command exits 0.

- [x] **Step 2: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust or launch-gate file was changed by this task.

- [x] **Step 3: Perform final code review**

Review fixture isolation, error behavior, route exactness, resize hit testing, forced colors, narrow layout, regression coverage, and production-path invariants. Fix every Critical or Important finding before proceeding.

- [x] **Step 4: Record evidence without committing**

Check every completed step and append exact red/green totals, computed desktop/narrow measurements, interaction results, process cleanup, static/full results, Git hygiene, scope, and review verdict. Do not stage or commit.

## Verification Record — 2026-08-01

- Red phase: the two new/extended suites ran 5 tests; 4 failed and 1 passed. Failures proved the crop-state factory was absent, the region badge was 10px, the close/toolbar/region controls were 18–40px, and no 18px marker inside a 44px handle existed yet.
- Focused green: development preview, `CaptureView`, crop readability, crop behavior, and capture workspace passed, 5 files / 75 tests.
- Desktop browser: at 1280×900 the exact crop preview opened without the generic browser warning or page overflow. Sampled visible type had a 12px minimum; close, toolbar, footer, region-select, region-action, and resize controls computed to 44px; resize pseudo-elements computed to 18px. Adding a second region produced 2 rows / 16 handles; moving region 2, zooming to 125%, undoing, redoing, closing, and full-page reopening all succeeded.
- Narrow browser: at 390×844 the dialog measured 390×844 and the page had no horizontal overflow. The toolbar contained its own overflow (`388px` client / `662px` scroll), the region strip contained its own overflow (`364px` client / `483px` scroll), the canvas measured 388×458, and the footer remained visible at 388×69. All sampled controls remained 44px; marker size remained 18px; adding, selecting, moving, and closing region state succeeded.
- Process hygiene: port 1420 was empty before the task. The temporary listener was PID 24564; after stopping that exact process, the port had zero listeners. The viewport override was reset and the created browser tab was finalized.
- Static and regression gates: `npm run typecheck` and `npm run lint` exited 0. `npm test -- --run` passed, 105 files / 635 tests.
- Patch hygiene and index: scoped `git diff --check` exited 0 with only existing LF/CRLF notices. `git diff --cached --name-only` was empty.
- Scope: this task changed only the capture development preview, its test, `CaptureView` preview plumbing, crop-editor CSS, the crop readability contract, and this plan. No Rust or launch-gate item was modified by this task; nothing was staged or committed.
- Final local review: no Critical or Important issue found. Production crop persistence and geometry remain delegated to the existing composable/domain code; development-only state is guarded by `import.meta.env.DEV`, non-Tauri runtime, and the exact supported preview values. Forced-colors, focus, keyboard, reduced-motion, and narrow overflow behavior remain covered by source inspection, existing tests, and real-browser verification.
