# Backup and Legacy Safety Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make backup restore and legacy migration safety flows use the shared modal boundary and remain legible and touch-safe at every viewport width.

**Architecture:** Move `BackupRestoreDialog.vue` onto the existing `acquireDialogDocumentBoundary` and `trapDialogFocus` primitives already used by the library-lock and legacy-import dialogs. Enforce the modal lifecycle and the 12 px/44 px safety UI baseline with focused Vitest behavior and source contracts; keep restore/import/rollback commands, acknowledgement rules, receipts, and emitted events unchanged.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Testing Library, Vitest.

## Global Constraints

- Every explicit visible pixel font in `BackupRestoreDialog.vue`, `LegacyImportDialog.vue`, `LegacyImportPanel.vue`, and `LegacyImportResult.vue` must be at least 12 px.
- Backup close and action buttons, legacy dialog close and action buttons, legacy panel buttons, and legacy result actions must provide at least a 44 px target.
- Backup restore must acquire and release the shared scroll-lock/background-inert boundary, trap focus with the shared helper, focus the safe cancel action first, focus the dialog itself while every control is disabled, and restore prior focus on unmount.
- Do not change restore/import/rollback commands, acknowledgement requirements, candidate validation, progress, history refresh, receipt contents, or emitted events.
- Do not change storage migration, updater recovery, account deletion, licensing, privacy policy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes, including existing legacy component changes, and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies or stage/commit files.

---

### Task 1: Lock backup restore modal ownership

**Files:**
- Create: `src/app/BackupRestoreDialog.test.ts`
- Modify: `src/app/BackupRestoreDialog.vue:1-147`

**Interfaces:**
- Consumes: `acquireDialogDocumentBoundary(modalRoot: HTMLElement)` and `trapDialogFocus(event, container)`.
- Produces: one release callback owned by the dialog mount lifecycle; unchanged `cancel` and `confirm` events.

- [x] **Step 1: Write failing modal lifecycle tests**

Add a real component test that creates an outside trigger, verifies initial cancel focus, background inert and body scroll lock, unmount cleanup and focus return, plus a second test that rerenders `busy: true` and expects the dialog boundary to own focus.

- [x] **Step 2: Run the backup dialog test and verify red**

Run: `npm test -- src/app/BackupRestoreDialog.test.ts`

Expected: lifecycle and busy-focus assertions fail because the dialog still owns a private partial trap and never acquires the shared document boundary.

- [x] **Step 3: Adopt the shared boundary and focus policy**

Import `onBeforeUnmount`, `watch`, `acquireDialogDocumentBoundary`, and `trapDialogFocus`; acquire the boundary on mount, focus cancel when idle or the dialog when busy, focus the dialog when `busy` becomes true, release and return prior focus on unmount, delegate Tab handling to `trapDialogFocus`, and add `tabindex="-1"` to the dialog section.

- [x] **Step 4: Run the backup dialog test and verify green**

Run: `npm test -- src/app/BackupRestoreDialog.test.ts`

Expected: all backup dialog behavior tests pass.

### Task 2: Lock the safety-flow readability contract

**Files:**
- Create: `src/app/BackupLegacySafetyReadability.test.ts`
- Modify: `src/app/BackupRestoreDialog.vue:129-147`
- Modify: `src/modules/legacy/components/LegacyImportDialog.vue:140-141`
- Modify: `src/modules/legacy/components/LegacyImportPanel.vue:423-425`
- Modify: `src/modules/legacy/components/LegacyImportResult.vue:52-53`

**Interfaces:**
- Consumes: scoped CSS selectors in the four safety-flow components.
- Produces: a regression contract for the 12 px text floor and named 44 px controls.

- [x] **Step 1: Write and run the failing readability contract**

Run: `npm test -- src/app/BackupLegacySafetyReadability.test.ts`

Expected: the font-floor and backup touch-target assertions fail; existing legacy action targets remain green.

- [x] **Step 2: Raise backup restore to the safety baseline**

Set `.close-button` to 44 by 44 px, `.dialog-actions button` to at least 44 px, and all 9/11 px backup labels and guidance to 12 px.

- [x] **Step 3: Raise legacy migration copy to the safety baseline**

Raise the legacy confirmation eyebrow, candidate metadata, migration statistics, issue details, history metadata, and result copy from 10/11 px to 12 px. Do not alter selectors, layout, animation, or behavior.

- [x] **Step 4: Run focused behavior and contract suites**

Run: `npm test -- src/app/BackupRestoreDialog.test.ts src/app/BackupLegacySafetyReadability.test.ts src/modules/legacy/components/LegacyImportDialog.test.ts src/modules/legacy/components/LegacyImportPanel.test.ts`

Expected: all focused files and tests pass, including confirmation, focus return, progress, rollback, history resilience, and privacy/safety copy behavior.

### Task 3: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-backup-legacy-safety-boundary.md`

- [x] **Step 1: Run static and production validation**

Run `npm run lint`, `npm run typecheck`, and `npm run build`.

- [x] **Step 2: Run the full frontend suite**

Run: `npm test`

- [x] **Step 3: Perform local code review and record completion**

Review the shared-boundary lifecycle, busy focus, restoration order, selectors, legacy dirty-worktree scope, excluded features, whitespace for untracked files, and staged state. Resolve every Critical or Important finding, check every plan item, and append exact verification counts.

## Verification Record

- Red phase: 2 test files ran; 4 tests failed and 2 passed as expected. The failures proved missing document-boundary ownership, missing busy-state focus, 12 explicit font declarations below 12 px, and the 36 px backup close control.
- Focused regression: 4 test files passed, 13 tests passed. After aligning the backup fixture with `BackupSummary.readyForRestore`, the 2 newly added files also passed independently with 6 tests.
- Static validation: `npm run lint` passed with zero warnings. The first `npm run typecheck` correctly rejected the incomplete test fixture; after adding the required field, type checking passed with zero errors.
- Production build: `npm run build` passed; Vite transformed 2054 modules.
- Full frontend regression: 146 test files passed, 796 tests passed.
- Local code review: no Critical or Important findings. The backup dialog acquires and releases the shared scroll/inert boundary, delegates Tab behavior to the shared trap, owns focus while busy, restores the prior trigger after releasing the boundary, and keeps acknowledgement/confirmation semantics unchanged.
- Readability review: all four safety-flow components have a 12 px explicit font floor. Backup close/actions and all named legacy actions retain at least 44 px targets; responsive layouts and reduced-motion behavior remain unchanged.
- Scope review: existing legacy import history-resilience changes were preserved; this batch changed only their existing CSS font declarations. Storage migration, updater recovery, account deletion, launch-only work, and `src-tauri/src/infrastructure/recognition_visual_split.rs` were not changed by this batch.
- Hygiene: tracked target files passed `git diff --check`; no-index checks on new files reported no whitespace errors (only LF-to-CRLF conversion warnings). No files were staged or committed.
