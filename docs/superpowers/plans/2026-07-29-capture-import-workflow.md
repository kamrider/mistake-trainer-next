# Capture Import Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move picker, drag/drop, and clipboard import orchestration out of `CaptureView.vue` and prevent completed imports from reopening a batch the user left.

**Architecture:** Keep `useCaptureFileImport` as the byte-reading/concurrency engine. A new workflow composable wraps it with picker commands, completion notices, post-import detail/list refresh selection, paste filtering, busy handoff, progress cleanup, and active-batch guards.

**Tech Stack:** Vue 3 refs, TypeScript, generated Tauri bindings, Vitest.

## Global Constraints

- Preserve all pre-existing worktree changes.
- Do not edit recognition, OCR, Rust, generated bindings, migrations, installer/release, or excluded pre-launch behavior.
- Keep `CaptureWorkspace` event names, the 150-image limit, two-file concurrency, source sequencing, Chinese notice/fallback copy, and picker command signature unchanged.
- Never reopen or display a detail-specific error on a closed/different batch; successful durable imports that finish after leaving must refresh the batch list.
- Do not stage or commit.

---

### Task 1: Build the import workflow controller

**Files:**

- Create: `src/modules/capture/composables/useCaptureImportWorkflow.ts`
- Create: `src/modules/capture/composables/useCaptureImportWorkflow.test.ts`

**Interfaces:**

- Consumes: active detail, busy/error callbacks, list/detail loaders, picker operation, and the existing `CaptureFileImportController`.
- Produces: `progress`, `importSelect()`, `importFiles(files)`, `importFromPaste(event)`, `clearProgress()`, and `dispose()`.

- [x] Add failing tests for picker command input, busy/error transitions, active detail refresh, late picker success, command errors, and picker fallback copy.
- [x] Cover multi-file notices, same-batch detail refresh, departed-batch list refresh, partial-failure fallback, progress cleanup, and empty/no-op outcomes.
- [x] Cover clipboard image filtering, text-control protection, completed-batch protection, and `preventDefault` only when image files are accepted.
- [x] Implement the controller and run its focused test file.

### Task 2: Integrate without changing workspace contracts

**Files:**

- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

- [x] Add a failing view regression that opens the system picker, leaves the batch, resolves the picker, and proves the old batch is not reopened.
- [x] Instantiate the workflow around `useCaptureFileImport`, replace `importSelect`, `importFiles`, and `handlePaste`, and route watcher/unmount cleanup through the workflow.
- [x] Keep `CaptureWorkspace` import events and progress prop unchanged; run workflow and view focused tests.

### Task 3: Verify and review

- [x] Run `npm run lint`, `npm run typecheck`, `npm run test:coverage`, and `npm run build`.
- [x] Confirm direct coverage of late picker, late file outcome, picker error, file fallback, notice formatting, and clipboard guards.
- [x] Confirm `CaptureView.vue` is smaller and no excluded subsystem was touched.
