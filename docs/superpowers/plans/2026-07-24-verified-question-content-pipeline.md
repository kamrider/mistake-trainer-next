# Verified Question Content Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> Do not start production tasks until the evidence gate in Task 1 passes.

**Goal:** Turn verified mistake-question images into editable text, LaTeX, and preserved
visual blocks through a local-first, auditable pipeline without damaging the encrypted
source or making an untrusted OCR result canonical.

**Architecture:** Use a staged cascade instead of running all of PP-StructureV3 for every
photo. OpenCV performs non-destructive quality checks and conditional perspective
correction; UVDoc is optional for detected curvature; PP-OCRv6 small finds question
anchors and reading structure; PP-DocLayout-M protects formulas, figures, and tables;
PP-OCRv6 medium and PP-FormulaNet run only when the user requests content extraction.
PaddleOCR-VL-1.6 is an optional difficult-page fallback and never a required Windows v1
dependency.

**Tech Stack:** Existing isolated Python 3.12 bake-off, OpenCV, RapidOCR as a lightweight
ONNX host/baseline, PP-OCRv6 small/medium, PP-DocLayout-M, optional UVDoc,
PP-FormulaNet, optional PaddleOCR-VL-1.6, Tauri/Rust worker boundary, SQLCipher,
AES-GCM assets, Vue 3 review UI.

## Global Constraints

- This plan follows
  `docs/superpowers/plans/2026-07-23-ppocrv6-question-content-bakeoff.md` and
  `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md`.
- Do not start product integration until the consented 60-image comparison is complete
  and the existing 300-image region safety gate passes.
- PP-OCRv6 small is the default anchor/structure candidate. PP-OCRv6 medium is an
  on-demand accuracy candidate, not an unconditional startup dependency.
- RapidOCR remains a replaceable lightweight runtime host and benchmark baseline; the
  persisted model identity must say `PP-OCRv6 small|medium`, never merely `RapidOCR`.
- Do not run full PP-StructureV3, tables, seals, charts, formula recognition, or UVDoc on
  every image.
- Preserve the encrypted color source permanently in this phase. Grayscale, CLAHE,
  thresholded, rectified, and dewarped images are disposable derivatives.
- Never erase pen circles, ticks, handwriting, formulas, or diagram strokes from the
  canonical source. Low-confidence content is visibly uncertain, not silently repaired.
- A printed or handwritten OCR result is an editable draft until the user confirms it.
- Geometry, graphs, maps, apparatus, molecule structures, and other non-text content
  remain image blocks. The application must not pretend they are faithfully reconstructable
  as plain Markdown.
- Persist block coordinates, confidence, engine/model versions, preprocessing recipe,
  source asset revision, and confirmation state with every derivative.
- Handwritten recognition cannot auto-publish. Even the best candidate must retain its
  source crop and expose low-confidence spans for correction.
- PaddleOCR-VL-1.6 is opt-in and separately installed or remotely configured. Failure or
  absence must fall back to the verified image/manual workflow.
- A custom complete-question detector may use Apache-2.0 PaddleDetection/RT-DETR only
  after consented corrected regions provide a representative training set. Do not add an
  AGPL YOLO runtime to the closed-source desktop application.
- OCR-derived text is for search, copy, accessibility, and editing. It does not justify
  deleting source images or claiming immediate storage savings.

## File Map

- Extend: `labs/question-region-bakeoff/` — preprocessing, layout, transcription, and
  formula/VL comparison harnesses.
- Create after gates:
  `src-tauri/migrations/0015_question_content_derivatives.sql` — derivative/job ledger.
- Create after gates:
  `src-tauri/src/modules/question_content.rs` — domain validation and confirmation rules.
- Create after gates:
  `src-tauri/src/infrastructure/question_content_worker.rs` — isolated local worker.
- Create after gates:
  `src-tauri/src/commands/question_content.rs` — opaque typed command boundary.
- Modify after gates:
  `src-tauri/src/bindings.rs`, `src-tauri/src/lib.rs`, and backup validation.
- Create after gates:
  `src/modules/capture/components/QuestionContentReview.vue` — source/block/Markdown
  review surface.
- Modify after gates:
  capture and library detail views to expose the confirmed derivative.
- Create: `docs/windows-question-content-acceptance.md` — real-device and privacy checks.

---

### Task 1: Lock the evidence gate and representative labels

**Files:**
- Modify: `labs/question-region-bakeoff/README.md`
- Create locally and keep ignored:
  `labs/question-region-bakeoff/data/content-manifest.json`
- Create locally and keep ignored:
  `labs/question-region-bakeoff/output-content/`

**Interfaces:**
- Consumes the same authorized images used by the region bake-off.
- Produces aggregate region, printed-text, handwriting, formula, visual-block, latency,
  and peak-memory metrics without committing images or filenames.

- [ ] **Step 1: Define the content manifest schema**

Add labels for `printed`, `printed_with_pen_marks`, `printed_answer`, `handwritten_note`,
`handwritten_answer`, `formula`, `geometry`, `graph`, `chemistry_structure`,
`apparatus`, `table`, `two_column`, `curved_page`, `perspective`, `shadow`,
`low_contrast`, and `blurred`. Each block contains normalized coordinates, expected
kind, and an optional human transcription; never store a student's name or document
path in report JSON.

- [ ] **Step 2: Add deterministic manifest validation tests**

Reject missing consent, coordinates outside `[0,1]`, overlapping identifiers, empty
expected text for a labeled transcription block, source files with EXIF/GPS, and a set
without at least five examples of every required difficult class.

Run:

```powershell
& $env:QUESTION_BAKEOFF_PYTHON -m unittest discover `
  -s labs/question-region-bakeoff/tests -v
```

Expected: all tests pass and validation reports exact counts by difficult class.

- [ ] **Step 3: Apply the production gate**

Proceed beyond the lab only when:

- question-start recall is at least `95%`;
- content-cut rate is below `0.5%`;
- false-split rate is below `3%`;
- p95 one-image warm latency is below `2 s` on the 4-core/8 GB Windows reference
  machine;
- printed transcription and formula metrics are reported separately;
- no handwritten result is classified as trusted without review.

If the gate fails, retain manual crop/image cards and continue collecting consented
corrections. Do not lower the thresholds to ship the feature.

---

### Task 2: Benchmark conditional preprocessing and UVDoc

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/preprocessing.py`
- Create: `labs/question-region-bakeoff/tests/test_preprocessing.py`
- Modify: benchmark report metadata and README.

**Interfaces:**

```python
@dataclass(frozen=True)
class PreprocessDecision:
    source_quality: QualityMetrics
    page_quad: tuple[Point, Point, Point, Point] | None
    perspective_applied: bool
    uvdoc_recommended: bool
    variants: tuple[DerivedVariant, ...]
```

- [ ] **Step 1: Test blur, exposure, page-quad, and curvature decisions**

Use generated fixtures for sharp/blurred pages, clipped highlights, deep shadows,
perspective, curved text lines, and no-page backgrounds. Test that poor quality returns
an actionable retake reason, reliable quads allow perspective correction, and UVDoc is
recommended only for measured curvature.

- [ ] **Step 2: Implement non-destructive variants**

Always keep `source_color`. Optionally produce `perspective_color`,
`clahe_luminance`, and `adaptive_threshold_candidate`. OCR may compare variants, but
the report records which one won and none replaces the source.

- [ ] **Step 3: Compare UVDoc only on curved-page labels**

Measure cold/warm latency, working set, OCR confidence change, and content-cut change.
Enable UVDoc in the future worker only when curvature classification improves the
authorized set and does not exceed the product latency budget.

---

### Task 3: Protect visual blocks without treating layout as a question detector

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/layout_engine.py`
- Create: `labs/question-region-bakeoff/tests/test_layout_engine.py`
- Modify: overlay/report generation.

**Interfaces:**

```python
BlockKind = Literal["text", "formula", "figure", "table", "unknown"]

@dataclass(frozen=True)
class ProtectedBlock:
    rect: NormalizedRect
    kind: BlockKind
    confidence: float
    engine: str
    engine_version: str
```

- [ ] **Step 1: Compare PP-DocLayout-M with the OpenCV visual-block baseline**

Evaluate whether formula, geometry, graph, chemistry, apparatus, and table blocks are
kept inside conservative question regions. PP-DocLayout-M may expand/protect a region;
it must never independently claim “this is one complete question.”

- [ ] **Step 2: Test anchor-plus-block assembly**

Cover question numbers, options that are not question starts, two columns, a formula
between anchors, one diagram shared with adjacent text, and unequal answer-page
numbering. Any uncertain grouping falls back to one larger region or manual review.

- [ ] **Step 3: Record the selected layout tier**

Prefer the medium/light layout model if it meets the content-cut gate. Do not bundle the
126 MB high-accuracy layout tier merely because it reports a higher generic benchmark.

---

### Task 4: Benchmark verified transcription, formulas, and the optional VL fallback

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/content_engine.py`
- Create: `labs/question-region-bakeoff/tests/test_content_engine.py`
- Modify: CLI engine registry and aggregate report.

**Interfaces:**

```python
@dataclass(frozen=True)
class ContentBlock:
    kind: Literal["markdown", "latex", "image"]
    rect: NormalizedRect
    content: str | None
    confidence: float | None
    requires_review: bool
    engine: str
    engine_version: str
```

- [ ] **Step 1: Compare PP-OCRv6 small and medium by content class**

Report printed Chinese, printed English, printed answers, pen-mark noise, handwritten
Chinese, and handwritten answers separately. Select small for anchors unless medium
improves recall by at least two percentage points; select medium for on-demand content
only if its accuracy benefit survives the latency/memory gate.

- [ ] **Step 2: Add formula extraction as an explicit block path**

Send only protected formula blocks to the chosen PP-FormulaNet tier. Test fractions,
roots, exponents, subscripts, inequalities, chemical charges, and multiline equations.
If confidence or syntax validation fails, return an image block with an editable LaTeX
draft rather than fabricated final math.

- [ ] **Step 3: Preserve non-text blocks**

Geometry, graphs, maps, apparatus, molecule structures, and ambiguous visual regions
produce `kind="image"` with no invented text. Ordinary OCR text around them remains
ordered Markdown blocks.

- [ ] **Step 4: Compare PaddleOCR-VL-1.6 on a small difficult subset**

Run only formula-heavy, severe-noise, complex-layout, and difficult handwriting samples.
Record additional accuracy, latency, working set, download size, and failure rate.
Adopt it only as an optional fallback behind explicit user choice; never make app startup
or ordinary capture depend on it.

---

### Task 5: Persist auditable derivatives after the gates pass

**Files:**
- Create: `src-tauri/migrations/0015_question_content_derivatives.sql`
- Create: `src-tauri/src/modules/question_content.rs`
- Create: `src-tauri/src/infrastructure/question_content_worker.rs`
- Create: `src-tauri/src/commands/question_content.rs`
- Test: `src-tauri/tests/question_content.rs`

**Interfaces:**

```rust
pub struct QuestionContentDerivative {
    pub id: String,
    pub source_asset_id: String,
    pub source_revision: i64,
    pub blocks: Vec<QuestionContentBlock>,
    pub preprocess_recipe: String,
    pub model_manifest: Vec<ModelVersion>,
    pub state: DerivativeState, // draft | confirmed | stale
    pub created_at_utc_ms: i64,
    pub confirmed_at_utc_ms: Option<i64>,
}
```

- [ ] **Step 1: Test ownership, provenance, staleness, and backup**

Reject cross-account/profile IDs, corrupt coordinates, missing model versions, oversized
Markdown/LaTeX, and a derivative whose source revision changed. Confirm that encrypted
backup/restore includes confirmed and draft derivatives but never model caches or
decrypted temporary images.

- [ ] **Step 2: Add a single-worker bounded job queue**

One worker processes one image at a time, checks cancellation between stages, removes
decrypted temporary files on every exit path, emits bounded progress, and resumes only
pending jobs after restart. Model absence returns a stable `model_unavailable` result and
leaves the source/capture batch untouched.

- [ ] **Step 3: Generate typed commands**

Expose opaque-ID commands to request, cancel, read, confirm, edit, and discard a
derivative. Vue never receives model paths, source file paths, subprocess arguments,
credentials, or raw diagnostic logs.

---

### Task 6: Build the user confirmation workflow

**Files:**
- Create: `src/modules/capture/components/QuestionContentReview.vue`
- Create: `src/modules/capture/components/QuestionContentReview.test.ts`
- Modify: capture and library detail views.
- Create: `docs/windows-question-content-acceptance.md`

- [ ] **Step 1: Test the source/block/preview editing loop**

Cover printed text, formula, preserved image blocks, low-confidence highlights,
handwritten draft warnings, keyboard navigation, undo, cancellation, stale-source
recovery, reduced motion, and failure without data loss.

- [ ] **Step 2: Implement explicit states**

Use `等待处理`, `正在分析`, `需要确认`, `已确认`, `原图已变化`, and `模型未安装`.
Show the color source beside the derivative. A user can correct Markdown/LaTeX, change a
block back to image, merge/split blocks, or discard the derivative without touching the
problem image.

- [ ] **Step 3: Keep installation choices honest**

Ship only the model tier that passes the default gate. Present medium and VL downloads
with exact disk size, source/license, and removal controls. Never download a model merely
because the user opens the capture page.

- [ ] **Step 4: Run production and real-device gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

Verify cold start, first-model load, warm processing, cancellation, 150-image capture
responsiveness, keyboard/focus behavior, backup/restore, and local-only operation on the
Windows reference machine.

## Later learning loop

Only after users explicitly consent to share corrected region labels, accumulate
representative corrections by subject, page layout, camera quality, and pen-mark class.
Train an Apache-2.0 PaddleDetection/RT-DETR complete-question detector only when held-out
evaluation proves it beats the anchor-plus-layout pipeline under the same safety gate.
Keep a manual fallback and make model rollback possible.

## Self-Review

- Spec coverage: conditional OpenCV/UVDoc, PP-OCRv6 small/medium, PP-DocLayout-M,
  PP-FormulaNet, optional PaddleOCR-VL-1.6, pen marks, printed and handwritten content,
  Markdown/LaTeX/image blocks, provenance, original retention, and future RT-DETR are
  each assigned to a concrete task.
- Scope boundary: this is a gated future plan. It does not authorize model downloads,
  schema changes, or product integration before real-image evidence exists.
- Placeholder scan: no TBD/TODO or unspecified “handle errors” step remains.
- Type consistency: the lab `ContentBlock` maps to the Rust `QuestionContentBlock`; both
  carry kind, bounds, confidence, review state, engine, and version.
