# Library Detail Editor Refresh Resilience Implementation Plan

**Goal:** Prevent same-problem detail refreshes and late save results from silently closing the editor or overwriting newer local input.

**Architecture:** Move the drawer's editable fields into a small identity-aware composable. It keeps an authoritative baseline per problem, preserves dirty fields across same-problem refreshes, acknowledges a submitted draft only when refreshed detail matches it, and resets on a real problem identity change. Keep the current detail visible while reloading the same problem so a save refresh does not create a false identity transition.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue.

---

### Task 1: Lock the regressions down with tests

**Files:**
- Create: `src/modules/library/composables/useProblemDetailEditor.test.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/app/views/LibraryView.test.ts`

- [x] Prove a same-problem detail refresh currently exits edit mode and loses dirty fields.
- [x] Cover a matching authoritative save result closing a submitted editor.
- [x] Cover newer typing after submit surviving the older matching save result.
- [x] Cover switching to another problem resetting the editor.
- [x] Cover same-problem reload retaining the current detail instead of flashing empty.

### Task 2: Implement identity-aware editor state

**Files:**
- Create: `src/modules/library/composables/useProblemDetailEditor.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`

- [x] Track the current problem identity and authoritative editable-field baseline.
- [x] Preserve dirty input and edit mode on same-problem background refresh.
- [x] Hydrate clean editors from authoritative same-problem changes without closing edit mode.
- [x] Record submitted values and close only when a matching refreshed result arrives and no newer edit exists.
- [x] Reset fields, pending submission, and edit mode on a real identity change.
- [x] Keep unsaved-change and time-limit validation behavior intact.

### Task 3: Make same-problem reloads non-destructive

**Files:**
- Modify: `src/app/views/LibraryView.vue`

- [x] Clear detail immediately only when opening a different problem.
- [x] Keep the current detail mounted while reloading the same problem after save.
- [x] Preserve the existing request-sequence protection against late detail responses.

### Task 4: Verify the batch

- [x] Run the new composable and drawer tests.
- [x] Run the affected library view and problem-action tests.
- [x] Run the full frontend test suite.
- [x] Run TypeScript checking and lint.
- [x] Run `git diff --check`, trailing-whitespace scan, plan-checkbox scan, and confirm the index remains untouched.

### Verification record

- [x] Focused tests: 4 files, 37 tests passed.
- [x] Affected library tests: editor, drawer, problem actions, and view all passed.
- [x] Full frontend tests: 97 files, 594 tests passed.
- [x] Typecheck: `vue-tsc --noEmit` passed.
- [x] Lint: `eslint . --max-warnings 0` passed.
- [x] Diff/worktree checks: passed; Git index remains empty.
