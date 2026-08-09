# Capture Item Editing Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move ordinary capture-image removal and crop editing orchestration out of `CaptureView.vue` while making confirmations and asynchronous results safe across batch changes.

**Architecture:** A capture composable owns the crop editor ref, destructive confirmation requests, revision-aware command inputs, busy/save state, preview invalidation, conflict refresh, and stale-result guards. `CaptureView` supplies the shared confirmation service, generated command adapters, batch loaders, and reactive state callbacks.

**Tech Stack:** Vue 3 refs, TypeScript, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition, OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep `CaptureWorkspace` and `CaptureCropEditor` event names, command signatures, confirmation copy, fallback copy, and preview invalidation semantics unchanged.
- Revalidate the batch and target after confirmation; never apply a late result, error, editor opening, or conflict refresh to a closed, different, or newer batch revision.
- Successful durable crop mutations may refresh the batch list even when their detail result is stale.
- Do not stage or commit.

---

### Task 1: Build the item editing controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureItemEditing.ts`
- Create: `src/modules/capture/composables/useCaptureItemEditing.test.ts`

**Interfaces:**

- Consumes: desktop availability, active detail, busy/save/detail/error callbacks, a structural confirmation callback, preview invalidation, list/detail loaders, and remove/preview/apply/revert command adapters.
- Produces: readonly `cropEditor`, `closeCropEditor()`, `removeItem(itemId)`, `openCropEditor(itemId)`, `applyCrop(recipes)`, and `revertCrop(derivationId)`.

- [x] Add failing tests for exact command inputs and the two existing destructive confirmation request objects.
- [x] Cover confirmation cancellation, switching batches during confirmation, invalid/missing crop targets, busy transitions, success state, cache invalidation, editor close, list refresh, command errors, and stable fallback copy.
- [x] Cover revision-conflict detail refresh plus late success, late error, and late preview completion guards.
- [x] Implement the typed controller and run `npm run test -- --run src/modules/capture/composables/useCaptureItemEditing.test.ts`.

### Task 2: Integrate without changing component contracts

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] Add a view regression that requests deletion, leaves the batch while confirmation is open, confirms, and proves no command targets the departed batch.
- [x] Instantiate the item editing controller with normalized generated commands and existing callbacks.
- [x] Replace the four inline functions and local crop editor ref; close the controller editor when leaving batch detail.
- [x] Keep all `CaptureWorkspace` and `CaptureCropEditor` bindings unchanged and run focused tests.

### Task 3: Verify and review

- [x] Run `npm run lint`, `npm run typecheck`, `npm run test:coverage`, and `npm run build`.
- [x] Confirm direct coverage of stale success, stale failure, stale preview, confirmation switching, and revision-conflict reload.
- [x] Confirm `CaptureView.vue` is smaller and the scoped diff does not touch excluded subsystems.
