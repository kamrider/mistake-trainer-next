# Storage Migration Modal Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the storage-migration confirmation behave like every other commercial modal by owning a document boundary, recovering escaped keyboard focus, and meeting the 44px/12px interaction baseline.

**Architecture:** `StorageMigrationDialog` will acquire and release the existing `dialog-document-boundary` while mounted and delegate Tab wrapping to `trapDialogFocus`. `SettingsView` keeps its existing post-close fallback to the current migration trigger, so rerendered controls remain recoverable without changing storage commands or restart behavior.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, existing dialog boundary utilities.

## Global Constraints

- Do not change storage migration commands, persistence, restart behavior, or public bindings.
- Do not implement device migration/recovery policy or any other deferred pre-launch work.
- Preserve unrelated dirty-worktree changes and do not edit `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this shared worktree.

---

### Task 1: Prove the missing modal document boundary

**Files:**
- Modify: `src/app/StorageMigrationDialog.test.ts`

**Interfaces:**
- Consumes: `StorageMigrationDialog`, real `document.body`, and Testing Library focus assertions.
- Produces: regression coverage for background inert, scroll lock, outside-focus recovery, and unmount cleanup.

- [x] **Step 1: Add a failing document-boundary test**

Attach a real launcher button to `document.body`, render the dialog, and assert:

```ts
expect(launcher).toHaveAttribute('inert')
expect(document.body.style.overflow).toBe('hidden')
launcher.focus()
await fireEvent.keyDown(dialog, { key: 'Tab' })
expect(cancelButton).toHaveFocus()
view.unmount()
expect(launcher).not.toHaveAttribute('inert')
expect(document.body.style.overflow).toBe(previousOverflow)
```

- [x] **Step 2: Run the focused test and verify failure**

Run: `pnpm exec vitest run src/app/StorageMigrationDialog.test.ts`

Expected: FAIL because the dialog does not currently acquire the shared document boundary.

### Task 2: Adopt the shared boundary and interaction baseline

**Files:**
- Modify: `src/app/StorageMigrationDialog.vue`
- Test: `src/app/StorageMigrationDialog.test.ts`

**Interfaces:**
- Consumes: `acquireDialogDocumentBoundary(panel)` and `trapDialogFocus(event, panel)`.
- Produces: idempotent cleanup on unmount, document-level background isolation, scroll ownership, and common focus-ring behavior.

- [x] **Step 1: Replace the local Tab implementation**

Import `onBeforeUnmount`, `acquireDialogDocumentBoundary`, and `trapDialogFocus`. Acquire the boundary after `panel` mounts, release it exactly once in `onBeforeUnmount`, keep Escape/busy behavior unchanged, and delegate all Tab behavior to `trapDialogFocus`.

- [x] **Step 2: Raise control and copy sizing**

Set `.close-button` to `44px` square, `.dialog-actions button` to `min-height: 44px`, and modal helper copy currently at `11px` to `12px` without changing hierarchy or wording.

- [x] **Step 3: Run focused dialog and Settings tests**

Run: `pnpm exec vitest run src/app/StorageMigrationDialog.test.ts src/app/views/SettingsView.test.ts`

Expected: PASS, including existing safe cancellation and focus return through `SettingsView`.

### Task 3: Verification and review

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-storage-migration-modal-boundary.md`

**Interfaces:**
- Consumes: completed implementation.
- Produces: verified and independently reviewed batch.

- [x] **Step 1: Run frontend gates**

Run `pnpm lint`, `pnpm typecheck`, `pnpm build`, and `pnpm test`.

Expected: all exit 0.

- [x] **Step 2: Run `git diff --check` for the changed files**

Expected: no whitespace errors.

- [x] **Step 3: Request independent code review**

Review nested modal depth behavior, cleanup on unmount, busy focus, parent focus fallback, and touch/readability changes. Fix every Critical or Important issue and rerun affected checks.

- [x] **Step 4: Mark every checkbox complete**

Only after focused tests, full gates, and follow-up review pass.
