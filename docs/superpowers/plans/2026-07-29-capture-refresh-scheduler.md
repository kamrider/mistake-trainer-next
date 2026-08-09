# Capture Refresh Scheduler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Coalesce capture batch-change events without losing affected batches and suppress stale detail-load errors after navigation.

**Architecture:** A small capture composable accumulates changed batch IDs during the debounce window, then refreshes the active detail, the batch list when other batches changed, and LAN status once. `CaptureView` retains its loaders but delegates timer ownership and guards detail-load failures with the existing request identity token.

**Tech Stack:** Vue 3 callbacks, TypeScript, Vitest fake timers.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition, OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep the `capture_batch_changed` event name and 120 ms debounce unchanged.
- Never surface a detail-load result or error after the user closes or switches away from that request.
- Do not stage or commit.

---

### Task 1: Build the refresh scheduler

**Files:**

- Create: `src/modules/capture/composables/useCaptureRefreshScheduler.ts`
- Create: `src/modules/capture/composables/useCaptureRefreshScheduler.test.ts`

- [x] Add failing fake-timer tests for active-only changes, inactive-only changes, mixed bursts, repeated IDs, active-batch changes before flush, and disposal.
- [x] Implement `schedule(batchId)`, `flush()`, and `dispose()` with a pending-ID set and one 120 ms timer.
- [x] Ensure callback failures settle without unhandled rejections and run the focused test.

### Task 2: Integrate and guard stale errors

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] Add a failing regression where detail loading rejects after the user closes the batch and prove no obsolete error appears.
- [x] Replace `refreshTimer`/`scheduleRefresh` with the scheduler while keeping the event subscription unchanged.
- [x] Guard the `loadDetail` catch branch with `requestedDetailBatchId === batchId` and dispose the scheduler on unmount.
- [x] Run scheduler and `CaptureView` focused tests.

### Task 3: Verify and review

- [x] Run lint, typecheck, full coverage, and production build.
- [x] Confirm mixed event bursts refresh both active detail and list exactly once, stale errors are suppressed, and excluded subsystems are untouched.
