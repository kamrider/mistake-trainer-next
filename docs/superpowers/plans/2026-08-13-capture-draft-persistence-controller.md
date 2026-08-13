# Capture Draft Persistence Controller Plan

**Goal:** Turn the existing queue primitive into a complete capture draft persistence boundary and remove command/error/watch policy from `CaptureView.vue`.

**Architecture:** `useCaptureDraftPersistence` composes `useCaptureDraftSaveQueue`. It owns public persistence state, exact update inputs, revision-conflict classification, safe detail replacement, retry/error behavior, blocked-state flushing, batch retention, and disposal. The view consumes readonly `unsaved`, `retryAvailable`, and `persistenceBusy` projections plus explicit update/retry/clear commands.

### Task 1: Controller contract

- [x] Test exact update input, success projection, revision conflict, command/transport failures, inactive batch rejection, retry, blocked flush, batch retention, and disposal.
- [x] Preserve the existing queue algorithm and tests unchanged.

### Task 2: Composition

- [x] Export the controller from the capture public API.
- [x] Replace local queue state, persistence function, watchers, retry wrapper, and update wrapper in `CaptureView.vue`.
- [x] Preserve unsaved-leave and transition-busy behavior through readonly projections.

### Task 3: Ratchet and verify

- [x] Add architecture ownership assertions and documentation.
- [x] Run focused and full verification.
