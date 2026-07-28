# Model-free Smart Image Splitting Implementation Plan

> **For Codex:** Use the `executing-plans` skill to implement this plan task by task. Keep the current dirty worktree intact and do not stage or commit unrelated changes.

**Goal:** Ship an available, model-free “智能切图” flow that splits multi-question page images with deterministic local visual analysis and places every accepted crop into the material library without creating or modifying cards. Present “全自动识题” as a separate, disabled future mode for OCR, subject/answer understanding, automatic card assembly, and export.

**Architecture:** Port the validated whitespace/layout algorithm from the Python OpenCV bakeoff into a dependency-free Rust production engine built on the existing `image` crate. The engine uses thresholded foreground density, column gaps, and row whitespace only; it never reads text or downloads a model. Recognition apply remains encrypted and atomic, but new operations create only derived `capture_items`, preserving the source item’s staged question/answer role. Existing operation-ledger draft fields remain readable so older recognition operations can still be reverted safely.

**Tech Stack:** Rust, Tauri, SQLite, AES-GCM asset storage, `image`, Vue 3, TypeScript, Vitest.

---

## Task 1: Lock the material-library-only persistence contract

**Files:**
- Modify: `src-tauri/tests/capture_recognition.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/app/views/CaptureView.vue`

**Steps:**

1. Keep the failing integration expectations that an accepted question crop and answer crop produce zero drafts, two unassigned material items, and no `capture_draft_items`.
2. Run the focused Rust and Vue tests and confirm they fail against the old automatic-card behavior.
3. Remove new-draft creation and draft-item linking from `apply_capture_recognition`; assign every derived item the accepted region role and keep it unassigned.
4. Keep `RecognitionOperationLedger.created_drafts` and its revert logic for backward compatibility, but write an empty list for new operations.
5. Update review confirmation and success notices to say that accepted crops enter the material library and existing cards are unchanged.
6. Re-run the focused tests and verify they pass.

## Task 2: Add the deterministic local visual splitter

**Files:**
- Create: `src-tauri/src/infrastructure/recognition_visual_split.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/infrastructure/capture_recognition_worker.rs`
- Modify: `src-tauri/src/commands/capture_recognition.rs`
- Modify: `src-tauri/tests/ocr_capability_command.rs`

**Steps:**

1. Add failing Rust tests using synthetic pages for vertically separated blocks, two columns, blank pages, stable reading order, and role inheritance.
2. Implement luminance conversion, Otsu thresholding, lightweight binary noise cleanup, central column-gap detection, row-density runs, gap merging, padding, bounds checks, and conservative confidence/reason codes.
3. Return no OCR anchors, no pairing tokens, and no group slots; inherit `staged_role` for every crop.
4. Make `CaptureRecognitionManager::for_product()` use this visual engine without any OCR runtime/model gate.
5. Change recognition job metadata to an honest model-free visual engine name/version.
6. Run worker, command, and capability tests.

## Task 3: Separate the available and future smart modes in capability contracts

**Files:**
- Modify: `src-tauri/src/modules/ocr_capability.rs`
- Modify: `src-tauri/tests/ocr_capability_command.rs`
- Modify: `src/shared/api/bindings.ts` only if generated bindings change
- Modify: `tests/repository-contract.test.ts`

**Steps:**

1. Add failing assertions that the current recognition feature is ready and requires no downloadable OCR model.
2. Make the capture recognition feature report the built-in local visual splitter as ready independently of PP-OCR component installation.
3. Preserve optional OCR component inventory for future work, but do not use it to gate current image splitting.
4. Update repository contract assertions so they do not require disabled runtime constants for the current feature.
5. Re-run contract, capability, and binding-drift tests.

## Task 4: Deliver the two-mode desktop experience

**Files:**
- Modify: `src/modules/ocr/components/CaptureRecognitionEntry.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionEntry.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/ocr/components/OcrCapabilityPanel.vue`
- Modify: `src/modules/ocr/components/OcrCapabilityPanel.test.ts`
- Modify: `src/app/views/SettingsView.test.ts`

**Steps:**

1. Add failing UI assertions for:
   - “智能切图 · 已开放”
   - no OCR/model-download requirement
   - output goes to the material library
   - “全自动识题 · 未开放”
   - future OCR/subject/answer/card/export scope
2. Rewrite the active entry’s preflight and progress copy around model-free visual splitting and mandatory crop review.
3. Add a clearly disabled future-mode card with no executable control.
4. Replace the settings download-oriented presentation with the same two-mode capability explanation, so the current release no longer offers small/medium OCR models as if required for splitting.
5. Ensure keyboard/accessibility labels identify the current and future modes correctly.
6. Run the focused component and view tests.

## Task 5: Documentation, regression, build, and desktop acceptance

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/windows-smart-question-organizing-acceptance.md`
- Modify: `labs/question-region-bakeoff/README.md` if production-engine status needs clarification

**Steps:**

1. Document the two-mode boundary, model-free algorithm, encrypted derived assets, source retention, material-only apply, and future full-auto scope.
2. Run:
   - focused Vue recognition/workspace/view/settings tests
   - `pnpm lint`
   - `pnpm typecheck`
   - `pnpm test`
   - `pnpm build`
   - focused Rust recognition/capability tests
   - Rust all-target tests
   - binding drift check
   - Tauri Windows build
3. Launch the built desktop app and verify the two modes, current-mode preflight, preview/apply copy, and material-library-only result on representative source images.
4. Report the exact installer path, test totals, any deliberately deferred OCR behavior, and any dataset edge cases that still require manual crop review.

## Acceptance Criteria

- “智能切图” works offline without OCR, an AI model, a model download, or a network request.
- Every accepted crop is an unassigned material-library item; no draft/card is created or modified.
- Question/answer role is inherited from the source item and remains editable in the material library.
- The source image remains available, derived crops remain encrypted, apply is atomic, and revert remains safe for both new and older ledger formats.
- Low-confidence or blank pages remain reviewable and are never silently discarded.
- “全自动识题” is visible but clearly disabled and never executes.
- Existing capture, drag/drop, crop, commit, backup, and sync behavior remains green.
