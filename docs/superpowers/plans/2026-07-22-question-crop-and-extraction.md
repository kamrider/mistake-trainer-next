# Question Capture, Crop, and Structured Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Do not ship automatic splitting before the manual workflow has real-device acceptance data and the suggestion benchmark passes.

**Current status (2026-07-22):** Desktop non-destructive 1–10 region crop is implemented in `3020025`; mobile optional crop, unified review, and recovery are implemented in `135455b`; the desktop workbench now has reachable real-dimension zoom, Space/middle-button pan, eight-direction handles, ordered previews, and narrow-window filmstrip access. The isolated OpenCV bake-off harness is available under `labs/question-region-bakeoff`, but no consented 60-image run has been completed. Real-device crop acceptance, RapidOCR comparison, production automatic suggestions, OCR, and structured reconstruction remain pending. Completed Phase 1 work must not be rebuilt, and the lab must not be wired into the signed application before its gates pass.

**Goal:** Let one phone photo safely become one or more clean question/answer assets, then optionally derive searchable and editable content without ever trusting OCR or generative reconstruction as the only copy.

**Product decision:** This is not a choice between phone crop, desktop crop, and OCR. Use a layered workflow: the phone captures continuously without a mandatory edit after every shot; the desktop is the canonical precise editor for crop, perspective correction, and multi-question splitting; a local engine may propose regions but never applies them silently; OCR and reconstruction are optional derived views and never replace the image master.

**Architecture:** Treat crop, OCR, and reconstruction as three separate layers. Crop creates immutable encrypted derived assets from a retained source; OCR creates text/layout metadata; structured reconstruction creates a hybrid view containing text/LaTeX plus preserved image regions. No layer overwrites its source.

**Tech Stack:** Existing Vue 3 canvas crop editor and Rust `image` crop/re-encode path; OpenCV in a disposable preprocessing spike for perspective/skew only; RapidOCR PP-OCRv6 tiny/small through ONNX Runtime as the first local text-box candidate; an isolated PaddleOCR-VL/PP-Structure sidecar only for comparison; existing SQLCipher/AES-GCM asset storage and outbox.

## Research Findings and Boundaries

- Adobe Scan and CamScanner establish the market pattern: automatically find a document boundary, correct perspective, enhance the page, then let the user crop or repair it. Google ML Kit additionally offers on-device rotation, shadow/stain cleanup, and optional manual crop. These products solve *page scanning*; they do not prove that arbitrary adjacent exam questions can be split without confirmation.
- Apple VisionKit and Google ML Kit document scanners are native mobile APIs. The current QR page is a browser page and cannot call them directly. Its existing `<input capture>` delegates to the system camera and on iOS returns one captured photo at a time.
- A custom live camera scanner in the web page would require `getUserMedia`, which browsers expose only in a secure HTTPS context. The current ephemeral LAN session is plain HTTP, so adding live edge detection would also require trusted local TLS provisioning or a native companion app. Neither belongs in the next Windows v1 increment.
- Paddle layout models identify generic elements such as text, formula, image, table, header, and footer; PaddleOCR-VL parses text, tables, formulas, and charts. Neither exposes a reliable built-in `question` region for Chinese school worksheets. Question splitting still needs recognized question-number anchors, reading order, layout rules, and a review step—or a custom detector trained on corrected samples.
- RapidOCR supports PP-OCRv6 tiny/small/medium models with ONNX Runtime and is the smallest credible local candidate for text boxes and question-number anchors on an 8 GB Windows baseline. It should be benchmarked before adding any runtime or model to the installer.
- PaddleOCR-VL is a compact 0.9B-class document model and supports x64 CPU, but its packaged runtime is still substantial; official offline Docker images are roughly 11 GB. MinerU is broader again. Both belong behind an optional downloadable “advanced parsing” experiment, not in the default app.
- Mathpix recognizes printed/handwritten STEM text, formulae, tables, and some chemistry/diagram content, while Aliyun exposes education-specific question/formula OCR. They are useful comparison adapters, but introduce upload privacy, regional availability, cost, and model drift. They are not the source of truth.
- Formula and chemistry reconstruction are specialized tasks. pix2tex generates LaTeX from formula images and MolScribe generates molecular graphs from chemical structure images, but neither makes arbitrary geometry, plots, maps, apparatus diagrams, or handwritten work safe to delete.
- Removing black pencil/pen marks from black printed strokes is intrinsically ambiguous when the strokes overlap. Any cleanup model must produce a separate derivative, an editable mask, a before/after comparison, and explicit acceptance. It is lower priority than crop and split.

### Product option decision

| Option | User speed | Reliability | Offline/size | Decision |
| --- | --- | --- | --- | --- |
| Force crop after every phone photo | Poor for batch capture | User-controlled, but easy to rush | Light | Reject as default |
| Optional phone quick crop in upload queue | Good | Suitable for obvious single rectangles | Light | Add later, never blocking |
| Desktop manual multi-region crop | Good after batch capture | Highest; large preview and undo/revert | Already local | Canonical editor; implemented |
| Desktop one-click local suggestions | Very good when correct | Safe only with visible regions and confirmation | Moderate model download | Next experiment |
| Replace images with OCR/HTML | Fast after processing | Unsafe for formulas and diagrams | Text is small, models are not | Reject as master format |
| Hybrid text + preserved image blocks | Good for search/export | Auditable and reversible | Additive storage | Long-term target |

Primary references:

- [Apple VisionKit document camera](https://developer.apple.com/documentation/visionkit/vndocumentcameraviewcontroller)
- [Apple document scanning UX](https://support.apple.com/en-ie/108963)
- [Google Drive scanning UX](https://support.google.com/drive/answer/3145835?co=GENIE.Platform%3DAndroid&hl=en)
- [ML Kit document scanner](https://developers.google.com/ml-kit/vision/doc-scanner/android)
- [Browser camera secure-context requirement](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)
- [HTML file/capture behavior](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/file)
- [PaddleX layout detection categories](https://paddlepaddle.github.io/PaddleX/3.0-rc/en/module_usage/tutorials/ocr_modules/layout_detection.html)
- [PaddleOCR-VL pipeline and hardware support](https://www.paddleocr.ai/latest/en/version3.x/pipeline_usage/PaddleOCR-VL.html)
- [PP-StructureV3](https://www.paddleocr.ai/main/en/version3.x/algorithm/PP-StructureV3/PP-StructureV3.html)
- [RapidOCR](https://github.com/RapidAI/RapidOCR)
- [MinerU](https://github.com/opendatalab/MinerU)
- [Mathpix OCR API](https://docs.mathpix.com/)
- [Aliyun education OCR](https://help.aliyun.com/en/ocr/product-overview/education-scenario-recognition-1)
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

**Harness status:** The consent-first manifest validator, deterministic OpenCV page/whitespace
baseline, safety metrics, atomic JSON/HTML report, and visual overlays are implemented as an
isolated developer lab. Synthetic tests prove mechanics only. The 60-image evaluation, RapidOCR
PP-OCRv6 comparison, memory/installer measurements, and 300-image decision gate are still open.

**Goal:** Validate the smallest local pipeline before adding a runtime dependency to the product.

- Start with a consented, anonymized 60-image bake-off and expand to at least 300 images before any production model decision. Cover one question, adjacent questions, two columns, skew, shadows, formulas, geometry, chemistry, handwritten marks, question-only, and answer pages.
- Record ground-truth crop rectangles and question-number anchors in a standalone fixture; do not use production user photos without explicit consent.
- Spike five independent capabilities:
  1. manual rectangle crop and multi-region split;
  2. OpenCV document quadrilateral, perspective, and skew correction;
  3. OCR text boxes/question-number anchors with RapidOCR PP-OCRv6 ONNX;
  4. generic layout/formula/image blocks with PaddleOCR-VL or PP-Structure in an isolated sidecar;
  5. one opt-in regional education API and Mathpix on a small non-sensitive comparison subset.
- Compare Rust `ort` integration with an isolated local sidecar for binary size, cold start, CPU time, peak RAM, Windows packaging, and license inventory.
- Exit the 60-image bake-off only if at least one local pipeline is fast and promising enough to justify the 300-image evaluation. Exit the 300-image gate only if question-start recall is at least 95%, content-cut rate is below 0.5%, false split rate is below 3%, every uncertain case is visibly flagged, and the 8 GB reference machine remains responsive. Otherwise keep automation experimental and rely on the implemented manual crop.

---

## Phase 1: Non-destructive manual crop and multi-question split

**Status:** Desktop and mobile implementations are complete, including the polished large-image desktop workbench. Remaining Phase 1 work is real-device acceptance on representative Windows scaling, iPhone Safari, and Android Chrome; do not replace the existing crop ledger or editor.

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
- Each queued thumbnail gets an optional `裁剪` action and the session gets a `拍完统一检查` entry. The optional editor supports one rectangle, rotate, reset, skip, and confirm; closing it never cancels the upload queue.
- Upload the original plus a normalized crop recipe. Let Rust create the encrypted derivative so mobile canvas re-encoding is not the only retained copy.
- Do not attempt semantic auto-splitting in the browser page in this phase. A native Apple/Google document scanner integration remains a future companion-app decision.

### Acceptance

- One photo can create 1–10 crops without decoding repeatedly or duplicating the source blob.
- Restart, backup, restore, discard, outbox, orphan cleanup, and 30-day source retention all preserve references correctly.
- Crops remain pixel-sharp for formulas; no crop can escape the decoded source or exceed product limits.
- Mouse, touch, keyboard, 200% Windows scaling, high contrast, and reduced motion all pass.

---

## Phase 2: Local automatic question-region suggestions

### Pipeline

1. Correct EXIF orientation. Offer a four-corner page/perspective suggestion only when a stable quadrilateral is found; otherwise preserve the rectangular source.
2. Run local OCR detection/recognition to obtain text polygons, reading order, and confidence.
3. Identify likely question anchors in the left margin using configurable Chinese/English patterns, for example Arabic numbers, Chinese section numbers, `题`, and option markers.
4. Set tentative vertical boundaries midway between consecutive high-confidence anchors.
5. Attach formula/image/table blocks whose centers lie inside the interval and expand boundaries so they do not cut through a detected block.
6. Return suggestions and uncertainty; do not create assets until the user confirms.

For answer sheets or handwritten solutions, do not assume that every question starts with a clean printed number. Fall back to coarse whitespace/column grouping or manual regions, and label the result `需要确认`.

### Interfaces and UX

- `capture_crop_suggest(itemId)` returns regions, anchors, confidence, and engine metadata.
- `capture_crop_suggest_batch(batchId)` runs with cancellation, bounded concurrency, progress events, and resumable results.
- `capture_perspective_suggest(itemId)` returns four normalized corners and confidence separately from semantic question regions.
- High-confidence results may be selected by default, never applied automatically. Low-confidence or multi-column pages open directly in review mode.
- Every user adjustment is stored as an anonymizable local correction record. Exporting corrections for future training requires explicit consent.

### Model decision gate

- Preferred first experiment: OpenCV perspective/skew + RapidOCR PP-OCRv6 small ONNX + deterministic question-number/column/range rules.
- If recall is inadequate, fine-tune an Apache-compatible detector on question-start anchors or regions. Do not introduce an AGPL model into the signed desktop app without a separate legal decision.
- Compare PaddleOCR-VL/PP-Structure, one domestic education OCR API, and Mathpix only through a provider-neutral lab contract. Do not use OpenAI or a general chat VLM by default for this deterministic crop task.
- Package no model in the main installer until cold start, peak RAM, installer delta, license inventory, and representative-device latency are measured. A successful model is an optional signed download with checksum and version pinning.

---

## Phase 3: Searchable hybrid OCR, not destructive text replacement

Add `problem_content_snapshots` and `problem_content_blocks` containing source asset/derivation, text, normalized bounds, block kind (`text | formula | figure | table | chemistry`), confidence, engine/version, language, and user corrections.

- Run ordinary Chinese/English text through the local OCR selected in Phase 0.
- Keep formulas as images initially; optionally add Paddle formula recognition, pix2tex, or a paid STEM adapter after a benchmark proves acceptable exact-match and render equivalence.
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
- Compare all OCR text against the source at block level. A visually plausible reconstruction with a wrong symbol, exponent, charge, unit, or graph label is a failure, not a partial success.
- Full existing lint/typecheck/Vue/Rust/binding/build gates remain mandatory.

## Recommended Delivery Order

1. Real-device accept the implemented desktop multi-region crop and fix any crop/revert defects.
2. Add `拍完统一检查` plus optional phone quick crop without slowing default capture.
3. Run the 60-image local bake-off; publish measurements before selecting a model.
4. If promising, expand to 300 images and ship Phase 2 only as reviewable suggestions via an optional signed model download.
5. Add searchable hybrid OCR after crop corrections provide a real evaluation set.
6. Revisit cleanup, VLMs, formula, chemistry, and a native phone companion only after measured demand.

This order solves the actual pain immediately, protects data, and creates the correction data needed for better automation without locking the product to a large model or a paid API.
