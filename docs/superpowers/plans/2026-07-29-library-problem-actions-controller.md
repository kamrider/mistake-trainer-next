# Library Problem Actions Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move single-problem update and status-change orchestration out of `LibraryView.vue` and prevent late commands from reopening or closing the wrong detail drawer.

**Architecture:** A library composable owns command inputs, saving/error state, durable sync scheduling, list refresh, active-problem identity checks, conditional detail reload, and conditional drawer close. `LibraryView` retains list/detail loading and supplies generated command adapters.

**Tech Stack:** Vue 3 callbacks, TypeScript, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition, OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep `ProblemDetailDrawer` events, command signatures, Chinese copy, list refresh, and sync scheduling semantics unchanged.
- Successful durable mutations must still schedule sync and refresh the list after navigation; stale results/errors must never reopen or close another detail.
- Do not stage or commit.

---

### Task 1: Build the problem action controller

**Files:**

- Create: `src/modules/library/composables/useLibraryProblemActions.ts`
- Create: `src/modules/library/composables/useLibraryProblemActions.test.ts`

- [x] Add failing tests for exact update/status inputs, saving transitions, command errors, and stable fallback copy.
- [x] Cover update success, status success, sync scheduling, list refresh, conditional detail reload/close, blocked calls, late success after close/navigation, and late failures.
- [x] Implement `updateProblem(input)` and `changeProblemStatus(problemId, targetStatus)` and run focused tests.

### Task 2: Integrate without changing drawer contracts

**Files:**

- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`

- [x] Add a failing view regression that saves a problem, closes the drawer before completion, resolves the command, and proves the drawer remains closed.
- [x] Instantiate the controller with normalized commands, existing refresh/detail callbacks, and sync scheduling.
- [x] Replace the two inline action functions while keeping drawer event bindings unchanged; run controller and view focused tests.

### Task 3: Verify and review

- [x] Run lint, typecheck, full coverage, and production build.
- [x] Confirm direct coverage of late update success/error and late status success/error, preserved durable side effects, and no excluded subsystem changes.
