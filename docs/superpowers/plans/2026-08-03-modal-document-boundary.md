# Modal Document Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every non-excluded root modal a consistent, nested-safe document scroll boundary and complete the missing focus lifecycle for the two inline capture modals.

**Architecture:** Add a document-scoped `acquireDialogScrollLock(ownerDocument)` utility backed by a `WeakMap<Document, ScrollLockState>` so nested locks restore the exact original inline overflow only after the final idempotent release. Migrate direct body-style owners and add the shared lock to the remaining non-excluded root modals; keep focus-return ownership where it already lives, and explicitly complete focus/open/close behavior for the layout-impact dialog and draft-card image lightbox.

**Tech Stack:** Vue 3 Composition API, TypeScript DOM APIs, Vitest, Testing Library Vue.

## Global Constraints

- `acquireDialogScrollLock(ownerDocument?: Document): () => void` must set `ownerDocument.body.style.overflow = 'hidden'` immediately.
- The first lock must remember the exact prior inline overflow; nested acquisition must not overwrite it.
- Releasing one of multiple locks, including out of acquisition order, must keep overflow hidden until the last outstanding lock releases.
- Every returned release function must be idempotent.
- All non-excluded root surfaces with `aria-modal="true"` must acquire the shared scroll lock for their modal lifetime.
- The nested impact alertdialog inside `CaptureRecognitionReview` must reuse the outer review lock rather than acquire a second component-local lock.
- `ReviewHistoryDetail` must acquire a lock only when its existing mobile media-query branch makes it modal; desktop detail remains non-modal and unlocked.
- Preserve existing Escape admission, busy-state gating, focus traps, initial focus, parent-owned focus returns, crop shortcuts, lightbox arrows, inert background, and data payloads.
- The layout-impact dialog and capture draft image lightbox must focus a safe close/return action on open, trap Tab, close with Escape, restore the triggering element when connected, and release scroll on close/unmount.
- Do not modify `BackupRestoreDialog.vue` or `StorageMigrationDialog.vue`; backup/restore and device/storage migration remain excluded.
- Do not modify licensing, privacy, support, account deletion, updater recovery, SLA, native/Rust behavior, recognition algorithms, or data transactions.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Build a nested-safe document scroll lock

**Files:**

- Create: `src/app/dialog-scroll-lock.ts`
- Create: `src/app/dialog-scroll-lock.test.ts`

**Interfaces:**

- Produces: `acquireDialogScrollLock(ownerDocument?: Document): () => void`.

- [x] **Step 1: Write nested and idempotent release tests**

Set body overflow to `auto`, acquire two locks, release the first, and assert overflow remains hidden until the second releases. Release the second twice and assert overflow remains `auto`; then set overflow to `clip`, run a fresh acquire/release cycle, and assert `clip` is restored.

```ts
const releaseFirst = acquireDialogScrollLock(document)
const releaseSecond = acquireDialogScrollLock(document)
expect(document.body.style.overflow).toBe('hidden')
releaseFirst()
expect(document.body.style.overflow).toBe('hidden')
releaseSecond()
releaseSecond()
expect(document.body.style.overflow).toBe('auto')
```

- [x] **Step 2: Run the utility suite red**

Run:

```powershell
npm test -- --run src/app/dialog-scroll-lock.test.ts
```

Expected: fail because the module does not exist.

- [x] **Step 3: Implement document-scoped lock accounting**

Create:

```ts
type ScrollLockState = { depth: number; previousOverflow: string }
const scrollLockStates = new WeakMap<Document, ScrollLockState>()

export function acquireDialogScrollLock(ownerDocument: Document = document) {
  let state = scrollLockStates.get(ownerDocument)
  if (!state) {
    state = { depth: 0, previousOverflow: ownerDocument.body.style.overflow }
    scrollLockStates.set(ownerDocument, state)
  }
  state.depth += 1
  ownerDocument.body.style.overflow = 'hidden'
  let released = false
  return () => {
    if (released) return
    released = true
    state.depth -= 1
    if (state.depth === 0) {
      ownerDocument.body.style.overflow = state.previousOverflow
      scrollLockStates.delete(ownerDocument)
    }
  }
}
```

- [x] **Step 4: Run utility tests and typecheck green**

Run the focused test and `npm run typecheck`.

### Task 2: Migrate existing direct body-style owners

**Files:**

- Modify: `src/app/components/ActionConfirmDialog.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/review/components/ReviewMediaLightbox.vue`
- Verify: corresponding component tests.

**Interfaces:**

- Consumes: `acquireDialogScrollLock()`.
- Preserves: every existing focus trap, focus return, keyboard shortcut, and emitted payload.

- [x] **Step 1: Replace direct overflow capture/restore**

In each component remove `previousBodyOverflow`/`previousOverflow`, add:

```ts
let releaseScrollLock: (() => void) | undefined
```

Acquire during mount before awaiting focus:

```ts
releaseScrollLock = acquireDialogScrollLock()
```

Release during unmount before existing focus restoration/listener cleanup:

```ts
releaseScrollLock?.()
```

- [x] **Step 2: Run the four migrated suites**

Run ActionConfirmDialog, CaptureCropEditor, CaptureRecognitionReview, and ReviewMediaLightbox tests together. Existing body restoration assertions must remain green.

### Task 3: Add shared locks to standard root modals

**Files:**

- Modify: `src/app/LibraryLockDialog.vue`
- Modify: `src/modules/legacy/components/LegacyImportDialog.vue`
- Modify: `src/modules/capture/components/CaptureLanDialog.vue`
- Modify: `src/modules/sync/components/SyncConflictBulkDialog.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Verify: corresponding component tests.

**Interfaces:**

- Consumes: `acquireDialogScrollLock()`.
- Preserves: parent-owned LAN focus restoration and drawer-local previous-focus restoration.

- [x] **Step 1: Acquire on mount and release on unmount**

Add `onBeforeUnmount` imports where missing, acquire synchronously at the start of each mount hook, and release from each unmount hook. For `ProblemDetailDrawer`, release before calling `previouslyFocused?.focus()`.

```ts
let releaseScrollLock: (() => void) | undefined
onMounted(async () => {
  releaseScrollLock = acquireDialogScrollLock()
  // existing initial-focus work
})
onBeforeUnmount(() => {
  releaseScrollLock?.()
  // existing focus return when owned here
})
```

- [x] **Step 2: Run the five standard modal suites**

Run LibraryLockDialog, LegacyImportDialog, CaptureLanDialog, SyncConflictBulkDialog, and ProblemDetailDrawer tests together.

### Task 4: Preserve responsive modality in review history

**Files:**

- Modify: `src/modules/review-history/components/ReviewHistoryDetail.vue`
- Verify: `src/modules/review-history/components/ReviewHistoryDetail.test.ts`
- Modify: `src/modules/review-history/components/ReviewHistoryDetail.mobile.test.ts`

**Interfaces:**

- Consumes: `acquireDialogScrollLock()` only in the mobile branch.
- Preserves: desktop sticky detail semantics and mobile inert-background ownership.

- [x] **Step 1: Add mobile lock/restoration assertions**

In the mobile suite set body overflow to `auto`, render, assert `hidden`, unmount, and assert `auto`. Ensure cleanup restores the test's original inline value.

- [x] **Step 2: Run the mobile suite red**

Expected: body overflow remains `auto` while the mobile dialog is mounted.

- [x] **Step 3: Acquire only after mobile detection**

Add a release handle. Inside the existing `if (mobile.value)` mount branch call `acquireDialogScrollLock()` before inerting the background; release it during unmount alongside `restoreBackground()`.

- [x] **Step 4: Run desktop and mobile suites green**

Confirm desktop does not gain dialog scroll ownership and mobile restores the exact prior value.

### Task 5: Complete the layout-impact modal lifecycle

**Files:**

- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**

- Consumes: `acquireDialogScrollLock()` and `trapDialogFocus(event, layoutImpact.value)`.
- Preserves: `applyLayout` payload and busy-state admission.

- [x] **Step 1: Add focus, Tab, Escape, scroll, and return regression**

Open “重新分组全部图片”, assert the “返回” button receives focus and body overflow is hidden; assert first/last Tab wrapping; press Escape and assert the dialog closes, overflow restores, and the launcher regains focus.

- [x] **Step 2: Run the workspace suite red**

Expected: the dialog does not focus “返回”, does not trap Tab or Escape, and does not lock body scrolling.

- [x] **Step 3: Add explicit layout modal ownership**

Add `layoutImpact`, `layoutReturnButton`, `layoutFocusReturn`, and `releaseLayoutScrollLock`. On open capture the active HTMLElement, acquire the lock, set `layoutConfirmOpen`, and focus the return button after `nextTick`. Implement `closeLayoutConfirm()` to release, close, then restore connected focus; implement a key handler with guarded Escape followed by `trapDialogFocus`. Release any outstanding lock from the component's existing `onBeforeUnmount`.

```ts
function handleLayoutImpactKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && !props.busy) {
    event.preventDefault()
    void closeLayoutConfirm()
    return
  }
  trapDialogFocus(event, layoutImpact.value)
}
```

Bind `ref`, `tabindex="-1"`, `@keydown.stop`, the return-button ref, and `@click="closeLayoutConfirm"` in the template.

- [x] **Step 4: Run the workspace suite green**

Confirm the existing exact-impact and `applyLayout` assertions still pass.

### Task 6: Complete the capture draft image modal lifecycle

**Files:**

- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.test.ts`

**Interfaces:**

- Consumes: `acquireDialogScrollLock()` and `trapDialogFocus(event, expandedDialog.value)`.
- Preserves: image selection, card flip, crop, role-change, drag, and thumbnail keyboard behavior.

- [x] **Step 1: Add open/close lifecycle regression**

Focus and click “放大查看 题目上半部分.png”, assert the modal close button receives focus and body is locked. Press Tab and assert the single close control retains focus with default prevented; press Escape and assert the overlay closes, body overflow restores, and the launcher regains focus.

- [x] **Step 2: Run the draft-card suite red**

Expected: focus remains on the launcher, body remains unlocked, and keyboard Escape from the active element does not close the overlay.

- [x] **Step 3: Implement expanded-image ownership**

Make `openImage` async, capture the connected launcher, acquire the lock, set `expanded`, and focus the close button after `nextTick`. Add `closeExpandedImage`, an Escape-first shared key handler, dialog/close refs, and unmount release cleanup.

```ts
function handleExpandedKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    void closeExpandedImage()
    return
  }
  trapDialogFocus(event, expandedDialog.value)
}
```

Route backdrop click, Escape, and close-button click through `closeExpandedImage()`.

- [x] **Step 4: Run the draft-card suite green**

Confirm all existing card interaction tests remain green.

### Task 7: Commercial-quality gates and local review

**Files:**

- Verify all files above and `docs/superpowers/plans/2026-08-03-modal-document-boundary.md`.

- [x] **Step 1: Run the complete affected suite**

Run the shared scroll-lock suite plus the 13 baseline component test files. Expected: 14 files pass with the new mobile, layout, and lightbox regressions.

- [x] **Step 2: Run final project gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 3: Review modality ownership and excluded scope**

Confirm every non-excluded root `aria-modal` surface acquires the shared lock, nested recognition impact does not double-acquire, layout/draft modal cleanup is idempotent, and direct `document.body.style.overflow` assignments remain only in the shared utility and explicitly excluded files.

- [x] **Step 4: Check workspace hygiene**

Run tracked/untracked whitespace checks, confirm the index is empty, confirm `dist/` remains ignored, and verify excluded dialog files plus the existing `recognition_visual_split.rs` modification remain untouched.

## Verification record

- Audit: twelve non-excluded root modal surfaces exist. Four directly mutate body overflow, five standard modal components have no scroll lock, mobile review history owns inert but not scroll, and the layout-impact plus capture-draft image modals lack complete focus/scroll lifecycles. The nested recognition impact dialog is covered by its outer modal.
- Baseline: 13 affected component test files passed, 93 tests.
- Red: the shared-lock suite failed before its module existed; mobile review history left body overflow `auto`; layout confirmation left focus on its launcher; capture-draft image expansion also left focus on its launcher. The latter two additionally had no shared scroll ownership or complete Escape/return lifecycle.
- Focused green: shared lock, direct-owner migrations, standard root modals, responsive history, layout confirmation, and draft image tests all passed in their focused groups.
- Affected green: the shared lock plus all 13 baseline component files passed, 14 files / 96 tests.
- Final gates: `npm run lint` and `npm run typecheck` passed; production build transformed 2050 modules; the single-worker full run passed 135 files / 758 tests.
- Local review: all twelve non-excluded root modal owners call `acquireDialogScrollLock`; `CaptureRecognitionReview` acquires exactly once for its outer and nested surfaces; no Vue component directly writes body overflow; responsive history remains conditional; layout and draft cleanup use the idempotent release contract.
- Hygiene: tracked and untracked whitespace checks reported no errors (only Git LF→CRLF notices); the index is empty; `dist/` remains ignored; excluded backup/storage dialog files are unchanged; the pre-existing `recognition_visual_split.rs` modification remains present and untouched.
