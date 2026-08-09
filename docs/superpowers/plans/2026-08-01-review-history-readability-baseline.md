# Review History Readability Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring every user-readable label, status, metadata value, and recovery action in the review-history module up to a commercial 12px text and 44px action-target baseline without breaking desktop or narrow-screen layouts.

**Architecture:** Keep semantic markup and component responsibilities unchanged. Add a module-scoped source contract that rejects explicit font sizes below 12px, then update the page, filter form, timeline, and detail sheet styles as one visual system and verify both desktop and narrow preview routes in the in-app browser.

**Tech Stack:** Vue 3 SFC scoped CSS, TypeScript, Vitest, Testing Library Vue, Vite local preview

## Global Constraints

- Do not modify Rust files or the existing `src-tauri/src/infrastructure/recognition_visual_split.rs` change.
- Do not implement launch-gate work for licensing, privacy, support operations, account deletion, device migration, update recovery, or SLA.
- Every explicit `font-size` in the review-history page, filters, timeline, and detail components must be at least 12px.
- Error recovery and explicit action buttons must retain at least a 44px target height.
- Existing desktop sticky detail, mobile modal sheet, focus restoration, focus trap, inert background, loading, stale-data, and reduced-motion behavior must remain unchanged.
- At 1280px width, filter controls, timeline metadata, rating seals, duration, and audit values must remain visible without horizontal page overflow.
- At 390px width, timeline rows must remain readable and the detail sheet must fit the viewport without horizontal overflow.
- Do not stage or commit the existing dirty worktree.

---

### Task 1: Readability Contract And Style Baseline

**Files:**
- Create: `src/modules/review-history/components/ReviewHistoryReadability.test.ts`
- Modify: `src/app/views/ReviewHistoryView.vue`
- Modify: `src/modules/review-history/components/ReviewHistoryFilters.vue`
- Modify: `src/modules/review-history/components/ReviewHistoryTimeline.vue`
- Modify: `src/modules/review-history/components/ReviewHistoryDetail.vue`

**Interfaces:**
- Consumes: the four Vue SFC sources via `node:fs` and `import.meta.url`.
- Produces: a regression test that reports every explicit `font-size: Npx` below 12 with its file name and matched declaration.

- [x] **Step 1: Add the failing source contract**

Read all four SFCs, collect `/font-size:\s*(\d+(?:\.\d+)?)px/g` matches below 12, and assert the resulting `{ file, declaration }[]` is empty. Also assert the history page contains a 44px minimum-height rule covering both `.error-banner button` and `.initial-error button`.

- [x] **Step 2: Run the contract and verify failure**

Run: `npm test -- --run src/modules/review-history/components/ReviewHistoryReadability.test.ts`

Expected: FAIL with the current 8px, 9px, 10px, and 11px declarations listed.

- [x] **Step 3: Raise the module typography and action target baseline**

Change every explicit sub-12px declaration in the four scoped style blocks to 12px. Remove the obsolete 38px retry-button declaration or raise it to 44px while retaining the existing combined 44px recovery-action rule. Do not change markup, responsive breakpoints, or behavioral code.

- [x] **Step 4: Run component and view tests**

Run: `npm test -- --run src/modules/review-history/components/ReviewHistoryReadability.test.ts src/modules/review-history/components/ReviewHistoryFilters.test.ts src/modules/review-history/components/ReviewHistoryTimeline.test.ts src/modules/review-history/components/ReviewHistoryDetail.test.ts src/modules/review-history/components/ReviewHistoryDetail.mobile.test.ts src/app/views/ReviewHistoryView.test.ts src/app/views/ReviewHistoryView.resilience.test.ts`

Expected: every focused test passes.

### Task 2: Desktop And Narrow-Screen Visual Verification

**Files:**
- Modify only Task 1 files if visual inspection finds clipping or horizontal overflow.

**Interfaces:**
- Consumes: Vite preview route `/#/report/history?preview=history`.
- Produces: verified desktop and narrow-screen layout evidence.

- [x] **Step 1: Start the local Vite server**

Run: `npm run dev -- --host 127.0.0.1`

Expected: Vite reports a local HTTP URL and remains running for browser inspection.

- [x] **Step 2: Inspect the desktop route**

At 1280px viewport width, open `/#/report/history?preview=history`, verify the filter form and timeline have no horizontal page overflow, open a history row, and verify audit labels, values, version badges, and close/retry actions remain readable.

- [x] **Step 3: Inspect the narrow route**

At 390px viewport width, reload the same route, verify two-column filters collapse correctly, timeline metadata wraps without clipping, open the detail sheet, and verify the sheet has no horizontal overflow while its close action receives focus.

- [x] **Step 4: Stop the local server**

Terminate only the Vite process started for this task.

### Task 3: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-review-history-readability-baseline.md`

**Interfaces:**
- Consumes: completed style and visual verification.
- Produces: checked task boxes and exact verification evidence.

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

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust file was edited during this task.

- [x] **Step 4: Record verification without committing**

Check every completed task box and append the exact contract red/green result, focused tests, desktop/narrow visual findings, static checks, complete suite, hygiene, index, and scope results. Do not run `git add` or `git commit`.

## Verification Record

- Contract red: the first run exposed a Vitest URL-scheme incompatibility; after changing the test-only source resolution to repository-root paths, the intended red run failed with 19 declarations below 12px across the four target SFCs while the 44px recovery-action assertion passed.
- Focused green: `npm test -- --run src/modules/review-history/components/ReviewHistoryReadability.test.ts src/modules/review-history/components/ReviewHistoryFilters.test.ts src/modules/review-history/components/ReviewHistoryTimeline.test.ts src/modules/review-history/components/ReviewHistoryDetail.test.ts src/modules/review-history/components/ReviewHistoryDetail.mobile.test.ts src/app/views/ReviewHistoryView.test.ts src/app/views/ReviewHistoryView.resilience.test.ts` passed 7 files and 12 tests.
- Desktop browser: at 1280px width, page/body horizontal overflow were both false; the filter was 921.8125px wide with 920px scroll width, the timeline was 610.4375px wide with 610px scroll width, all sampled target labels computed to 12px, the detail computed to 295.375px wide with 278px scroll width, and the close target was 44px by 44px.
- Narrow browser: at 390px by 844px, page/body horizontal overflow were both false; the filter resolved to two 153.5px columns inside 345px, the timeline row was 345px wide with 343px scroll width, the two-line note remained 12px and wrapped normally, and the detail sheet was 375px wide with 360px scroll width.
- Narrow interaction: opening a row moved focus to the 44px by 44px close button; closing the sheet removed it and restored focus to the originating history row.
- Preview server: the project-defined port 1420 was already served by an existing workspace process, so the failed attempted local start exited without creating a listener; visual verification reused the existing process and did not stop it.
- Static checks: `npm run typecheck` and `npm run lint` both exited 0.
- Complete frontend suite: `npm test -- --run` passed 101 files and 626 tests.
- Hygiene: `git diff --check` exited 0 (existing line-ending warnings only), and `git diff --cached --name-only` was empty.
- Scope: this task changed only the four review-history SFC style baselines, the new readability contract, and this plan record. No Rust file was modified by this task; all pre-existing dirty worktree changes were preserved. Nothing was staged or committed.
