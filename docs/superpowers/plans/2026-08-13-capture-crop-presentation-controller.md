# Capture Crop Presentation Controller Plan

**Goal:** Move ordinary/development/recognition crop modal coordination, launcher lookup, successor focus, and modal-return-focus state out of `CaptureView.vue` into the capture feature.

**Architecture:** First move the generic modal focus controller from `src/app` to `src/shared/ui`. Then add `useCaptureCropPresentation` under `src/modules/capture/composables`; it consumes crop operations and active batch/editor state, owns development editor state plus both focus controllers, and exposes readonly visible state and explicit modal actions. `CaptureView` only adapts item-editing and OCR operations.

### Task 1: Correct generic UI dependency direction

- [x] Move `modal-return-focus.ts` and its tests to `src/shared/ui`.
- [x] Update CaptureView import and verify focused tests, typecheck, and architecture contract.

### Task 2: Controller tests and implementation

- [x] Create tests for launcher capture/fallback, failed open restoration, close restoration, derived-item successor focus, development preview close/apply, recognition edit close/save, context change, and competing restore generations.
- [x] Implement explicit options interface and readonly `visibleCropEditor`/development state.
- [x] Own all `data-crop-*` and recognition edit control lookup inside the feature controller.
- [x] Re-export the controller from `src/modules/capture/index.ts`.
- [x] Run focused tests green.

### Task 3: Compose and verify

- [x] Delete three DOM lookup functions, two focus controllers, development crop ref, and six crop modal wrapper functions from CaptureView.
- [x] Preserve development preview, ordinary crop, quality seed, recognition proposal, and template bindings through controller adapters.
- [x] Add architecture assertions preventing crop presentation workflow return to CaptureView and document ownership.
- [ ] Run CaptureView/controller tests, typecheck, architecture, lint, all tests, and diff checks.
- [ ] Recount CaptureView and choose quality analysis or batch-detail persistence next.
