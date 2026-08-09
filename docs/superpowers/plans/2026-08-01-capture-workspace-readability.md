# Capture Workspace Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the complete capture workspace—from batch list through organizing and commit controls—commercially readable and touch-friendly at desktop and 390px widths.

**Architecture:** Add one exact development/browser mode that reuses the existing pure fixture but shows its batch-list state, enabling visual coverage of both workspace branches without native calls. Enforce a component-wide 12px minimum for explicit visible type and 44px minimum targets for every real workspace action through a source contract, then change CSS only; leave domain and event behavior untouched.

**Tech Stack:** Vue 3, TypeScript, Vue Router, Vitest, Testing Library Vue, Vite browser preview, scoped CSS

## Global Constraints

- Do not modify Rust, native commands, capture transactions, drag/drop behavior, persistence, recognition behavior, crop behavior, or desktop initialization.
- The batch-list preview must run only when `import.meta.env.DEV`, `!isTauri()`, and `route.query.preview === 'capture-batches'` are all true.
- Reuse `createCaptureDevelopmentPreview`; do not read files, call native APIs, or persist preview interactions.
- Every explicit visible pixel font size in `CaptureWorkspace.vue` must be at least 12px.
- Every real button, select, and single-line input in the workspace must expose at least a 44px target. Decorative icons, progress bars, status dots, drag ghosts, and passive badges may remain visually smaller.
- Preserve current information hierarchy, copy, keyboard behavior, dialogs, focus handling, responsive breakpoints, reduced-motion behavior, and all emitted event contracts.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.
- Preserve the dirty worktree; do not stage or commit.

---

### Task 1: Workspace Readability And Target Contract

**Files:**
- Create: `src/modules/capture/components/CaptureWorkspaceReadability.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`

**Interfaces:**
- Consumes: the workspace SFC source from the repository root.
- Produces: a contract rejecting explicit font sizes below 12px and asserting 44px action/input rules.

- [x] **Step 1: Add the failing source contract**

Read and compact `CaptureWorkspace.vue`. Assert that every explicit pixel `font-size` is at least 12px. Assert `button { min-height:44px }`, `input,select { min-height:44px }`, and exact 44px rules for `.batch-menu-button`; assert 44px minimums for the shared primary action rule, batch menu, back button, layout-impact actions, recognition-result action, subject option/confirm actions, pair-suggestion actions, and material actions.

- [x] **Step 2: Run the contract and verify red**

Run: `npm test -- --run src/modules/capture/components/CaptureWorkspaceReadability.test.ts`

Expected: FAIL with existing 9px, 10px, and 11px type plus 32–43px actions.

- [x] **Step 3: Raise visible type and action geometry**

Change every explicit 9px, 10px, or 11px workspace font size to 12px. Add a scoped 44px baseline for buttons and single-line controls, then raise every more-specific undersized rule named by the contract. Keep the 42px passive `.round-icon`, 8px status/progress marks, passive badges, and drag ghost dimensions unchanged.

- [x] **Step 4: Run focused workspace tests**

Run: `npm test -- --run src/modules/capture/components/CaptureWorkspaceReadability.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureThumbnail.test.ts`

Expected: all focused suites pass.

### Task 2: Safe Batch-List Browser Preview

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Test: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Extends: `DevelopmentCapturePreviewMode` with `'capture-batches'`.
- Produces: the existing preview batches with `detail` cleared only for that exact mode; `capture-card` and `crop-editor` behavior remains unchanged.

- [x] **Step 1: Add the exact development mode**

Accept `capture-batches` in the existing computed predicate. In `loadDevelopmentCapturePreview`, assign:

```ts
detail.value = mode === 'capture-batches' ? undefined : preview.detail
```

Keep the existing crop-editor assignment restricted to `mode === 'crop-editor'`.

- [x] **Step 2: Run CaptureView and fixture regressions**

Run: `npm test -- --run src/app/views/CaptureView.test.ts src/app/views/capture-development-preview.test.ts`

Expected: both suites pass and native behavior remains green.

### Task 3: Desktop And Narrow Browser Verification

**Files:**
- Modify only Task 1 or Task 2 files if visual verification reveals clipping, overlap, or page overflow.

**Interfaces:**
- Consumes: `/#/inbox?preview=capture-batches` and `/#/inbox?preview=capture-card`.
- Produces: computed-layout and interaction evidence for both major workspace states.

- [x] **Step 1: Start and record a temporary Vite server**

Confirm port 1420 is empty, start `npm run dev -- --host 127.0.0.1`, and record the exact listener PID.

- [x] **Step 2: Verify batch list at desktop and 390px**

At 1280×900 and 390×844, confirm no generic browser warning or page overflow; visible text computes to at least 12px; create/open/menu actions compute to at least 44px; open the exact batch overflow menu and confirm its destructive action is reachable and the layout changes from grid to one column at narrow width.

- [x] **Step 3: Verify organizing workspace at desktop and 390px**

Confirm toolbars, subject options, material actions, card inspector fields, and commit actions remain reachable without page overflow. Select the loose material, measure its role/draft actions at 44px, select a subject option, and confirm the narrow layout stacks the organizer and commit dock cleanly.

- [x] **Step 4: Restore browser and process state**

Reset the viewport, finalize only created tabs, stop the exact Vite execution and listener PID, and confirm port 1420 returns to zero listeners.

### Task 4: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-capture-workspace-readability.md`

**Interfaces:**
- Consumes: red/green tests, browser evidence, Git hygiene, and final code review.
- Produces: checked steps and exact verification evidence.

- [x] **Step 1: Run static and full regression gates**

Run: `npm run typecheck`

Run: `npm run lint`

Run: `npm test -- --run`

Expected: every command exits 0.

- [x] **Step 2: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust or launch-gate file was changed by this task.

- [x] **Step 3: Perform final code review**

Review preview isolation, selector coverage, typography hierarchy, action geometry, responsive stacking, source-contract robustness, accessibility semantics, regressions, and production-path invariants. Fix every Critical or Important finding.

- [x] **Step 4: Record evidence without committing**

Check all steps and append exact red/green totals, desktop/narrow measurements, interaction results, process cleanup, static/full results, Git hygiene, scope, and review verdict. Do not stage or commit.

## Verification Record — 2026-08-01

- Red phase: `CaptureWorkspaceReadability.test.ts` ran 3 tests and all 3 failed. The type scan reported 36 explicit 9–11px declarations; target checks failed on the missing baseline and existing 32–43px rules.
- Contract green: after the CSS changes, the new 3-test contract passed and the explicit sub-12px count was zero.
- Focused green: workspace readability, workspace behavior, draft card, thumbnail, `CaptureView`, and the development fixture passed, 6 files / 78 tests.
- Batch-list desktop browser (1280×900): no generic warning or page overflow; computed minimum visible type was 12px. The grid exposed three 310px tracks, the subject select and create action measured 45px, and the overflow/menu actions measured 44px. The exact `数学的更多操作` menu opened and exposed `删除批次`.
- Batch-list narrow browser (390×844): no page overflow; the grid became one 335px column. The select measured 45px; create, menu, and delete actions measured 44px; the 148px menu stayed inside the viewport.
- Organizing desktop browser (1280×900): no page overflow; the organizer measured 340px + 585px columns and minimum text was 12px. All nine subject buttons, subject confirmation, five selected-material actions, and commit action measured 44px; the inspector input measured 49px. Selecting the loose material and the `物理` subject option both succeeded.
- Organizing narrow browser (390×844): no page overflow; organizer and subject sections became one 335px / 295px column, commit actions stacked vertically, all sampled action heights remained 44px, and the inspector input remained 49px. Material actions, card inspector, and commit action were visually reachable while scrolling.
- Review fix: final review found that the 68px fixed mobile navigation could cover the sticky commit dock and that the enlarged batch menu reduced long-title clearance. The mobile dock now uses `bottom:82px`, and the batch heading uses 56px right padding. Focused review tests passed, 2 files / 31 tests.
- Review-fix browser evidence: at 390×844 the dock ended at y=762 and the navigation began at y=776, leaving exactly 14px. The batch heading's 56px padding placed its content edge at x=274 while the menu began at x=295, leaving 21px clearance. Page overflow remained false.
- Process hygiene: the primary listener was PID 22332 and the short review listener was PID 27844. Both exact processes were stopped; port 1420 ended with zero listeners. Temporary viewport overrides were reset and created tabs were finalized.
- Final gates: `npm run typecheck` and `npm run lint` exited 0 after the review fix. `npm test -- --run` passed, 106 files / 638 tests.
- Patch hygiene and index: scoped `git diff --check` exited 0 with only existing LF/CRLF notices. `git diff --cached --name-only` was empty.
- Scope: this task changed only `CaptureWorkspace` CSS, its readability contract, the exact development batch-list mode in `CaptureView`, and this plan. It did not modify Rust, native commands, domain transactions, or launch-gate items; nothing was staged or committed.
- Final local review: the one Important responsive-overlap issue and the title-clearance issue were fixed and reverified. No Critical or remaining Important issue was found; production initialization and all emitted workspace behavior remain unchanged.
