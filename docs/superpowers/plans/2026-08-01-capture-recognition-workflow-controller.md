# Capture Recognition Workflow Controller Implementation Plan

> **Execution:** Follow the `executing-plans` checkpoints in this session and record verification evidence before closing the batch.

**Goal:** Move recognition task orchestration out of `CaptureView.vue` and make rapid review decisions, optimistic state, and late async results safe enough for a commercial desktop workflow.

**Architecture:** A dedicated Vue composable owns recognition capability/job/operation/notice/editor state and all recognition command transitions. `CaptureView` remains the route and page coordinator, supplies typed command adapters and the active/requested batch boundary, and forwards the controller contract to `CaptureWorkspace`. Review decisions use a per-job serial queue with optimistic projection so distinct rapid decisions are never dropped and a failed command rolls back only its own pending decision.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, generated Tauri bindings.

## Scope and constraints

- Preserve existing Chinese product copy, persistent undo notice, model-setup routing, crop proposal behavior, and command payloads.
- Keep pair-suggestion application in the capture page; it is a capture mutation, not a recognition-job command.
- Ignore status and mutation results after the requested/active batch changes or the controller is reset/disposed.
- Keep recognition busy state separate from ordinary capture mutations while continuing to block unsafe page transitions during a recognition operation.
- Do not modify Rust, generated bindings, OCR algorithms/models, installer/release work, or excluded launch-gate items.
- Preserve all unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.

## Task 1: Lock the controller behavior with failing tests

**Files:**

- Create: `src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts`

- [x] Add a harness with typed recognition operations and mutable active/requested batch state.
- [x] Assert exact start/apply/revert payloads and successful detail/job/operation/notice transitions.
- [x] Assert two rapid review decisions are persisted serially and neither is dropped.
- [x] Assert a failed review removes only that optimistic decision while retaining later queued choices.
- [x] Assert a result or error arriving after reset/batch change does not mutate state or surface stale feedback.
- [x] Run the new test and confirm it fails because the composable does not exist.

## Task 2: Implement the recognition controller

**Files:**

- Create: `src/modules/ocr/composables/useCaptureRecognitionWorkflow.ts`

- [x] Own capability, feature, job, last operation, persistent notice, busy state, and proposal editor state.
- [x] Implement guarded capability/status/last-operation loading.
- [x] Implement start, cancel, proposal preview/save, apply, and revert with active-batch and lifecycle guards.
- [x] Implement a per-job serial review queue that coalesces a later pending decision for the same suggestion while preserving decisions for different suggestions.
- [x] Keep an authoritative job plus optimistic queued projections so server responses cannot erase unsent local choices.
- [x] Expose reset/dispose and recognition-change refresh methods.
- [x] Run controller tests to green.

## Task 3: Integrate the page boundary

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Verify: `src/app/views/CaptureView.test.ts`
- Verify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

- [x] Replace inline recognition refs/functions with `useCaptureRecognitionWorkflow`.
- [x] Supply normalized command adapters, including transport-failure mapping for typed commands.
- [x] Reset recognition state when leaving a batch and dispose it on unmount.
- [x] Route recognition events through the controller and keep OCR setup navigation in the page.
- [x] Preserve the workspace props/emits and proposal editor template contract.
- [x] Run controller, page, entry, review, workspace, and crop-editor tests.

## Task 4: Commercial-quality gates and review

- [x] Run TypeScript typecheck and ESLint.
- [x] Run the full Vitest suite serially if shared-load timing is unstable.
- [x] Inspect the scoped diff for stale-result protection, queue correctness, exact copy/payload preservation, and unintended scope expansion.
- [x] Run whitespace checks and confirm no files are staged, no generated artifacts appeared, and `recognition_visual_split.rs` was untouched.
- [x] Record baseline, red, focused, full, review, and hygiene results below.

## Verification record

- Baseline: 2 files / 46 tests passed (`CaptureView` + `CaptureRecognitionReview`).
- Red: the new controller suite failed at import resolution because `useCaptureRecognitionWorkflow.ts` did not exist.
- Controller green: 1 file / 5 tests passed; TypeScript passed before page integration.
- Focused integration: controller, page, entry, review, workspace, and crop editor passed, 6 files / 91 tests.
- Explicit non-blocking detail regression plus controller: 2 files / 42 tests passed.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed; serial full Vitest passed, 127 files / 700 tests.
- Review: `CaptureView.vue` is 813 lines (down from the audited 1,060); recognition commands are confined to the adapter block; active/requested batch checks cover start, cancel, review, preview/edit, apply, revert, status, reset, and disposal; the main batch detail no longer waits for optional recognition queries.
- Hygiene: target whitespace checks passed; nothing is staged or committed; no generated artifact entered the target status; the pre-existing `recognition_visual_split.rs` modification remains present and was not edited in this batch; excluded launch-gate work was not changed.
