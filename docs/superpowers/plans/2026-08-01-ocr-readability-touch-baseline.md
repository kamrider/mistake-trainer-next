# OCR Readability And Touch Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make all visible OCR capability, entry, and review copy at least 12px and every OCR action target at least 44px without enlarging decorative icons or changing recognition behavior.

**Architecture:** Keep the three existing Vue component boundaries and all recognition state transitions unchanged. Add a module-scoped source contract that distinguishes visible typography from decorative geometry, then update only CSS declarations and verify the settings-hosted capability panel in the real browser preview plus all existing OCR interaction tests.

**Tech Stack:** Vue 3 SFC scoped CSS, TypeScript, Vitest, Testing Library Vue, Vite local preview

## Global Constraints

- Do not modify templates, scripts, recognition state machines, native commands, Rust files, or OCR runtime/model behavior.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.
- Every explicit visible `font-size: Npx` in the three target SFCs must be at least 12px; the existing `.sr-only` 1px geometry is not typography and remains unchanged.
- Every button or link presented as an OCR action must have a minimum 44px target. Decorative `.panel-icon`, `.mode-icon`, and `.recognition-icon` geometry and the 8px progress bar remain unchanged.
- Preserve existing responsive breakpoints, review keyboard shortcuts, focus behavior, confirmation dialog, disabled states, and reduced-motion behavior.
- Do not stage or commit the dirty worktree.

---

### Task 1: OCR Readability And Action Contract

**Files:**
- Create: `src/modules/ocr/components/OcrReadability.test.ts`
- Modify: `src/modules/ocr/components/OcrCapabilityPanel.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionEntry.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`

**Interfaces:**
- Consumes: the three SFC sources through `node:fs` using repository-root paths.
- Produces: regression coverage for visible pixel typography and the exact action selectors that must remain at least 44px.

- [x] **Step 1: Add the failing source contract**

Create a Vitest test that reads the three SFCs, reports each explicit `font-size` below 12px, compacts CSS whitespace, and asserts these selectors retain 44px targets: `.enhancement-control button`; the entry action group and `.secondary-link`; the review `.icon-button`, `.preview-placeholder button`, `.review-position button`, and base `button` rule.

- [x] **Step 2: Run the contract and verify the intended red result**

Run: `npm test -- --run src/modules/ocr/components/OcrReadability.test.ts`

Expected: FAIL with current 9px, 10px, and 11px declarations plus the current 34px, 38px, 39px, and 42px action targets.

- [x] **Step 3: Raise visible copy and real action targets**

Change every explicit 9px, 10px, and 11px `font-size` in the three target SFCs to 12px. Set capability enhancement actions to 44px; entry primary/secondary/text actions and the secondary template link to 44px; review close, placeholder recovery, previous/next, and base action buttons to 44px. Keep decorative icon sizes and the progress bar unchanged.

- [x] **Step 4: Run the OCR contract and component tests**

Run: `npm test -- --run src/modules/ocr/components/OcrReadability.test.ts src/modules/ocr/components/OcrCapabilityPanel.test.ts src/modules/ocr/components/CaptureRecognitionEntry.test.ts src/modules/ocr/components/CaptureRecognitionReview.test.ts`

Expected: all focused files and tests pass.

### Task 2: Real Settings Preview Verification

**Files:**
- Modify only Task 1 style files if the real preview reveals clipping or horizontal overflow.

**Interfaces:**
- Consumes: the existing Vite application route `/#/settings` and the settings-hosted `OcrCapabilityPanel`.
- Produces: desktop and narrow-screen evidence for the capability cards, badges, lists, and local-only disclosure.

- [x] **Step 1: Use a workspace Vite preview without disturbing user processes**

Confirm the project-defined local preview is reachable on port 1420. Do not stop or replace a user-owned server process.

- [x] **Step 2: Inspect the desktop settings route**

At 1280px viewport width, open `/#/settings`, locate the `智能功能模式` region, and verify its two-card grid, 12px labels/list copy, badges, and disclosure fit without page or panel horizontal overflow.

- [x] **Step 3: Inspect the narrow settings route**

At 390px viewport width, verify the mode grid collapses to one column, long mode descriptions wrap, no horizontal overflow appears, and any rendered OCR action target remains at least 44px.

- [x] **Step 4: Restore browser state**

Reset the temporary viewport and finalize only the browser tabs created for this verification. Leave the existing Vite process running.

### Task 3: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-ocr-readability-touch-baseline.md`

**Interfaces:**
- Consumes: completed contract, focused tests, browser evidence, and repository checks.
- Produces: checked task boxes and an exact verification record.

- [x] **Step 1: Run static checks**

Run: `npm run typecheck`

Run: `npm run lint`

Expected: both commands exit 0.

- [x] **Step 2: Run the complete frontend suite**

Run: `npm test -- --run`

Expected: every frontend test file and test passes.

- [x] **Step 3: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust file was changed by this task.

- [x] **Step 4: Record verification without committing**

Check every completed task box and append the exact red/green contract result, focused and full test totals, desktop/narrow browser findings, static checks, hygiene, index, and scope results. Do not run `git add` or `git commit`.

## Verification Record

- Contract red: `npm test -- --run src/modules/ocr/components/OcrReadability.test.ts` failed both intended tests, reporting 20 explicit 9px, 10px, or 11px declarations and the capability panel's current 38px action target.
- Focused green: the OCR readability contract plus `OcrCapabilityPanel`, `CaptureRecognitionEntry`, and `CaptureRecognitionReview` tests passed 4 files and 22 tests.
- Desktop browser: at 1280px by 900px, the real `/#/settings` route rendered the intelligence panel as two 421.5px columns. The panel was 909px wide with 907px scroll width, page and panel horizontal overflow were false, and sampled eyebrow, badge, list, and disclosure copy all computed to 12px.
- Narrow browser: at 390px by 844px, the mode grid collapsed to one 305px column. Both cards were 305px wide with 303px scroll width, the panel was 343px wide with 341px scroll width, page and panel horizontal overflow were false, and long descriptions and lists wrapped without clipping.
- Default preview state: no model capability payload was present, so the settings page rendered no install/remove action. The capability action's 44px height is covered by the source contract; entry and review action sizes are likewise covered together with their existing interaction tests.
- Preview cleanup: port 1420 had no listener before verification, so this task started its own Vite process. Browser tabs and viewport were finalized/reset, the execution unit and its recorded child PID 17064 were stopped, and the final 1420 listener count was 0.
- Static checks: `npm run typecheck` and `npm run lint` both exited 0.
- Complete frontend suite: `npm test -- --run` passed 102 files and 628 tests.
- Hygiene: `git diff --check` exited 0 with only pre-existing Windows line-ending warnings, and `git diff --cached --name-only` was empty.
- Scope: this task changed only CSS in the three OCR SFCs, added the OCR readability contract, and added this plan record. No template, script, recognition behavior, Rust file, or launch-gate item was changed by this task. Existing dirty worktree changes were preserved; nothing was staged or committed.
