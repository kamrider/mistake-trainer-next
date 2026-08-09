# Modal Background Inert Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every non-excluded root modal truly isolate its background for assistive technology and programmatic focus, including nested and out-of-order modal closure.

**Architecture:** Add `acquireDialogBackgroundInert(modalRoot)` to walk from a connected modal root to `document.body`, marking every sibling along that ancestor path inert with per-element reference counts and exact pre-existing-state restoration. Compose it with the existing scroll lock as `acquireDialogDocumentBoundary(modalRoot)`, then migrate twelve root modal owners from scroll-only acquisition to the combined boundary and remove the review-history-specific inert implementation.

**Tech Stack:** Vue 3 Composition API, TypeScript DOM APIs, Vitest, Testing Library Vue.

## Global Constraints

- `acquireDialogBackgroundInert(modalRoot: HTMLElement): () => void` must never set `inert` on the modal root or any ancestor that contains it.
- Every HTMLElement sibling along the modal-root-to-body path must receive the inert attribute for the boundary lifetime.
- An element that was inert before acquisition must remain inert after the final release.
- Overlapping nested acquisitions must reference-count shared background elements; releasing an outer boundary before an inner one must not make the effective background interactive.
- Every release function must be idempotent and safe after a marked element is disconnected.
- `acquireDialogDocumentBoundary(modalRoot: HTMLElement): () => void` must acquire both background inert and the existing document scroll lock, then release both exactly once.
- All twelve non-excluded root `aria-modal` owners must use the combined boundary. The nested recognition impact alertdialog must remain covered by the outer review boundary and must not acquire separately.
- `ReviewHistoryDetail` must continue acquiring only in its mobile modal branch; desktop detail remains non-modal, unlocked, and non-inert.
- Preserve all existing focus traps, Escape admission, initial focus, focus returns, busy gating, crop shortcuts, lightbox arrows, scroll restoration, and emitted payloads.
- Do not modify `BackupRestoreDialog.vue` or `StorageMigrationDialog.vue`; backup/restore and device/storage migration remain excluded.
- Do not modify licensing, privacy, support, account deletion, updater recovery, SLA, native/Rust behavior, recognition algorithms, or data transactions.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Build the combined document boundary

**Files:**

- Create: `src/app/dialog-document-boundary.ts`
- Create: `src/app/dialog-document-boundary.test.ts`
- Reuse unchanged: `src/app/dialog-scroll-lock.ts`

**Interfaces:**

- Produces: `acquireDialogBackgroundInert(modalRoot: HTMLElement): () => void`.
- Produces: `acquireDialogDocumentBoundary(modalRoot: HTMLElement): () => void`.

- [x] **Step 1: Write nested, out-of-order, and pre-existing inert tests**

Build a page tree containing a background sibling, a pre-inert sibling, an outer modal, outer content, and a nested modal. Set body overflow to `auto`; acquire outer then nested boundaries; release outer first; assert body remains hidden and the background remains inert. Release nested twice; assert overflow restores to `auto`, normal background inert is removed, and pre-inert remains.

```ts
const releaseOuter = acquireDialogDocumentBoundary(outerModal)
const releaseNested = acquireDialogDocumentBoundary(nestedModal)
releaseOuter()
expect(document.body.style.overflow).toBe('hidden')
expect(background).toHaveAttribute('inert')
releaseNested()
releaseNested()
expect(document.body.style.overflow).toBe('auto')
expect(background).not.toHaveAttribute('inert')
expect(preInert).toHaveAttribute('inert')
```

- [x] **Step 2: Run the boundary suite red**

Run:

```powershell
npm test -- --run src/app/dialog-document-boundary.test.ts
```

Expected: fail because the boundary module does not exist.

- [x] **Step 3: Implement reference-counted sibling acquisition**

Create an `inertStates` WeakMap with `{ depth, wasInert }`. Traverse with `current = modalRoot`; while `current !== ownerDocument.body`, inspect `current.parentElement`, acquire every HTMLElement child other than `current`, then continue with the parent. Record acquired elements for reverse-order idempotent release.

```ts
type InertState = { depth: number; wasInert: boolean }
const inertStates = new WeakMap<HTMLElement, InertState>()

export function acquireDialogBackgroundInert(modalRoot: HTMLElement) {
  const acquired: HTMLElement[] = []
  let current: HTMLElement | null = modalRoot
  const body = modalRoot.ownerDocument.body
  while (current && current !== body) {
    const parent = current.parentElement
    if (!parent) break
    for (const sibling of parent.children) {
      if (sibling === current || !(sibling instanceof HTMLElement)) continue
      let state = inertStates.get(sibling)
      if (!state) {
        state = { depth: 0, wasInert: sibling.hasAttribute('inert') }
        inertStates.set(sibling, state)
      }
      state.depth += 1
      sibling.setAttribute('inert', '')
      acquired.push(sibling)
    }
    current = parent
  }
  let released = false
  return () => {
    if (released) return
    released = true
    for (const element of [...acquired].reverse()) {
      const state = inertStates.get(element)
      if (!state) continue
      state.depth -= 1
      if (state.depth === 0) {
        if (!state.wasInert) element.removeAttribute('inert')
        inertStates.delete(element)
      }
    }
  }
}
```

- [x] **Step 4: Compose inert and scroll ownership**

Add:

```ts
export function acquireDialogDocumentBoundary(modalRoot: HTMLElement) {
  const releaseScrollLock = acquireDialogScrollLock(modalRoot.ownerDocument)
  const releaseBackgroundInert = acquireDialogBackgroundInert(modalRoot)
  let released = false
  return () => {
    if (released) return
    released = true
    releaseBackgroundInert()
    releaseScrollLock()
  }
}
```

- [x] **Step 5: Run boundary tests and typecheck green**

Run the new boundary test, existing scroll-lock test, and `npm run typecheck`.

### Task 2: Migrate existing full-screen and teleported owners

**Files:**

- Modify: `src/app/components/ActionConfirmDialog.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/review/components/ReviewMediaLightbox.vue`
- Verify: corresponding component tests.

**Interfaces:**

- Consumes: `acquireDialogDocumentBoundary(rootElement)`.
- Preserves: current root refs, focus traps, listener cleanup, and focus return.

- [x] **Step 1: Replace scroll-only handles with combined boundaries**

Replace each scroll-lock import and `releaseScrollLock` variable with the combined import and `releaseDialogBoundary`. During mount, require the existing panel/dialog/review root ref before acquisition:

```ts
if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
```

Release from the same unmount position previously used for the scroll lock.

- [x] **Step 2: Run the four component suites**

Run ActionConfirmDialog, CaptureCropEditor, CaptureRecognitionReview, and ReviewMediaLightbox suites together.

### Task 3: Migrate standard root modals

**Files:**

- Modify: `src/app/LibraryLockDialog.vue`
- Modify: `src/modules/legacy/components/LegacyImportDialog.vue`
- Modify: `src/modules/capture/components/CaptureLanDialog.vue`
- Modify: `src/modules/sync/components/SyncConflictBulkDialog.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Verify: corresponding component tests.

**Interfaces:**

- Consumes: `acquireDialogDocumentBoundary(panelOrDrawer)`.
- Preserves: LAN parent focus ownership and problem-drawer local focus return.

- [x] **Step 1: Replace five scroll-only acquisitions**

Import the combined boundary, rename release handles, acquire from the already-mounted panel/dialog/drawer ref, and release from each existing unmount hook.

- [x] **Step 2: Run the five standard modal suites**

Run all five corresponding tests together.

### Task 4: Replace responsive history's local inert implementation

**Files:**

- Modify: `src/modules/review-history/components/ReviewHistoryDetail.vue`
- Modify: `src/modules/review-history/components/ReviewHistoryDetail.mobile.test.ts`
- Verify: `src/modules/review-history/components/ReviewHistoryDetail.test.ts`

**Interfaces:**

- Consumes: `acquireDialogDocumentBoundary(detailLayer.value)` only in the mobile branch.
- Removes: local `inertBackground`, `setBackgroundInert()`, and `restoreBackground()`.

- [x] **Step 1: Strengthen the mobile background contract**

Append an external background button beside the rendered component, assert it becomes inert while mounted and restores after unmount, alongside the existing scroll and focus assertions.

- [x] **Step 2: Run the mobile suite against the current implementation**

Expected: fail because the current history-specific implementation only finds siblings inside `.history-page`; the standalone background sibling remains interactive.

- [x] **Step 3: Replace local inert and scroll ownership**

Remove the Map and two helper functions. In the existing `if (mobile.value)` branch, acquire the combined boundary from `detailLayer.value`; release it in unmount. Do not acquire or inert on desktop.

- [x] **Step 4: Run desktop and mobile history suites**

Confirm mobile focus/scroll/inert behavior and desktop non-modal behavior remain green.

### Task 5: Migrate the two inline capture modals

**Files:**

- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Verify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Verify: `src/modules/capture/components/CaptureDraftCard.test.ts`

**Interfaces:**

- Consumes: `acquireDialogDocumentBoundary(layoutImpact.value)` and `acquireDialogDocumentBoundary(expandedDialog.value)` after the modal DOM exists.
- Preserves: the complete focus/Tab/Escape/return behavior added in the prior batch.

- [x] **Step 1: Acquire layout boundary after render**

Replace the pre-render scroll acquisition with a nextTick callback that verifies `layoutConfirmOpen` and `layoutImpact.value`, acquires the combined boundary, then focuses the return button. Keep close/unmount release idempotent.

- [x] **Step 2: Acquire expanded-image boundary after render**

In `openImage`, after setting `expanded` and awaiting `nextTick`, acquire from `expandedDialog.value` before focusing the close button. Keep close/unmount release idempotent.

- [x] **Step 3: Run workspace and draft-card suites**

Confirm both existing complete modal lifecycle regressions remain green.

### Task 6: Commercial-quality gates and local review

**Files:**

- Verify all files above and `docs/superpowers/plans/2026-08-03-modal-background-inert-boundary.md`.

- [x] **Step 1: Run the complete affected suite**

Run document-boundary and scroll-lock utilities plus the 13 modal component test files. Expected: 15 files pass.

- [x] **Step 2: Run final project gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 3: Review boundary ownership and excluded scope**

Confirm all twelve non-excluded root modal owners call `acquireDialogDocumentBoundary`, only the shared scroll module calls `acquireDialogScrollLock`, only the shared inert module and unrelated card-face state set inert, review history no longer has local inert bookkeeping, nested recognition acquires once, and excluded files are unchanged.

- [x] **Step 4: Check workspace hygiene**

Run tracked/untracked whitespace checks, confirm the index is empty, confirm `dist/` remains ignored, and verify excluded dialogs plus the pre-existing `recognition_visual_split.rs` modification remain untouched.

## Verification record

- Audit: only mobile `ReviewHistoryDetail` manually inerts background siblings; eleven other non-excluded root modals rely on ARIA and focus trapping without DOM inert isolation. The local history implementation does not provide a reusable nested reference-count contract.
- Baseline: the scroll lock plus 13 affected component files passed, 14 files / 96 tests after the prior batch.
- Red: the combined-boundary suite failed before its module existed; the strengthened mobile-history regression proved the local `.history-page` implementation left an external background control interactive.
- Focused green: nested boundary and scroll utility, four full-screen/teleported modals, five standard roots, responsive history, and two inline capture modals all passed their focused suites; typecheck passed after explicitly typing the ancestor traversal parent.
- Affected green: document boundary, scroll lock, and all 13 modal component files passed, 15 files / 97 tests.
- Final gates: `npm run lint` and `npm run typecheck` passed; production build transformed 2051 modules; the single-worker full run passed 136 files / 759 tests.
- Local review: all twelve non-excluded root modal owners call `acquireDialogDocumentBoundary`; only the combined boundary calls `acquireDialogScrollLock`; review history has no local inert bookkeeping; the nested recognition impact reuses one outer acquisition; the unrelated card-face inert binding remains intact.
- Hygiene: tracked and untracked whitespace checks reported no errors (only Git LF→CRLF notices); the index is empty; `dist/` remains ignored; excluded backup/storage dialog files are unchanged; the pre-existing `recognition_visual_split.rs` modification remains present and untouched.
