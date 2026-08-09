# Capture Batch Lifecycle Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move capture batch creation, discard, collection completion, and library commit orchestration out of `CaptureView.vue` while preventing stale asynchronous results from mutating a different batch.

**Architecture:** A focused capture composable owns command inputs, busy state, product error copy, LAN shutdown ordering, refresh callbacks, commit messaging, and sync scheduling. `CaptureView` retains refs and supplies generated-command adapters plus page-level loaders.

**Tech Stack:** Vue 3, TypeScript, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition, OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep `CaptureWorkspace` event names, command signatures, Chinese copy, and sync scheduling semantics unchanged.
- Never apply a late batch-specific message, detail result, or detail refresh to a closed, different, or newer revision of the batch; successful durable mutations must still refresh the batch list and schedule sync when required.
- Do not stage or commit.

---

### Task 1: Build the lifecycle controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureBatchLifecycle.ts`
- Create: `src/modules/capture/composables/useCaptureBatchLifecycle.test.ts`

**Interfaces:**

- Consumes: active batch detail, LAN batch identity, busy/error/message callbacks, page refresh callbacks, sync scheduling callback, and four typed command adapters.
- Produces: `createBatch(subject)`, `discardBatch(batchId)`, `finishCollecting(subject)`, and `commitReady()`.

- [x] Add failing tests that prove exact create/update/commit inputs and discard ID forwarding.
- [x] Cover busy transitions, command failures, stable Chinese fallbacks, LAN stop-before-mutation ordering, refresh behavior, commit copy, and sync scheduling only when committed count is positive.
- [x] Cover late success, late failure, active batch replacement, and same-batch revision change so stale work cannot update messages or reload detail, while a successful commit still refreshes the list and schedules sync.
- [x] Implement the smallest typed controller that satisfies those tests.
- [x] Run `npm run test -- --run src/modules/capture/composables/useCaptureBatchLifecycle.test.ts` and require a pass.

### Task 2: Integrate without changing the view contract

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] Add a view regression that starts commit, closes the batch, resolves the command, and proves the batch remains closed and no commit message is applied.
- [x] Instantiate `useCaptureBatchLifecycle` with normalized generated commands and existing loaders/LAN/sync callbacks.
- [x] Replace the four inline lifecycle implementations with the controller methods while leaving all `CaptureWorkspace` bindings unchanged.
- [x] Run lifecycle and `CaptureView` focused tests.

### Task 3: Verify and review

- [x] Run `npm run lint`, `npm run typecheck`, `npm run test:coverage`, and `npm run build`.
- [x] Confirm the lifecycle controller has direct coverage of stale success and failure branches.
- [x] Confirm the scoped diff reduces `CaptureView.vue`, preserves copy and command shapes, and does not touch excluded subsystems.
