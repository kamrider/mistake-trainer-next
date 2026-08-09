# Dialog Focus Policy Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the remaining non-excluded component-local focus-ring algorithms with one tested product-wide policy, including a safe empty-control fallback during busy states.

**Architecture:** Extend `src/app/dialog-focus.ts` to expose the ordered, filtered focus-ring query as `getDialogFocusableElements(container)`, and keep `trapDialogFocus(event, container)` as the sole wrap/recovery behavior. Migrate eight non-excluded modal, drawer, sheet, and lightbox consumers while preserving each component's existing Escape, arrow-key, initial-focus, return-focus, inert-background, and body-scroll ownership.

**Tech Stack:** Vue 3 Composition API, TypeScript DOM APIs, Vitest, Testing Library Vue.

## Global Constraints

- The shared query must include enabled buttons, links, inputs, selects, textareas, summaries, and non-negative explicit tab stops in document order.
- Elements with `tabindex="-1"`, or inside `[hidden]` or `[inert]` ancestors, must not enter the focus ring.
- Tab and Shift+Tab must recover focus to the correct ring edge when focus is on the container or outside the surface.
- A modal with no enabled focusable descendant must prevent Tab escape and focus its `tabindex="-1"` container.
- Preserve every component's current Escape behavior, crop shortcuts, lightbox arrow navigation, mobile-only review-history modality, unsaved-changes guard, initial focus, body overflow, background inerting, and focus restoration.
- Do not modify `BackupRestoreDialog.vue` or `StorageMigrationDialog.vue`; backup/restore and device/storage migration are excluded from this remediation scope.
- Do not modify licensing, privacy, support, account deletion, updater recovery, SLA, native/Rust behavior, recognition algorithms, or data transactions.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Expose the canonical focus-ring query

**Files:**

- Modify: `src/app/dialog-focus.ts`
- Modify: `src/app/dialog-focus.test.ts`

**Interfaces:**

- Produces: `getDialogFocusableElements(container: HTMLElement | undefined): HTMLElement[]`.
- Preserves: `trapDialogFocus(event: KeyboardEvent, container: HTMLElement | undefined): void`.

- [x] **Step 1: Add query-contract assertions**

Extend the existing utility test to call `getDialogFocusableElements(container)` and assert the exact `[first, last]` order while excluding the disabled button, `[href][tabindex="-1"]`, hidden descendant, and inert descendant.

```ts
expect(getDialogFocusableElements(container)).toEqual([first, last])
```

- [x] **Step 2: Run the utility suite red**

Run:

```powershell
npm test -- --run src/app/dialog-focus.test.ts
```

Expected: fail because `getDialogFocusableElements` is not exported.

- [x] **Step 3: Extract and reuse the shared query**

Add:

```ts
export function getDialogFocusableElements(container: HTMLElement | undefined) {
  if (!container) return []
  return Array.from(
    container.querySelectorAll<HTMLElement>(dialogFocusableSelector),
  ).filter(element =>
    element.getAttribute('tabindex') !== '-1'
    && !element.closest('[hidden], [inert]'),
  )
}
```

Then replace the inline query inside `trapDialogFocus` with:

```ts
const focusable = getDialogFocusableElements(container)
```

- [x] **Step 4: Run utility tests and typecheck green**

Run the utility suite and `npm run typecheck`.

### Task 2: Prove and fix the busy-state empty ring

**Files:**

- Modify: `src/modules/legacy/components/LegacyImportDialog.test.ts`
- Modify: `src/modules/legacy/components/LegacyImportDialog.vue`

**Interfaces:**

- Consumes: `trapDialogFocus(event, panel.value)`.
- Preserves: `cancel` and `confirm` emits plus acknowledgement gating.

- [x] **Step 1: Add a failing all-disabled regression**

Render with `busy: true`, dispatch a cancelable Tab event on the dialog, and assert that the event is prevented and the dialog receives focus.

```ts
const dialog = screen.getByRole('dialog')
const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true })
dialog.dispatchEvent(tab)
expect(tab.defaultPrevented).toBe(true)
expect(dialog).toHaveFocus()
```

- [x] **Step 2: Run the legacy suite red**

Run:

```powershell
npm test -- --run src/modules/legacy/components/LegacyImportDialog.test.ts
```

Expected: Tab is not prevented and the untabbable section does not receive focus.

- [x] **Step 3: Adopt the shared trap and container fallback**

Import the utility, keep the Escape branch, replace the local query/wrap algorithm with:

```ts
trapDialogFocus(event, panel.value)
```

Add `tabindex="-1"` to the dialog section so the empty-ring fallback is focusable.

- [x] **Step 4: Run the legacy suite green**

Run the focused legacy test and typecheck.

### Task 3: Migrate standard dialog consumers

**Files:**

- Modify: `src/app/LibraryLockDialog.vue`
- Modify: `src/modules/capture/components/CaptureLanDialog.vue`
- Modify: `src/modules/sync/components/SyncConflictBulkDialog.vue`
- Verify: corresponding `.test.ts` files.

**Interfaces:**

- Consumes: `trapDialogFocus(event, container)` from `src/app/dialog-focus.ts`.
- Preserves: busy-state cancellation admission, LAN state-refresh focus recovery, and sync bulk-choice payloads.

- [x] **Step 1: Replace three local algorithms**

In each component, import the helper from its relative `app/dialog-focus` path. Keep the Escape branch and replace every local selector/first/last block with the relevant call:

```ts
trapDialogFocus(event, panel.value)
trapDialogFocus(event, dialog.value)
```

- [x] **Step 2: Run the three dialog suites**

Run LibraryLockDialog, CaptureLanDialog, and SyncConflictBulkDialog tests together. Expected: all existing interaction and payload assertions remain green.

### Task 4: Migrate complex modal surfaces without changing lifecycle ownership

**Files:**

- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/review-history/components/ReviewHistoryDetail.vue`
- Modify: `src/modules/review/components/ReviewMediaLightbox.vue`
- Verify: corresponding `.test.ts` files.

**Interfaces:**

- Consumes: `trapDialogFocus` and, for initial drawer focus, `getDialogFocusableElements`.
- Preserves: crop keyboard commands, nested ActionConfirm event isolation, mobile-only history sheet modality, lightbox arrow navigation, body locks, background inerting, and return focus.

- [x] **Step 1: Migrate the crop editor Tab branch**

Keep Escape, Space, arrow, delete, and undo/redo handling unchanged. Replace only the `event.key === 'Tab'` selector/wrap block with:

```ts
if (event.key === 'Tab') trapDialogFocus(event, dialog.value)
```

- [x] **Step 2: Migrate the problem drawer query and trap**

Replace `focusableElements()` with shared querying during mount:

```ts
getDialogFocusableElements(drawer.value)[0]?.focus()
```

Replace the local Tab block with `trapDialogFocus(event, drawer.value)` while keeping asynchronous guarded Escape close and previous-focus restoration.

- [x] **Step 3: Migrate the responsive history detail**

Keep the global key listener and `mobile.value` condition. Replace only the mobile Tab query/wrap block with:

```ts
if (mobile.value) trapDialogFocus(event, detailLayer.value)
```

Do not add desktop trapping, body lock, or desktop dialog semantics.

- [x] **Step 4: Migrate the review media lightbox**

Keep Escape and ArrowLeft/ArrowRight branches before the shared call, then use:

```ts
trapDialogFocus(event, dialog.value)
```

Retain its document listener, body overflow restoration, and return-focus behavior.

- [x] **Step 5: Run the four complex-surface suites**

Run CaptureCropEditor, ProblemDetailDrawer, ReviewHistoryDetail, and ReviewMediaLightbox tests together.

### Task 5: Commercial-quality gates and local review

**Files:**

- Verify all files above and `docs/superpowers/plans/2026-08-03-dialog-focus-policy-consolidation.md`.

- [x] **Step 1: Run the complete affected suite**

Run the shared utility plus all eight consumer suites. Expected: 9 files pass, with the new busy-state regression included.

- [x] **Step 2: Run final project gates**

Run:

```powershell
npm run lint
npm run typecheck
npm run build
npm test -- --run --maxWorkers=1
```

- [x] **Step 3: Review behavior and excluded scope**

Search for `querySelectorAll<HTMLElement>` in non-excluded modal surfaces. Confirm the eight consumers use the shared policy; Escape/arrow/shortcut branches still precede the trap; mobile history trapping remains conditional; and backup/storage migration files are unchanged.

- [x] **Step 4: Check workspace hygiene**

Run tracked and untracked whitespace checks, confirm the index is empty, confirm `dist/` remains ignored, and verify the existing `recognition_visual_split.rs` modification remains present and untouched.

## Verification record

- Audit: ten local focus-ring queries remained after the first shared-utility batch. Two belong to explicitly excluded backup/storage migration flows; eight non-excluded consumers are in scope.
- Baseline: the eight consumer suites passed, 8 files / 36 tests.
- Red: the shared-query contract failed because `getDialogFocusableElements` was not exported; the legacy busy-state regression failed because Tab was not prevented when every control was disabled.
- Focused green: the shared utility plus all eight migrated consumers passed, 9 files / 39 tests; focused standard-dialog and complex-surface groups also passed independently.
- Final gates: `npm run lint` and `npm run typecheck` passed; production build transformed 2049 modules; the single-worker full run passed 134 files / 755 tests.
- Local review: only the two explicitly excluded backup/storage migration components retain local focus queries. All eight in-scope consumers use the shared policy while retaining Escape, crop shortcuts, lightbox arrows, guarded drawer close, mobile-only history trapping, body overflow, inert background, and return-focus ownership.
- Hygiene: tracked and untracked whitespace checks reported no errors (only Git LF→CRLF notices); the index is empty; `dist/` remains ignored; excluded `BackupRestoreDialog.vue` and `StorageMigrationDialog.vue` are unchanged; the pre-existing `recognition_visual_split.rs` modification remains present and untouched.
