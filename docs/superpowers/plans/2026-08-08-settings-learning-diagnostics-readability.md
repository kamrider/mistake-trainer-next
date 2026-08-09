# Settings Learning and Diagnostics Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make subject selection, review-rhythm preferences, and diagnostics settings consistently legible and touch-safe at every viewport width.

**Architecture:** Treat `SettingsSubjectPanel.vue`, `SettingsReviewPanel.vue`, and `SettingsDiagnosticsPanel.vue` as one learning-and-support settings UI boundary and enforce it with a source-level Vitest contract. Keep subject invariants, preference intents, diagnostic privacy, native-runtime gating, persistence, and emitted events unchanged; modify only scoped CSS font sizes and control minimum dimensions.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Every explicit visible pixel font in the three target components must be at least 12 px.
- Subject chips, custom-subject removal, subject text entry, sound preference, save actions, review save, and diagnostic export must provide at least a 44 px target at every viewport width.
- Do not change subject invariants, custom-subject behavior, capture sound behavior, review policy behavior, diagnostic export orchestration, native-runtime gating, receipt privacy, or emitted events.
- Do not change storage migration, updater recovery, account deletion, licensing, privacy policy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the learning-and-diagnostics commercial UI contract

**Files:**
- Create: `src/app/components/SettingsLearningDiagnosticsReadability.test.ts`

- [x] **Step 1: Write the failing source contract**

Add three assertions: a 12 px explicit-font floor across the three components, subject-control 44 px targets at base width, and review/diagnostic base-button 44 px targets.

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/app/components/SettingsLearningDiagnosticsReadability.test.ts`

Expected: all three tests fail because seven declarations use 9–11 px text and several controls rely on mobile-only 44 px rules.

### Task 2: Raise subject settings to the commercial baseline

**Files:**
- Modify: `src/app/components/SettingsSubjectPanel.vue:189-387`

- [x] **Step 1: Make subject controls touch-safe at every width**

Set the base button, built-in subject chip, custom-subject removal button, custom-subject text input, and sound toggle to at least 44 px. Preserve the visually hidden checkbox dimensions.

- [x] **Step 2: Raise subject metadata to 12 px**

Change custom-subject chip copy and sound-toggle guidance to 12 px.

### Task 3: Raise review and diagnostics settings to the commercial baseline

**Files:**
- Modify: `src/app/components/SettingsReviewPanel.vue:121-257`
- Modify: `src/app/components/SettingsDiagnosticsPanel.vue:105-128`

- [x] **Step 1: Make review and diagnostic actions touch-safe at every width**

Set each component's base button to at least 44 px. Preserve the existing 104 px review option cards.

- [x] **Step 2: Raise review and diagnostic metadata to 12 px**

Raise review option hints/action feedback and diagnostic contract/receipt/error copy to 12 px.

- [x] **Step 3: Run the contract and existing behavior suites**

Run: `npm test -- src/app/components/SettingsLearningDiagnosticsReadability.test.ts src/app/components/SettingsSubjectPanel.test.ts src/app/components/SettingsReviewPanel.test.ts src/app/components/SettingsDiagnosticsPanel.test.ts`

Expected: all four files pass; subject invariants and intents, review policy and persistence intents, diagnostic native gating, receipt privacy, and export intentions remain intact.

### Task 4: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-settings-learning-diagnostics-readability.md`

- [x] **Step 1: Run static validation**

Run `npm run lint` and `npm run typecheck`.

- [x] **Step 2: Run the production build and full frontend suite**

Run `npm run build` and `npm test`.

- [x] **Step 3: Review scope and record completion**

Inspect all target files, perform whitespace checks that include untracked files, confirm excluded behavior stayed untouched, replace every task checkbox with `[x]`, and append exact verification counts.

## Verification Record

- Red contract: all 3 tests failed as expected, identifying 7 declarations below 12 px plus base-width touch targets that depended on mobile-only rules.
- Focused regression: 4 test files passed, 14 tests passed.
- Static validation: `npm run lint` passed with zero warnings; `npm run typecheck` passed with zero TypeScript errors.
- Production build: `npm run build` passed; Vite transformed 2054 modules.
- Full frontend regression: 144 test files passed, 790 tests passed.
- Local code review: no Critical or Important findings. All three target components have a 12 px explicit font floor; subject, review, and diagnostic controls have 44 px minimum targets at base width; review option cards retain their 104 px minimum height; responsive layouts remain unchanged.
- Scope review: only scoped styles and the source-level readability contract changed in the target components. Subject invariants, emitted intentions, review policy behavior, diagnostic native gating and privacy-safe receipts, excluded launch-only functionality, and `src-tauri/src/infrastructure/recognition_visual_split.rs` were not changed by this batch.
- Hygiene: no-index whitespace checks reported no whitespace errors (only the repository's LF-to-CRLF conversion warning); no files were staged or committed.
