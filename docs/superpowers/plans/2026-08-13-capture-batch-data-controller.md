# Capture Batch Data Controller Plan

**Goal:** Move batch list/detail loading, requested-detail identity, stale-response protection, and route projection out of `CaptureView.vue`.

**Architecture:** Add `useCaptureBatchData` in the capture feature. It owns readonly batch/detail/requested-id state and exposes explicit load, clear, detail replacement, and development-preview hydration commands. Generated-command normalization stays in the app adapter; recognition preloads and route replacement are injected capabilities, breaking the current implicit mutable-variable coupling.

### Task 1: Specify data ownership

- [x] Test list/detail success, command and transport failures, stale responses, recognition preload, route projection, clear invalidation, external detail replacement, and development hydration.
- [x] Keep operations expressed as `AppResult` and route/recognition effects behind callbacks.

### Task 2: Compose CaptureView

- [x] Export `useCaptureBatchData` from the capture public API.
- [x] Replace local batch/detail/requested-id refs and load functions.
- [x] Route organizer, item editing, OCR, draft persistence, development preview, and clear-detail changes through explicit controller commands.

### Task 3: Ratchet and verify

- [x] Add architecture ownership assertions and documentation.
- [x] Run focused and full verification, then decide whether draft persistence or Rust boundary is the next highest-risk remainder.
