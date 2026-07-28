# Automatic Question Region Suggestions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Stop after Task 3 unless the real-image gate passes.

**Goal:** Add a safe desktop “自动框题” workflow that proposes one or more question/answer regions locally, lets the user correct them in the existing crop workbench, and never replaces the source image or silently trusts OCR.

**Architecture:** Keep mobile capture fast: the phone may offer the existing optional quick crop, but never forces editing after every shot. Run automatic region suggestions on the Windows app after upload, using a separately benchmarked OpenCV + RapidOCR/ONNX pipeline; suggestions are only normalized rectangles and confidence metadata that feed the existing non-destructive crop editor. OCR text, formula reconstruction, handwriting erasure, and cloud education APIs remain separate derived/experimental layers.

**Tech Stack:** Existing Vue 3 crop workbench, Tauri/Rust command boundary, SQLCipher/AES-GCM assets, isolated Python 3.12 bake-off, OpenCV 5.0.0.93, RapidOCR 3.9.2 hosting explicit PP-OCRv6 small/medium models, ONNX Runtime 1.27.0.

## Global Constraints

- Do not rebuild the implemented desktop multi-region crop or mobile quick-crop flows.
- Do not block continuous phone capture with a mandatory editor.
- Never overwrite or automatically delete an encrypted source asset.
- Never apply an automatic region without visible user confirmation.
- Every suggestion records engine name, engine version, confidence, and uncertainty reason.
- Cloud processing is opt-in, per batch, and disabled by default; API credentials never ship in the desktop binary.
- OCR/reconstruction is a derived view, not the canonical problem record.
- Production integration is forbidden until at least 300 representative, consented images pass: question-start recall `>= 95%`, content-cut rate `< 0.5%`, false-split rate `< 3%`, and one-image latency `< 2 s` on the 4-core/8 GB Windows reference machine.
- DocLayout-YOLO must not be embedded because its published repository is AGPL-3.0; MinerU and full PaddleOCR-VL are comparison tools only because their runtime/storage footprint is too large for the default Windows v1 installer.

## Product Decision

| Stage | Default behavior | Reason |
| --- | --- | --- |
| Phone capture | Keep shooting; quick crop is optional | A required edit after each shot destroys batch speed, and a small screen is poor for multi-question splitting. |
| Desktop organization | “自动框题” followed by visual confirmation | The large crop editor already supports 1–10 ordered regions, zoom, pan, handles, undoable source retention, and manual fallback. |
| OCR | Search/copy helper after the crop is verified | Ordinary OCR can guide question-number anchors but cannot safely reconstruct every formula, geometry figure, graph, or chemistry diagram. |
| Handwriting cleanup | Experimental before/after derivative | Overlapping black handwriting and printed strokes are ambiguous; the clean result must never replace the source silently. |
| Cloud education OCR | Optional benchmark/accelerator | Baidu, Tencent, and Alibaba expose question-splitting APIs, but introduce image upload, cost, credentials, regional availability, and model drift. |

## File Map

- Modify: `labs/question-region-bakeoff/requirements.txt` — pin the OCR comparison runtime.
- Create: `labs/question-region-bakeoff/question_bakeoff/engine.py` — common analysis protocol and engine resolver.
- Create: `labs/question-region-bakeoff/question_bakeoff/rapidocr_engine.py` — OCR boxes, question-anchor rules, and conservative region assembly.
- Modify: `labs/question-region-bakeoff/question_bakeoff/cli.py` — add `--engine opencv-whitespace|ppocrv6-small-anchor|ppocrv6-medium-anchor`.
- Modify: `labs/question-region-bakeoff/question_bakeoff/report.py` — run a selected engine and persist comparable runtime/model metadata.
- Create: `labs/question-region-bakeoff/tests/test_rapidocr_engine.py` — deterministic box-to-region tests without loading a real model.
- Modify: `labs/question-region-bakeoff/README.md` — exact consented 60/300-image comparison workflow.
- Create after the gate: `src-tauri/migrations/0014_capture_region_suggestions.sql` — resumable suggestion ledger.
- Create after the gate: `src-tauri/src/modules/capture_suggestions.rs` — account-scoped use cases and validation.
- Create after the gate: `src-tauri/src/commands/capture_suggestions.rs` — typed Tauri commands only.
- Modify after the gate: `src-tauri/src/bindings.rs` and `src-tauri/src/lib.rs` — register/export commands.
- Create after the gate: `src/modules/capture/components/CaptureSuggestionReview.vue` — batch progress and uncertainty review.
- Modify after the gate: `src/modules/capture/components/CaptureCropEditor.vue` — load suggested regions into the existing editor.
- Modify after the gate: `src/modules/capture/CaptureView.vue` — start/cancel/batch-review orchestration.

---

### Task 1: Add a replaceable OCR engine contract to the isolated lab

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/engine.py`
- Modify: `labs/question-region-bakeoff/question_bakeoff/cli.py`
- Modify: `labs/question-region-bakeoff/question_bakeoff/report.py`
- Test: `labs/question-region-bakeoff/tests/test_cli_report.py`

**Interfaces:**
- Consumes: existing `Analysis` and `Suggestion` data structures.
- Produces: `resolve_engine(name: str) -> Callable[[Path], Analysis]` and CLI `run ... --engine <name>`.

- [ ] **Step 1: Write the failing engine-selection tests**

```python
def test_resolve_engine_rejects_unknown_name():
    with pytest.raises(ValueError, match="unsupported question-region engine"):
        resolve_engine("chat-model")

def test_cli_selects_named_engine(monkeypatch, fixture_manifest, tmp_path):
    selected = []
    monkeypatch.setattr("question_bakeoff.cli.write_benchmark_report", lambda *args, engine_name: selected.append(engine_name) or report())
    assert main(["run", str(fixture_manifest), "--output", str(tmp_path / "out"), "--engine", "opencv-whitespace"]) == 0
    assert selected == ["opencv-whitespace"]
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `py -3.12 -m pytest labs/question-region-bakeoff/tests/test_cli_report.py -q`

Expected: FAIL because `resolve_engine` and `--engine` do not exist.

- [ ] **Step 3: Implement the engine registry**

```python
from collections.abc import Callable
from pathlib import Path

from .opencv_baseline import Analysis, analyze_image

Analyzer = Callable[[Path], Analysis]

def resolve_engine(name: str) -> Analyzer:
    engines: dict[str, Analyzer] = {"opencv-whitespace": analyze_image}
    try:
        return engines[name]
    except KeyError as error:
        raise ValueError(f"unsupported question-region engine: {name}") from error
```

Pass the resolved analyzer into `write_benchmark_report`; do not use a module-level mutable engine.

- [ ] **Step 4: Run the lab tests**

Run: `py -3.12 -m pytest labs/question-region-bakeoff/tests -q`

Expected: all existing and new tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add labs/question-region-bakeoff
git commit -m "test: make question region engines comparable"
```

### Task 2: Add PP-OCRv6 anchor-based question suggestions through RapidOCR

**Files:**
- Modify: `labs/question-region-bakeoff/requirements.txt`
- Create: `labs/question-region-bakeoff/question_bakeoff/rapidocr_engine.py`
- Modify: `labs/question-region-bakeoff/question_bakeoff/engine.py`
- Test: `labs/question-region-bakeoff/tests/test_rapidocr_engine.py`

**Interfaces:**
- Consumes: OCR rows shaped as polygon, text, confidence; normalized source dimensions.
- Produces: `suggest_from_ocr_boxes(width, height, boxes) -> tuple[Suggestion, ...]` and model-explicit analyzers using engines `ppocrv6-small-anchor` and `ppocrv6-medium-anchor`.

- [ ] **Step 1: Pin the isolated dependencies**

Add exactly:

```text
rapidocr==3.9.2
onnxruntime==1.27.0
```

These remain under `.tools/question-bakeoff-python` and do not enter `package.json`, `Cargo.toml`, or the signed installer.

- [ ] **Step 2: Write deterministic failing tests for Chinese/English question anchors**

```python
@pytest.mark.parametrize("text", ["1. 已知函数", "12、如图", "（3）求证", "Q4. Choose"])
def test_question_anchor_patterns(text):
    assert is_question_anchor(text)

def test_regions_do_not_cut_detected_formula_or_figure_blocks():
    boxes = [
        box(40, 100, 300, 145, "1. 已知", 0.98),
        box(60, 150, 900, 360, "formula-or-figure", 0.90, kind="non_text"),
        box(40, 400, 300, 445, "2. 求", 0.98),
    ]
    regions = suggest_from_ocr_boxes(1000, 800, boxes)
    assert len(regions) == 2
    assert regions[0].rect.bottom >= 360 / 800
    assert regions[0].rect.bottom <= regions[1].rect.y
```

Also cover two columns, answer pages without printed numbers, low-confidence anchors, Roman/Chinese section headings, option labels that must not start a new question, and one question spanning multiple visual blocks.

- [ ] **Step 3: Run the focused test and verify it fails**

Run: `py -3.12 -m pytest labs/question-region-bakeoff/tests/test_rapidocr_engine.py -q`

Expected: FAIL because `rapidocr_engine.py` does not exist.

- [ ] **Step 4: Implement conservative anchor assembly**

```python
QUESTION_ANCHOR = re.compile(r"^\s*(?:\(?\d{1,3}\)?[.、．)]|Q\d{1,3}[.:])\s*", re.IGNORECASE)
OPTION_ONLY = re.compile(r"^\s*[A-H][.、．)]\s*", re.IGNORECASE)

def is_question_anchor(text: str) -> bool:
    return bool(QUESTION_ANCHOR.match(text)) and not bool(OPTION_ONLY.match(text))
```

Group boxes by detected column, sort top-to-bottom, create a boundary halfway between consecutive high-confidence anchors, then expand each boundary to include every intersecting OCR/non-text block. If fewer than two reliable anchors exist, return one low-confidence full-content region with `uncertain_reason="insufficient_question_anchors"`; never invent confident splits from whitespace alone.

- [ ] **Step 5: Run the isolated runtime and tests**

Run: `.\scripts\question-bakeoff.ps1 -InstallDependencies -SelfCheck`

Expected: JSON reports RapidOCR 3.9.2 and ONNX Runtime 1.27.0 in addition to the existing versions.

Run: `py -3.12 -m pytest labs/question-region-bakeoff/tests -q`

Expected: all tests PASS without network access after dependency/model installation.

- [ ] **Step 6: Commit**

```powershell
git add labs/question-region-bakeoff scripts/question-bakeoff.ps1
git commit -m "feat: benchmark local OCR question anchors"
```

### Task 3: Run the consented 60-image bake-off and decide whether to continue

**Files:**
- Modify: `labs/question-region-bakeoff/README.md`
- Create locally but keep ignored: `labs/question-region-bakeoff/data/manifest.json`
- Create locally but keep ignored: `labs/question-region-bakeoff/output-opencv/`
- Create locally but keep ignored: `labs/question-region-bakeoff/output-ppocr-small/`
- Create locally but keep ignored: `labs/question-region-bakeoff/output-ppocr-medium/`

**Interfaces:**
- Consumes: anonymized photos and human-labeled normalized question rectangles/anchors.
- Produces: two auditable reports with identical metrics and a written go/no-go decision.

- [ ] **Step 1: Prepare 60 authorized, anonymized copies**

Use at least five samples from each difficult class: adjacent questions, two columns, perspective/shadow, geometry, formula-heavy math/physics, chemistry structures, handwriting over print, answer pages, long multi-image questions, and pages without clean question numbers. Remove EXIF/GPS and visible names/class/QR codes before they enter the ignored fixture directory.

- [ ] **Step 2: Validate the manifest**

Run: `.\scripts\question-bakeoff.ps1 validate labs/question-region-bakeoff/data/manifest.json`

Expected: `validated 60 consented sample(s)`.

- [ ] **Step 3: Run both candidates**

```powershell
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json --output labs/question-region-bakeoff/output-opencv --engine opencv-whitespace
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json --output labs/question-region-bakeoff/output-ppocr-small --engine ppocrv6-small-anchor
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json --output labs/question-region-bakeoff/output-ppocr-medium --engine ppocrv6-medium-anchor
```

- [ ] **Step 4: Inspect every overlay and classify failures**

Record `cut_formula`, `cut_figure`, `merged_questions`, `false_option_split`, `column_order`, `handwriting_confusion`, `no_anchor`, and `latency` per sample. A plausible crop that loses an exponent, unit, charge, graph label, or diagram edge is a content-cut failure.

- [ ] **Step 5: Apply the gate**

Continue to 300 images only if at least one PP-OCRv6 tier materially improves question-start recall over OpenCV, creates no regression in content-cut rate, and keeps p95 warm latency under 2 seconds. Prefer small unless medium improves question-start recall by at least two percentage points. Otherwise stop production automation, retain the current manual crop as canonical, and evaluate one opt-in domestic education API on a non-sensitive 20-image subset before reconsidering.

- [ ] **Step 6: Commit only documentation and aggregate metrics**

Do not commit photos, overlays, source filenames, or absolute paths.

```powershell
git add labs/question-region-bakeoff/README.md docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md
git commit -m "docs: record question region model gate"
```

### Task 4: Persist resumable suggestions after the 300-image gate

**Files:**
- Create: `src-tauri/migrations/0014_capture_region_suggestions.sql`
- Create: `src-tauri/src/modules/capture_suggestions.rs`
- Create: `src-tauri/src/commands/capture_suggestions.rs`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/capture_region_suggestions.rs`

**Interfaces:**
- Produces:

```rust
pub struct CaptureRegionSuggestion {
    pub id: String,
    pub item_id: String,
    pub regions: Vec<CaptureCropRecipe>,
    pub engine: String,
    pub engine_version: String,
    pub confidence: f32,
    pub state: SuggestionState,
}

pub fn capture_region_suggest(input: SuggestInput) -> AppResult<CaptureRegionSuggestion>;
pub fn capture_region_suggest_batch(input: SuggestBatchInput) -> AppResult<SuggestionJob>;
pub fn capture_region_suggestion_cancel(job_id: String) -> AppResult<()>;
```

- [ ] **Step 1: Write failing account/profile boundary, cancellation, restart, and geometry tests**

Verify that opaque IDs from another account/profile return the same not-found result, cancellation is observed between images, a restarted job resumes only pending items, corrupt model output is rejected, and every region passes the existing crop recipe validator.

- [ ] **Step 2: Run the focused Rust test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_region_suggestions`

Expected: FAIL because the migration/module/commands do not exist.

- [ ] **Step 3: Add the suggestion ledger**

Store only item IDs, normalized JSON regions, engine metadata, confidence, state (`pending | accepted | rejected | stale`), and timestamps. Never store source paths, decrypted bytes, or OCR text in this table. Mark a suggestion stale when the item is moved, removed, reverted, or superseded by a different derivation.

- [ ] **Step 4: Implement bounded background execution**

Use one inference worker by default, emit progress after each image, check cancellation between images, and keep the UI responsive. Treat model absence as `suggestion_model_unavailable`, not as a capture failure.

- [ ] **Step 5: Generate bindings and run Rust gates**

```powershell
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Expected: bindings are stable and all Rust tests PASS.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri src/shared/api/bindings.ts
git commit -m "feat: persist safe question region suggestions"
```

### Task 5: Add one-click desktop review without changing phone capture speed

**Files:**
- Create: `src/modules/capture/components/CaptureSuggestionReview.vue`
- Create: `src/modules/capture/components/CaptureSuggestionReview.test.ts`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.test.ts`
- Modify: `src/modules/capture/CaptureView.vue`
- Modify: `src/modules/capture/CaptureView.test.ts`

**Interfaces:**
- Consumes: typed suggestion/job commands from Task 4.
- Produces: toolbar action `自动框题`, cancellable batch progress, uncertainty queue, and accepted regions passed to the existing `captureCropApply` command.

- [ ] **Step 1: Write failing interaction tests**

Cover: model-unavailable fallback, cancel, retry, high/low-confidence labels, opening suggested regions in the existing crop editor, changing a region before apply, rejecting all, reduced motion, keyboard focus, and no new mobile upload step.

- [ ] **Step 2: Run the focused Vue tests and verify they fail**

Run: `pnpm vitest run src/modules/capture/components/CaptureSuggestionReview.test.ts src/modules/capture/components/CaptureCropEditor.test.ts src/modules/capture/CaptureView.test.ts`

Expected: FAIL because the suggestion review UI is absent.

- [ ] **Step 3: Implement the review flow**

Use three explicit states: `正在分析`, `建议 N 个区域`, and `需要确认`. High confidence may preselect regions but still requires the existing “生成 N 张裁剪图” action. Low confidence opens the editor immediately. Keep `手动裁剪` permanently available beside `自动框题`.

- [ ] **Step 4: Run frontend and production gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Expected: all commands PASS; initial JS stays below 300 KB gzip and no OCR/model bytes enter a frontend chunk.

- [ ] **Step 5: Real-device acceptance**

On the 4-core/8 GB Windows reference machine, confirm one-image p95 below 2 seconds, cancellation feedback below 250 ms, 150-image batch scrolling remains smooth, and the phone can continue taking/uploading photos while desktop analysis is pending.

- [ ] **Step 6: Commit**

```powershell
git add src/modules/capture
git commit -m "feat: review automatic question regions on desktop"
```

## Separate Follow-up Plans (Do Not Fold Into This Feature)

1. **Hybrid OCR content:** verified text blocks for search/copy, verified LaTeX for formulas, and preserved image blocks for geometry, plots, maps, apparatus, and chemistry.
2. **Handwriting cleanup:** opt-in before/after preview with an editable mask; first compare Tencent Cloud handwriting erasure and one domestic alternative on non-sensitive fixtures.
3. **Cloud question splitting:** provider-neutral backend adapter for Tencent/Baidu/Alibaba education OCR, explicit privacy/cost disclosure, per-batch consent, quota, idempotency, and local manual fallback.
4. **Raw-source retention policy:** after a verified crop and at least 30 days, let the user explicitly purge the raw source; never claim OCR alone provides the storage saving.

## Research References

- Google ML Kit document scanner: <https://developers.google.com/ml-kit/vision/doc-scanner>
- Apple VisionKit document camera: <https://developer.apple.com/documentation/visionkit>
- Adobe Scan crop and auto-detect workflow: <https://www.adobe.com/devnet-docs/adobescan/android/en/scan.html>
- RapidOCR: <https://github.com/RapidAI/RapidOCR>
- PaddleOCR: <https://www.paddleocr.ai/>
- Tencent Cloud question splitting: <https://cloud.tencent.com/document/product/866/115930>
- Baidu Cloud education paper cutting: <https://cloud.baidu.com/doc/OCR/s/Tlqom5gvh>
- Alibaba Cloud education paper cutting: <https://help.aliyun.com/en/ocr/developer-reference/api-recognizeedupapercut>
- Tencent Cloud handwriting erasure: <https://cloud.tencent.com/document/product/866/133907>
- Mathpix STEM OCR: <https://docs.mathpix.com/>
- pix2tex formula OCR: <https://github.com/lukas-blecher/LaTeX-OCR>
- MolScribe chemistry recognition: <https://github.com/thomas0809/MolScribe>
