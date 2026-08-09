# Report Refresh And Selection Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make report refreshes single-flight and preserve the user's export-candidate selection plus the last authoritative snapshot state across same-source refreshes and transient failures.

**Architecture:** Move candidate source, loading, reconciliation, and selection rules into a focused `useExportCandidateSelection` composable. Keep report/snapshot orchestration in `ReportView.vue`, add a synchronous non-reactive refresh latch, and only replace authoritative loaded flags after successful reads.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue

## Global Constraints

- Do not modify Rust files or the existing `src-tauri/src/infrastructure/recognition_visual_split.rs` change.
- Do not implement launch-gate work for licensing, privacy, support operations, account deletion, device migration, update recovery, or SLA.
- A same-source refresh must preserve visible candidates and explicit selection while pending and after failure.
- A successful same-source refresh must retain only selected IDs still returned; new IDs remain unselected.
- An initial load or real source change selects all successfully returned candidates.
- Full report refresh must be synchronously single-flight, including two clicks in the same event-loop turn.
- A failed refresh must preserve the last successful snapshot/trash data and loaded flags.
- Do not stage or commit the existing dirty worktree.

---

### Task 1: Candidate Selection Lifecycle

**Files:**
- Create: `src/modules/export/composables/useExportCandidateSelection.ts`
- Create: `src/modules/export/composables/useExportCandidateSelection.test.ts`

**Interfaces:**
- Consumes: `ExportCandidate`, `ExportCandidateSource`, and an injected `load(source)` operation returning `AppResult<ExportCandidate[]>`.
- Produces: `source`, `candidates`, `selectedIds`, `loading`, `error`, `loadCandidates(source?)`, `changeSource(source)`, `replacePreview(source, items)`, `toggle(problemId)`, `select(problemIds)`, and `clear()`.

- [x] **Step 1: Write failing lifecycle tests**

Cover these exact scenarios: first success selects all; same-source pending and failure retain candidates/selection; same-source success intersects the old explicit selection with returned IDs; a source change clears the old source and selects all new results; a second in-flight call returns without invoking the operation.

- [x] **Step 2: Run the focused test and verify failure**

Run: `npm test -- --run src/modules/export/composables/useExportCandidateSelection.test.ts`

Expected: FAIL because `useExportCandidateSelection.ts` does not exist.

- [x] **Step 3: Implement the lifecycle controller**

Use a synchronous `inFlight` boolean in addition to the reactive loading state. Track the last successfully loaded source separately. On same-source success assign `selectedIds` by filtering the returned candidate order against the previous selected-ID set; on first/source-replacement success select every returned ID. Normalize thrown failures to `可导出的题目没有读取成功，请稍后重试。` without clearing same-source state.

- [x] **Step 4: Run the focused test and verify success**

Run: `npm test -- --run src/modules/export/composables/useExportCandidateSelection.test.ts`

Expected: all candidate lifecycle tests PASS.

### Task 2: Report Refresh Integration

**Files:**
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: `useExportCandidateSelection({ load })` from Task 1.
- Produces: a refresh action that returns early while a refresh, candidate load, or durable export mutation is already active.

- [x] **Step 1: Add failing report integration tests**

Add tests proving that two direct same-turn refresh clicks invoke each native read only once; candidate rows and a deliberate deselection remain visible during a same-source refresh and after its failure; a successful same-source refresh does not select new or previously deselected IDs; and failed snapshot/trash refreshes keep prior successful empty-state copy.

- [x] **Step 2: Run the report test and verify failure**

Run: `npm test -- --run src/app/views/ReportView.test.ts`

Expected: the new concurrency, selection-retention, and loaded-state tests FAIL against the current implementation.

- [x] **Step 3: Integrate the controller and refresh latch**

Replace page-local candidate request/state code with the composable. Add `let reportRefreshInFlight = false`; set it before the first await and clear it in `finally`. Do not reset `snapshotsLoaded` or `trashLoaded` at refresh start. Disable full refresh while candidate loading, route preview candidates through `replacePreview`, and route clear/toggle/select events through the composable actions.

- [x] **Step 4: Run report and export tests**

Run: `npm test -- --run src/app/views/ReportView.test.ts src/modules/export/composables/useExportCandidateSelection.test.ts src/modules/export/components/ExportCandidatePicker.test.ts src/modules/export/components/ExportSnapshotHistory.test.ts`

Expected: all focused tests PASS.

### Task 3: Quality Gate And Plan Record

**Files:**
- Modify: `docs/superpowers/plans/2026-07-31-report-refresh-selection-resilience.md`

**Interfaces:**
- Consumes: completed implementation from Tasks 1-2.
- Produces: checked task boxes and a verification record with exact command outcomes.

- [x] **Step 1: Run static checks**

Run: `npm run typecheck`

Run: `npm run lint`

Expected: both commands exit 0.

- [x] **Step 2: Run the complete frontend suite**

Run: `npm test -- --run`

Expected: every test file and test passes.

- [x] **Step 3: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm the pre-existing OCR file was not edited during this task.

- [x] **Step 4: Record verification without committing**

Check every completed task box and append the exact test, typecheck, lint, and hygiene results. Do not run `git add` or `git commit`.

## Verification Record

- Candidate lifecycle red phase: failed because `useExportCandidateSelection.ts` did not exist.
- Report integration red phase: 4 new tests failed against duplicate refresh, selection reset, and loaded-state reset behavior.
- Focused green phase: 4 test files passed, 30 tests passed.
- Typecheck: `npm run typecheck` exited 0.
- Lint: `npm run lint` exited 0 with zero warnings.
- Complete frontend suite: 99 test files passed, 617 tests passed.
- Patch hygiene: `git diff --check` exited 0; existing LF-to-CRLF notices were warnings only.
- Git index: `git diff --cached --name-only` returned no files.
- Scope: no Rust files were edited during this task; the pre-existing `recognition_visual_split.rs` modification was left untouched.
