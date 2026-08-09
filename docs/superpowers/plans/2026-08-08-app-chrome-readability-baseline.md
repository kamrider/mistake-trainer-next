# App Chrome Readability Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The active workspace policy forbids sub-agents unless the user explicitly requests them, so this session uses local review.

**Goal:** Establish and enforce a 12 px text floor and 44 px interactive-target floor across the always-visible application shell, global notices, and library access boundary.

**Architecture:** Add one source-level readability contract covering `App.vue`, `AppShell.vue`, and `LibraryAccessScreen.vue`. Production changes remain CSS-only: raise explicit undersized text and interactive dimensions while preserving component behavior, responsive structure, ARIA, and visual hierarchy.

**Tech Stack:** Vue single-file component CSS, Vitest source contracts, Node filesystem reads.

## Global Constraints

- Every explicit pixel font size in the three covered files must be at least 12 px.
- Desktop and mobile navigation buttons must be at least 44 px high.
- Global restore and Windows compatibility notice close buttons must be at least 44×44 px.
- Library access primary and secondary actions must remain at least 44 px high.
- Decorative marks and non-interactive indicators are not required to be 44 px.
- Preserve all copy, events, routes, states, animation preferences, and responsive behavior.
- Do not touch backup/restore dialogs, storage migration, updater recovery, APIs, dependencies, or recognition code.
- Do not stage or commit; preserve unrelated dirty-worktree changes.

---

### Task 1: Add the app chrome readability contract

**Files:**
- Create: `src/app/AppChromeReadability.test.ts`

**Interfaces:**
- Consumes: raw UTF-8 source of `src/app/App.vue`, `src/app/AppShell.vue`, and `src/app/LibraryAccessScreen.vue`.
- Produces: a regression gate for explicit pixel fonts and critical interactive dimensions.

- [x] **Step 1: Add the font-floor assertion**

Read all three files, collect every `font-size: Npx` where `N < 12`, include its file path and token in the failure output, and assert the violation list is empty.

- [x] **Step 2: Add critical target assertions**

Assert `.nav-item` contains `min-height: 44px`, `.restore-notice button` contains both `width: 44px` and `height: 44px`, `.route-error button` contains `min-height: 44px`, and `.access-primary, .access-secondary` contains `min-height: 46px`.

- [x] **Step 3: Run the contract in red state**

Run: `npm test -- src/app/AppChromeReadability.test.ts`

Expected: FAIL for 10/11 px text, the 43 px desktop navigation target, and the 30×30 px global notice close target.

---

### Task 2: Raise app-wide shell readability

**Files:**
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/App.vue`
- Modify: `src/app/LibraryAccessScreen.vue`

**Interfaces:**
- Preserves: component props, events, templates, route recovery, library access actions, and mobile navigation.

- [x] **Step 1: Correct application shell typography and navigation**

Raise `.brand small` from 10 px to 12 px. Raise `.nav-item` from 43 px to 44 px and `.nav-indicator` to the same 44 px so the visual active background stays aligned.

- [x] **Step 2: Correct global notice typography and dismissal target**

Raise `.restore-notice small` from 11 px to 12 px. Raise `.restore-notice button` from 30×30 px to 44×44 px without changing its icon size or accessible label.

- [x] **Step 3: Correct library access secondary text**

Raise `.access-eyebrow` and `.access-footnote` from 11 px to 12 px; preserve line height and letter spacing.

- [x] **Step 4: Run the new contract and existing shell/access tests**

Run: `npm test -- src/app/AppChromeReadability.test.ts src/app/AppShell.test.ts src/app/LibraryAccessScreen.test.ts src/app/App.test.ts`

Expected: all tests pass.

---

### Task 3: Quality gates and review

**Files:**
- Verify: `src/app/AppChromeReadability.test.ts`
- Verify: `src/app/AppShell.vue`
- Verify: `src/app/App.vue`
- Verify: `src/app/LibraryAccessScreen.vue`
- Modify: `docs/superpowers/plans/2026-08-08-app-chrome-readability-baseline.md`

**Interfaces:**
- Preserves: complete application tests and production build.

- [x] **Step 1: Run static gates**

Run: `npm run lint`

Run: `npm run typecheck`

Expected: no warnings or errors.

- [x] **Step 2: Run production build**

Run: `npm run build`

Expected: Vite production build passes.

- [x] **Step 3: Run complete regression suite**

Run: `npm test`

Expected: every Vitest file and test passes.

- [x] **Step 4: Review and record evidence**

Confirm the contract covers exactly the three intended files, no covered font is below 12 px, critical controls meet the target floor, indicators remain aligned, excluded files are untouched, the staged index is empty, and `dist/` is ignored. Append exact command results in a `Verification Record` section.

## Verification Record

- Red phase: the new contract ran 2 tests and both failed. It reported four exact font violations (`App.vue` 11 px, `AppShell.vue` 10 px, two `LibraryAccessScreen.vue` 11 px values) plus the 43 px navigation target; the notice target remained 30×30 px behind the first failing target assertion.
- Related green phase: 4 files, 13 tests passed.
- ESLint: passed with zero warnings.
- Vue/TypeScript typecheck: passed.
- Production build: passed; Vite transformed 2054 modules.
- Complete Vitest: 138 files, 770 tests passed.
- Covered source audit: no explicit pixel font below 12 px remains in `App.vue`, `AppShell.vue`, or `LibraryAccessScreen.vue`.
- Target audit: desktop navigation is 44 px, its indicator is 44 px, active offset advances by 49 px (44 px item plus 5 px gap), global notice close buttons are 44×44 px, route recovery remains 44 px, and library access actions remain 46 px.
- Scope audit: only the intended app chrome/access files and the new contract/plan were changed in this batch. `BackupRestoreDialog.vue` and `StorageMigrationDialog.vue` were not modified; the existing `recognition_visual_split.rs` modification was not touched.
- Diff check passed. The staged index remains empty and `dist/` remains ignored.
- Local review found no Critical or Important issues.
