# Capture Import Intake Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make drag/drop and clipboard imports accept every supported image reliably, reject unsupported files explicitly, and expose one authoritative intake contract across component and workflow boundaries.

**Architecture:** Move file-format classification from `CaptureWorkspace` and clipboard adapters into `useCaptureFileImport`, where capacity, concurrency, file reads, and per-file outcomes already live. Extend the typed outcome with unsupported names and attempted count so `useCaptureImportWorkflow` can produce truthful copy and avoid unnecessary detail refreshes when nothing was imported. Keep `CaptureWorkspace` responsible only for drop affordance/admission and forwarding the original file list.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, browser File/Clipboard/Drag APIs.

## Global Constraints

- Support PNG, JPEG/JPG, and WebP by recognized MIME type or filename extension; accept supported extensions when MIME is blank or generic.
- Never read or send unsupported file bytes to the native import command.
- Unsupported files must be named in user-facing copy and must not disappear silently.
- Apply the 150-item capacity limit only to supported files; report unsupported files separately from capacity-skipped and failed supported files.
- A drop during `busy`, completed-batch state, or browser-only mode must not emit an import intention or show an active drop target.
- Preserve two-worker import concurrency, source ordering, active-batch stale guards, picker behavior, progress semantics, partial-success behavior, and clipboard protection for text inputs.
- Do not change capture storage limits, native/Rust validation, recognition, synchronization, storage/device migration, updater recovery, account deletion, bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize the unified intake contract

**Files:**

- Modify: `src/modules/capture/composables/useCaptureFileImport.test.ts`
- Modify: `src/modules/capture/composables/useCaptureImportWorkflow.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

- [x] **Step 1: Add file-classification regression**

Pass a blank-MIME `photo.PNG`, generic-MIME `answer.jpeg`, recognized WebP, and unsupported PDF. Assert the three images reach `importBytes`, the PDF bytes are never read, and the outcome contains `unsupportedNames: ['notes.pdf']` plus `attemptedCount: 3`.

- [x] **Step 2: Add capacity separation regression**

With one remaining slot, pass two supported images and one PDF. Assert one supported image is attempted, one is capacity-skipped, and the PDF is unsupported rather than counted among skipped images.

- [x] **Step 3: Add workflow notice and no-refresh regression**

Return an unsupported-only outcome and assert copy names the files and supported formats, no detail/list refresh runs, and busy ownership is not fabricated. Extend the mixed partial-outcome test to preserve capacity and failed-file copy alongside unsupported copy.

- [x] **Step 4: Add clipboard forwarding regression**

Provide blank-MIME PNG and PDF clipboard file items. Assert both original files reach the authoritative importer while text inputs remain protected.

- [x] **Step 5: Add drop admission regression**

Drop blank-MIME PNG plus PDF and assert both are emitted unchanged. While busy, assert an image drop emits nothing, `aria-disabled` is true, and dragenter does not activate the target.

- [x] **Step 6: Run focused suites red**

Run file importer, import workflow, and workspace tests. Expect missing outcome fields, silent component filtering, clipboard filtering, and busy drop activation assertions to fail.

### Task 2: Implement authoritative classification and outcomes

**Files:**

- Modify: `src/modules/capture/composables/useCaptureFileImport.ts`
- Modify: `src/modules/capture/composables/useCaptureFileImport.test.ts`

- [x] **Step 1: Add supported-image classification**

Recognize `image/png`, `image/jpeg`, and `image/webp`; when MIME is blank or `application/octet-stream`, fall back to case-insensitive `.png`, `.jpg`, `.jpeg`, or `.webp` extension checks.

- [x] **Step 2: Partition before capacity and byte reads**

Create supported and unsupported arrays before calculating remaining capacity. Never call `arrayBuffer()` for unsupported entries.

- [x] **Step 3: Extend the outcome**

Return `unsupportedNames` and `attemptedCount` on capacity-zero, unsupported-only, partial, and successful outcomes while retaining batch ID, failed names, skipped count, progress, and busy semantics.

- [x] **Step 4: Run importer tests green and typecheck**

Run the importer suite and `npm run typecheck` after updating typed fixtures.

### Task 3: Integrate workflow, clipboard, and drop UX

**Files:**

- Modify: `src/modules/capture/composables/useCaptureImportWorkflow.ts`
- Modify: `src/modules/capture/composables/useCaptureImportWorkflow.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

- [x] **Step 1: Build truthful unsupported copy**

Append `文件名…不是支持的图片格式；仅支持 PNG、JPEG 和 WebP。` before capacity/failed notices, preview at most three names, and retain all existing partial-import wording.

- [x] **Step 2: Avoid false refresh work**

When `attemptedCount` is zero, publish the unsupported notice but do not set refreshing busy state or reload detail/list. Always clear progress in `finally`.

- [x] **Step 3: Forward clipboard file items unchanged**

Collect all `kind === 'file'` clipboard items, including blank MIME and unsupported files, and let the importer classify them. Keep input/textarea and empty-clipboard guards.

- [x] **Step 4: Make the drop affordance admission-aware**

Forward the original `FileList`, gate dragenter/dragover/drop by `!busy && desktopAvailable && batch is not completed`, set `aria-disabled`, and render disabled copy while another capture operation is active.

- [x] **Step 5: Run focused and adjacent tests**

Run importer, workflow, workspace, Capture view, and capture-file/import workflow integrations.

### Task 4: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review boundary ownership and truthfulness**

Confirm MIME/extension policy exists only in the importer, component and clipboard forward original files, unsupported bytes never cross the native boundary, every outcome field is populated consistently, and unsupported-only attempts do not pretend to import or refresh.

- [x] **Step 3: Check workspace hygiene and record evidence**

Run target whitespace checks, confirm the index is empty, confirm build output remains ignored, and verify the pre-existing recognition modification was untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Baseline: file importer, import workflow, Capture workspace, and Capture view passed, 4 files / 79 tests.
- Red: focused importer, workflow, and workspace suites failed in 7 intended assertions (outcome fields, pre-read classification, unsupported notice/no-refresh, clipboard forwarding, and drop admission).
- Focused green: importer, workflow, workspace, and Capture view passed, 4 files / 83 tests; `npm run typecheck` passed after tightening the emitted-event assertion.
- Final gates: `npm run lint`, `npm run typecheck`, production `npm run build` (2,047 modules), and single-worker full Vitest (132 files / 742 tests) passed.
- Review: format policy exists only in `useCaptureFileImport`; clipboard and drop adapters forward original files; unsupported bytes are not read; all outcome paths populate unsupported/attempted/capacity fields; unsupported-only outcomes publish feedback without busy ownership or refresh.
- Hygiene: targeted tracked and untracked whitespace checks reported no errors; index is empty; `dist` remains absent from status; the pre-existing `recognition_visual_split.rs` modification remains present and was not edited in this batch.
