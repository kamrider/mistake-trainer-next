# Capture Quality Analysis Controller Plan

**Goal:** Move capture quality state, request policy, stale-result protection, dismissal, batch reset, and crop-seed projection out of `CaptureView.vue` into the capture feature.

**Architecture:** Add `useCaptureQualityAnalysis` under `src/modules/capture/composables`. It consumes the active batch id and one quality-check operation, owns readonly reports/errors/checking/dismissed state, watches the batch identity to reset contextual state, and projects a quality report into the existing crop-editor seed contract. `CaptureView` only binds controller state/actions to `CaptureWorkspace` and adapts the generated command.

### Task 1: Specify the controller boundary

- [x] Test successful checks, per-item caching, duplicate admission, dismissal, crop-seed projection, command failures, transport failures, stale completion, and batch reset.
- [x] Keep the operation typed as `Promise<AppResult<CaptureQualityReport>>` so generated-command normalization remains at the app composition edge.

### Task 2: Implement and compose

- [x] Implement readonly state and explicit `check`, `dismiss`, `cropSeed`, and `reset` behavior.
- [x] Export the controller from `src/modules/capture/index.ts`.
- [x] Replace the four quality refs and three local functions in `CaptureView.vue` with controller composition.

### Task 3: Ratchet and verify

- [x] Add an architecture assertion preventing quality workflow return to `CaptureView.vue` and document ownership.
- [x] Run controller/CaptureView tests, typecheck, architecture, lint, all tests, and diff checks.
- [x] Recount `CaptureView.vue` and proceed to batch-detail persistence/lifecycle.
