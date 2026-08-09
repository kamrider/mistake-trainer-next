# Export Snapshot Mutation Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make export-snapshot deletion and restoration one shared single-flight transaction so their list updates and recycle-bin refreshes cannot overwrite each other.

**Architecture:** Extract delete confirmation, command execution, recycle-bin refresh, restore execution, list updates, errors, and sync scheduling from `ReportView.vue` into `useExportSnapshotMutations`. The controller owns `deletingId`, `restoringId`, and a shared `mutationBusy`; `ExportSnapshotHistory.vue` uses that shared state to disable every conflicting delete and restore control while identifying the active row.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri command inputs and generated bindings unchanged.
- Preserve the existing recoverable-delete confirmation copy and 30-day recycle-bin model.
- Do not block snapshot file generation, candidate loading, or snapshot creation unless they conflict with delete/restore state.

---

### Task 1: Export Snapshot Mutation Controller

**Files:**
- Create: `src/modules/export/composables/useExportSnapshotMutations.ts`
- Create: `src/modules/export/composables/useExportSnapshotMutations.test.ts`

**Interfaces:**
- Consumes: live snapshot/deleted-snapshot getters, list/error/trash-loaded callbacks, delete confirmation, delete/restore/trash-list operations, and sync scheduling.
- Produces: readonly `deletingId`, `restoringId`, computed `mutationBusy`, `deleteSnapshot(snapshotId)`, and `restoreSnapshot(deleted)`.

- [x] **Step 1: Write failing controller tests**

Cover busy state during confirmation and commands, cancellation, exact command IDs, duplicate/conflicting mutation rejection, successful list ordering, sync scheduling, recycle-bin refresh, application failures, thrown failures, and a successful delete whose recycle-bin refresh fails.

- [x] **Step 2: Run the focused controller test and verify RED**

Run: `pnpm vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts`

Expected: FAIL because `useExportSnapshotMutations.ts` does not exist.

- [x] **Step 3: Implement the minimal controller**

Set `deletingId` before awaiting confirmation so the full transaction is single-flight. Return on cancellation or stale targets. On delete success remove the snapshot, schedule sync, then replace the recycle-bin list only from the matching awaited refresh; use `快照已删除，但回收区暂时没有刷新成功。` if that refresh fails. On restore success prepend the restored snapshot, remove its deleted entry, and schedule sync. Always release the active ID in `finally` and preserve existing fallback copy.

- [x] **Step 4: Run the focused controller test and verify GREEN**

Run: `pnpm vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts`

Expected: PASS.

### Task 2: Shared Mutation State in Snapshot History

**Files:**
- Modify: `src/modules/export/components/ExportSnapshotHistory.vue`
- Modify: `src/modules/export/components/ExportSnapshotHistory.test.ts`

**Interfaces:**
- Consumes: required `mutationBusy: boolean` plus existing active IDs.
- Produces: all delete and restore buttons disabled during a mutation, with active labels `正在删除导出快照：<title>` and `正在恢复导出快照：<title>`.

- [x] **Step 1: Write a failing component regression test**

Render two active snapshots and one deleted snapshot with a delete active. Assert both delete buttons and the restore button are disabled, while the active delete button exposes the progress label. Rerender with restore active and assert all conflicting controls remain disabled with the restore progress label.

- [x] **Step 2: Run the focused component test and verify RED**

Run: `pnpm vitest run src/modules/export/components/ExportSnapshotHistory.test.ts`

Expected: FAIL because only the active row is currently disabled.

- [x] **Step 3: Implement the shared presentation state**

Add `mutationBusy`, use it for every delete and restore button, and derive active-row accessible labels from the matching IDs without changing generation behavior.

- [x] **Step 4: Run the focused component test and verify GREEN**

Run: `pnpm vitest run src/modules/export/components/ExportSnapshotHistory.test.ts`

Expected: PASS.

### Task 3: Report View Integration and Full Gates

**Files:**
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: `useExportSnapshotMutations`, existing confirmation controller, normalized Tauri commands, and shared sync controller.
- Produces: unchanged view events with single-flight delete/restore semantics and `mutationBusy` passed to history.

- [x] **Step 1: Write a failing integration regression test**

Confirm a deletion backed by a deferred `exportDelete`, assert the recycle-bin restore button is disabled and cannot call `exportRestore`, then resolve deletion and verify one sync schedule plus the refreshed recycle-bin state.

- [x] **Step 2: Run the focused view test and verify RED**

Run: `pnpm vitest run src/app/views/ReportView.test.ts`

Expected: FAIL because restore remains enabled while deletion is pending.

- [x] **Step 3: Replace inline delete/restore functions with the controller**

Instantiate the controller with live refs, confirmation copy, normalized command operations, and sync scheduling. Remove inline delete/restore bodies, pass `mutationBusy`, and retain the existing history event names.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `pnpm vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts src/modules/export/components/ExportSnapshotHistory.test.ts src/app/views/ReportView.test.ts`

Expected: PASS.

- [x] **Step 5: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0; the new controller has complete statement/function/line coverage; production build succeeds.

- [x] **Step 6: Review the final diff without committing**

Run `git diff --check` for modified tracked files, inspect the new controller and plan, and verify all scoped files remain unstaged.

Expected: no whitespace errors, no unrelated edits, and the existing dirty worktree remains unstaged.
