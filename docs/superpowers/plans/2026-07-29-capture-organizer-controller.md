# Capture Organizer Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the six revision-aware capture organizing mutations out of `CaptureView.vue` while preventing late results from reopening a batch the user left.

**Architecture:** A capture composable owns command input construction, busy/save-state transitions, product error copy, conflict refresh, and active-batch identity checks. `CaptureView` supplies generated command adapters and remains responsible for page loading and non-organizer workflows.

**Tech Stack:** Vue 3 refs, TypeScript, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition/OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep all component events, command signatures, Chinese error messages, and two-state revision semantics unchanged.
- Never apply a successful or failed late result to a different/closed batch.
- Do not stage or commit.

---

### Task 1: Build the organizer controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureOrganizerActions.ts`
- Create: `src/modules/capture/composables/useCaptureOrganizerActions.test.ts`

- [x] Write failing tests for exact inputs to layout, subject, move, staged role, merge, and draft deletion.
- [x] Add tests for busy/save-state transitions, per-action fallback copy, revision-conflict reload, blocked calls, and leave-during-request behavior.
- [x] Run the new test and verify it fails because the composable is missing.
- [x] Implement a shared mutation runner plus the six typed public actions.
- [x] Run the controller test and verify all cases pass.

### Task 2: Integrate without changing the view contract

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] Add a view regression that starts an organizer mutation, leaves the batch, resolves the command, and proves the old batch is not reopened.
- [x] Instantiate the controller with normalized generated commands and replace the six inline implementations with controller methods.
- [x] Keep every existing `CaptureWorkspace` event binding unchanged.
- [x] Run controller and `CaptureView` focused tests.

### Task 3: Verify and review

- [x] Run lint, typecheck, full coverage, and production build.
- [x] Confirm the scoped diff preserves inputs/copy/conflict behavior, reduces `CaptureView.vue`, and does not touch excluded subsystems.
