# Question Crop and Structured Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Do not start Phase 2 before Phase 1 has real-device acceptance data.

**Goal:** Let one phone photo safely become one or more clean question/answer assets, then optionally derive searchable and editable content without ever trusting OCR or generative reconstruction as the only copy.

**Product decision:** Desktop is the primary precise editor because the large preview and existing batch organizer make multi-question splitting faster and more trustworthy. Mobile gets an optional quick crop after capture, never a mandatory stop after every photo. Automatic detection proposes regions on desktop and can be applied in one click, but a user can always adjust or revert it.

**Architecture:** Treat crop, OCR, and reconstruction as three separate layers. Crop creates immutable encrypted derived assets from a retained source; OCR creates text/layout metadata; structured reconstruction creates a hybrid view containing text/LaTeX plus preserved image regions. No layer overwrites its source.

**Tech Stack:** Vue 3 and Canvas/Cropper.js for manual crop UI; Rust `image` for bounded crop/re-encode; a disposable spike comparing RapidOCR ONNX through Rust `ort` against a PaddleOCR sidecar; existing SQLCipher/AES-GCM asset storage and outbox.

## Research Findings and Boundaries

- Apple Notes/Files and Google Drive use automatic document boundaries, allow manual corner correction, and support multi-page scanning. That is the interaction benchmark, but their target is a whole page rather than a question inside a page.
- Apple VisionKit and Google ML Kit document scanner are native mobile APIs. The current QR experience is a browser page, so it cannot directly call either API. Adding them would require a native phone companion, which is outside the present Windows-first product.
- Paddle layout detection identifies generic document regions such as text, formula, image, table, header, and footer; it does not have a built-in `question` region. Question splitting therefore needs OCR coordinates plus question-number rules or a custom detector trained on corrected crops.
- RapidOCR is a practical first local OCR candidate: ONNX-based, Chinese/English capable, cross-platform, offline, and Apache-2.0. It is small enough to benchmark against the 8 GB Windows baseline before accepting a heavier framework.
- PaddleOCR-VL-1.5 is about 1.93 GB, while MinerU documents a 16 GB RAM minimum and large disk requirements for local parsing. These may be optional downloadable labs, not default v1 dependencies.
- Formula and chemistry reconstruction are specialized tasks. pix2tex generates LaTeX from formula images and MolScribe generates molecular graphs from chemical structure images, but neither makes arbitrary diagrams safe to delete.
- Removing black pencil/pen marks from black printed strokes is not a reliable general-purpose cleanup step. Any cleanup model must produce a preview and mask, preserve the source, and require confirmation.

Primary references:

- [Apple VisionKit document camera](https://developer.apple.com/documentation/visionkit/vndocumentcameraviewcontroller)
- [Apple document scanning UX](https://support.apple.com/en-ie/108963)
- [Google Drive scanning UX](https://support.google.com/drive/answer/3145835?co=GENIE.Platform%3DAndroid&hl=en)
- [ML Kit document scanner](https://developers.google.com/ml-kit/vision/doc-scanner/android)
- [PaddleX layout detection categories](https://paddlepaddle.github.io/PaddleX/3.0-rc/en/module_usage/tutorials/ocr_modules/layout_detection.html)
- [RapidOCR](https://github.com/RapidAI/RapidOCR)
- [PaddleOCR-VL-1.5 model](https://huggingface.co/PaddlePaddle/PaddleOCR-VL-1.5/tree/main)
- [MinerU](https://github.com/opendatalab/MinerU)
- [pix2tex](https://github.com/lukas-blecher/LaTeX-OCR/blob/main/README.md)
- [MolScribe](https://github.com/thomas0809/MolScribe)

## Global Constraints

- Never mutate or replace an existing encrypted asset blob.
- Never delete a source photo automatically because a crop, OCR, cleanup, or reconstruction succeeded.
- All coordinates use normalized `[0, 1]` values and are validated in Rust; the UI never supplies a file path.
- A crop must remain within the decoded image, have a non-zero area, and produce an output within existing dimension/pixel/byte limits.
- Every automatic result records engine name, model/version, confidence, and creation time.
- Cloud or VLM processing is opt-in and disabled by default; local capture and manual crop remain fully offline.
- The crop workflow must preserve the current rapid batch path: capture can continue without editing each image.

---

## Phase 0: Disposable model and workflow spike

**Goal:** Validate the smallest local pipeline before adding a runtime dependency to the product.

- Build an anonymized evaluation set of at least 300 phone images covering one question, adjacent questions, two columns, skew, shadows, formulas, geometry, chemistry, handwritten marks, question-only, and answer pages.
- Record ground-truth crop rectangles and question-number anchors in a standalone fixture; do not use production user photos without explicit consent.
- Spike three independent capabilities:
  1. manual rectangle crop and multi-region split;
  2. OCR text boxes/question-number anchors with RapidOCR ONNX;
  3. generic layout blocks with PaddleOCR/PaddleX.
- Compare Rust `ort` integration with an isolated local sidecar for binary size, cold start, CPU time, peak RAM, Windows packaging, and license inventory.
- Exit only if question-start recall is at least 95%, proposed regions never silently discard content, and the 8 GB reference machine remains responsive. Otherwise ship manual crop first and keep automation experimental.

---

## Phase 1: Non-destructive manual crop and multi-question split

### Data model

Add migration `0012_asset_derivations.sql`:

- `asset_derivations`: `id`, `account_id`, `source_asset_id`, `derived_asset_id`, `kind`, `recipe_json`, `engine`, `engine_version`, `confidence`, timestamps.
- `capture_source_retention`: source asset, batch, `retain_until_utc_ms`, and reason. Retain at least 30 days after a derived item is committed.
- Extend capture items with `superseded_by_derivation_id` rather than deleting the source item.

`recipe_json` v1 supports a normalized rectangle, clockwise rotation, output media type, maximum edge, and JPEG quality. A later version may add four-corner perspective geometry without changing existing recipes.

### Rust commands

- `capture_crop_preview(itemId, recipe)` returns a bounded in-memory preview.
- `capture_crop_apply(itemId, recipes[])` atomically creates one or more encrypted derived assets/items and marks the source as superseded.
- `capture_crop_revert(derivationId)` restores the retained source when no committed problem depends on the derived item.
- `capture_item_split(itemId, regions[])` is the multi-region form used when one photo contains several questions.

The apply operation decrypts once, decodes once, crops all regions in memory, validates every result, writes staged encrypted blobs, then commits every asset/item/derivation in one transaction. Any failure removes staged blobs and leaves the source unchanged.

### Desktop interaction

- Add a clear `裁剪 / 分题` action to every large organizer preview and keyboard shortcut `C`.
- Open a full-height editor with zoom/pan, a high-resolution image, undo/redo, rotate, reset, and a movable rectangle.
- `添加区域` lets one source photo produce several numbered regions. Regions appear as a vertical filmstrip and can be reordered before applying.
- Applying animates the source into the new cards, but respects reduced motion. The organizer keeps current question/answer role and draft assignment where unambiguous; extra regions return to the unassigned lane.
- Show `已从原图裁剪 · 30 天内可恢复` instead of hiding the derivation.

### Mobile interaction

- After selection/capture, continue uploading by default so batch speed is unchanged.
- Each queued thumbnail gets an optional `裁剪` action. It opens a lightweight rectangular crop editor and uploads the crop recipe with the source, not only destructive canvas pixels.
- Do not attempt semantic auto-splitting in the browser page in this phase. A native Apple/Google document scanner integration remains a future companion-app decision.

### Acceptance

- One photo can create 1–10 crops without decoding repeatedly or duplicating the source blob.
- Restart, backup, restore, discard, outbox, orphan cleanup, and 30-day source retention all preserve references correctly.
- Crops remain pixel-sharp for formulas; no crop can escape the decoded source or exceed product limits.
- Mouse, touch, keyboard, 200% Windows scaling, high contrast, and reduced motion all pass.

---

## Phase 2: Local automatic question-region suggestions

### Pipeline

1. Correct EXIF orientation and optionally estimate paper perspective/skew.
2. Run local OCR detection/recognition to obtain text polygons, reading order, and confidence.
3. Identify likely question anchors in the left margin using configurable Chinese/English patterns, for example Arabic numbers, Chinese section numbers, `题`, and option markers.
4. Set tentative vertical boundaries midway between consecutive high-confidence anchors.
5. Attach formula/image/table blocks whose centers lie inside the interval and expand boundaries so they do not cut through a detected block.
6. Return suggestions and uncertainty; do not create assets until the user confirms.

### Interfaces and UX

- `capture_crop_suggest(itemId)` returns regions, anchors, confidence, and engine metadata.
- `capture_crop_suggest_batch(batchId)` runs with cancellation, bounded concurrency, progress events, and resumable results.
- High-confidence results may be selected by default, never applied automatically. Low-confidence or multi-column pages open directly in review mode.
- Every user adjustment is stored as an anonymizable local correction record. Exporting corrections for future training requires explicit consent.

### Model decision gate

- Preferred first experiment: RapidOCR ONNX + deterministic question-number/range rules.
- If recall is inadequate, fine-tune an Apache-compatible detector on question-start anchors or regions. Do not introduce an AGPL model into the signed desktop app without a separate legal decision.
- PaddleOCR-VL/Qwen-VL/API adapters may be compared in a lab, but are not the default because they increase download size, RAM, latency, and nondeterminism.

---

## Phase 3: Searchable hybrid OCR, not destructive text replacement

Add `problem_content_snapshots` and `problem_content_blocks` containing source asset/derivation, text, normalized bounds, block kind (`text | formula | figure | table | chemistry`), confidence, engine/version, language, and user corrections.

- Run ordinary Chinese/English text through the local OCR selected in Phase 0.
- Keep formulas as images initially; optionally add a formula adapter after a benchmark proves acceptable exact-match and render equivalence.
- Keep geometry, graphs, maps, and experimental diagrams as cropped image blocks.
- Treat chemical structure recognition as an optional specialized adapter; keep the image beside any generated SMILES/molfile.
- Render a hybrid question: selectable text and LaTeX where verified, original image snippets for everything else.
- Use OCR for search, copy, accessibility, and later AI features. Do not make training/export depend on OCR success.

Storage policy: after the 30-day recovery window, a user may purge the raw source and keep verified cropped assets. OCR text itself is tiny, but it is additive; meaningful storage reduction comes from cropping/re-encoding and eventually purging retained raw sources, not from pretending OCR can replace every image.

---

## Phase 4: Experimental cleanup and full reconstruction

- Add `清理笔迹（实验）` only as a preview with before/after slider and editable mask.
- Never auto-accept cleanup where black handwriting overlaps printed text, formulas, or diagrams.
- Offer cloud/VLM adapters only behind explicit download/privacy/cost disclosure and per-batch consent.
- A reconstructed web version must show confidence and retain a one-click `查看原图` comparison.
- Geometry vectorization and chemistry graph reconstruction remain separate experiments with their own benchmarks; they are not acceptance requirements for crop or OCR.

## Quality Gates

- Rust property tests for normalized geometry, rotation, overflow, zero-area regions, malicious recipe JSON, and transaction rollback.
- Fault injection after each staged blob write and before/after database commit.
- Golden-image crop tests for EXIF rotation, JPEG/PNG/WebP, transparency, formulas, and 80-million-pixel bounds.
- Visual interaction tests for handles, zoom, multi-region reorder, focus, reduced motion, and touch targets.
- Performance target on the 4-core/8 GB Windows machine: manual preview under 300 ms after decryption; one automatic suggestion under 2 seconds on a typical 12 MP photo; cancellation observed within 250 ms.
- Model evaluation reports question-start recall, region IoU, content-cut rate, false split rate, CPU time, RAM, download size, and failures by subject/layout.
- Full existing lint/typecheck/Vue/Rust/binding/build gates remain mandatory.

## Recommended Delivery Order

1. Ship Phase 1 desktop manual multi-region crop first.
2. Add the optional phone quick crop without slowing default capture.
3. Run the local OCR spike and ship Phase 2 only as suggestions.
4. Add searchable hybrid OCR after crop corrections provide a real evaluation set.
5. Revisit cleanup, VLMs, formula, and chemistry adapters only after measured user demand.

This order solves the actual pain immediately, protects data, and creates the correction data needed for better automation without locking the product to a large model or a paid API.
