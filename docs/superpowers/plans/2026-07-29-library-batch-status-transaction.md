# Library Batch Status Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make library batch archive, trash, and restore actions single-flight, selection-safe, and visibly in progress.

**Architecture:** Extract the async batch-status transaction from `LibraryView.vue` into a focused composable that captures the submitted IDs, owns the transaction boundary through callbacks, and removes only successfully processed IDs from the latest selection. Keep presentation state in `LibraryWorkspace.vue`, where all conflicting bulk controls share one disabled/busy condition and the active action exposes progress text.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri command inputs and generated bindings unchanged.
- Use test-driven development and retain the current stable Chinese fallback copy.

---

### Task 1: Batch Status Transaction Controller

**Files:**
- Create: `src/modules/library/composables/useLibraryBatchStatus.ts`
- Create: `src/modules/library/composables/useLibraryBatchStatus.test.ts`

**Interfaces:**
- Consumes: `ProblemStatusFilter`, `ProblemStatusInput`, and `AppResult<number>`; callbacks for current selection, busy state, errors, refresh, sync, and the command operation.
- Produces: `useLibraryBatchStatus(options)` returning `changeBatchStatus(targetStatus: ProblemStatusFilter): Promise<void>`.

- [x] **Step 1: Write failing controller tests**

Cover exact captured command input, busy transitions, duplicate-call rejection, durable success ordering, removal of only submitted IDs from the latest selection, recoverable command errors, and thrown-command fallback copy.

- [x] **Step 2: Run the focused controller test and verify RED**

Run: `pnpm vitest run src/modules/library/composables/useLibraryBatchStatus.test.ts`

Expected: FAIL because `useLibraryBatchStatus.ts` does not exist.

- [x] **Step 3: Implement the minimal controller**

Capture `const requestedIds = [...options.selectedProblemIds()]` before awaiting, return when it is empty or already busy, set busy and clear the error, then call:

```ts
options.operation({ problemIds: requestedIds, targetStatus })
```

On success, schedule sync, replace selection with `options.selectedProblemIds().filter(id => !requestedIds.includes(id))`, and refresh. On an application error keep the selection and show `result.error.userMessage`; on a thrown error show `批量操作没有完成，请稍后重试。`; always clear busy in `finally`.

- [x] **Step 4: Run the focused controller test and verify GREEN**

Run: `pnpm vitest run src/modules/library/composables/useLibraryBatchStatus.test.ts`

Expected: PASS.

### Task 2: Busy-State Presentation

**Files:**
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`

**Interfaces:**
- Consumes: optional prop `changingBatchStatus?: ProblemStatusFilter | null`.
- Produces: disabled conflicting batch controls and progress labels `正在归档…`, `正在移入回收站…`, or `正在恢复学习…`.

- [x] **Step 1: Write a failing component test**

Rerender an active selection with `changingBatchStatus: 'trashed'`; assert the status button is named `正在移入回收站…`, contains a loader, and all training, status, clear-selection, select-all, and batch-management controls are disabled.

- [x] **Step 2: Run the focused component test and verify RED**

Run: `pnpm vitest run src/modules/library/components/LibraryWorkspace.test.ts`

Expected: FAIL because the busy prop does not yet affect the controls or label.

- [x] **Step 3: Implement the minimal presentation state**

Add `changingBatchStatus`, compute a shared `batchInteractionBusy` from it and `startingExperience`, use it for every conflicting control, and render `LoaderCircle` plus the target-specific progress label only on the active status action.

- [x] **Step 4: Run the focused component test and verify GREEN**

Run: `pnpm vitest run src/modules/library/components/LibraryWorkspace.test.ts`

Expected: PASS.

### Task 3: View Integration and Regression Gate

**Files:**
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`

**Interfaces:**
- Consumes: `useLibraryBatchStatus` and `changingBatchStatus` presentation prop.
- Produces: one command per user transaction, selection-safe completion, sync scheduling, and list refresh.

- [x] **Step 1: Write a failing integration regression test**

Use a deferred `problemChangeStatus` result, select one problem, click `移入回收站`, verify the button becomes `正在移入回收站…` and cannot issue a second command, then resolve success and verify one sync schedule and refresh.

- [x] **Step 2: Run the focused view test and verify RED**

Run: `pnpm vitest run src/app/views/LibraryView.test.ts`

Expected: FAIL because the view has no batch-status busy state.

- [x] **Step 3: Replace the inline transaction with the controller**

Add `changingBatchStatus`, instantiate `useLibraryBatchStatus` with live selection callbacks and normalized command operation, remove the inline `changeBatchStatus`, and pass `changingBatchStatus` to `LibraryWorkspace`.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `pnpm vitest run src/modules/library/composables/useLibraryBatchStatus.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/app/views/LibraryView.test.ts`

Expected: PASS.

- [x] **Step 5: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0; no coverage regression in the new controller; production build succeeds.

- [x] **Step 6: Review the final diff without committing**

Run: `git diff --check` and inspect `git diff` only for the files in this plan.

Expected: no whitespace errors, no unrelated edits, and the existing dirty worktree remains unstaged.
