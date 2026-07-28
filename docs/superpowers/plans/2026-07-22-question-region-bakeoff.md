# Question Region Bake-off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a disposable, offline benchmark lab that measures whether a small OpenCV question-region baseline is safe enough to justify later RapidOCR/model experiments, without adding any runtime or model to the signed app.

**Architecture:** A standalone Python package under `labs/` loads an explicitly consented manifest of normalized ground-truth rectangles and anchors, runs a deterministic OpenCV page/whitespace baseline, computes reproducible safety metrics, and writes JSON plus visual overlays. The lab never reads the application database or encrypted blob store; the product continues to expose only manual, confirmed crop operations.

**Tech Stack:** Python 3.12, NumPy 2.3.5, `opencv-python-headless` 5.0.0.93, standard-library `unittest`, JSON/HTML reports, PowerShell runner with dependencies installed only under ignored `.tools/`.

## Global Constraints

- Do not add Python, OpenCV, ONNX Runtime, RapidOCR, or model files to the Tauri application, installer, Cargo graph, pnpm graph, or production commands.
- Do not read production databases, encryption keys, blob paths, or user photos; every benchmark image must be copied into an explicitly consented and anonymized fixture directory.
- All rectangles and anchor points use validated finite normalized `[0, 1]` coordinates.
- No automatic result may mutate an image or create a capture asset; this lab produces reports only.
- The 60-image gate remains advisory and the 300-image thresholds remain unchanged: question-start recall at least 95%, content-cut rate below 0.5%, false split rate below 3%, and every uncertain case visibly flagged.
- Test fixtures are synthetic and prove mechanics only; they are not evidence that the product thresholds pass.
- Output directories, copied benchmark photos, local Python dependencies, and caches must remain ignored by Git.

---

### Task 1: Define and validate the benchmark contract

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/schema.py`
- Create: `labs/question-region-bakeoff/question_bakeoff/metrics.py`
- Create: `labs/question-region-bakeoff/question_bakeoff/__init__.py`
- Create: `labs/question-region-bakeoff/tests/test_schema_metrics.py`
- Create: `labs/question-region-bakeoff/fixtures/manifest.example.json`

**Interfaces:**
- Consumes: manifest JSON `{ "schemaVersion": 1, "consent": {...}, "samples": [...] }` with relative image paths, layout/tags, normalized ground-truth `regions`, and optional `anchors`.
- Produces: `NormalizedRect`, `NormalizedPoint`, `BenchmarkSample`, `BenchmarkManifest`, `Suggestion`, `load_manifest(path)`, `evaluate_sample(sample, suggestions)`, and `aggregate_metrics(results)`.

- [x] **Step 1: Write failing validation and metric tests**

Cover non-finite/out-of-bounds rectangles, absolute or escaping image paths, missing affirmative consent, deterministic maximum-IoU matching, unmatched ground truth as 100% cut, unmatched predictions as false splits, anchor tolerance, and aggregate counters.

- [x] **Step 2: Run the tests and verify the package is missing**

Run:

```powershell
$env:PYTHONPATH='labs/question-region-bakeoff'
python -m unittest discover -s labs/question-region-bakeoff/tests -p 'test_schema_metrics.py' -v
```

Expected: FAIL because `question_bakeoff.schema` and `question_bakeoff.metrics` do not exist.

- [x] **Step 3: Implement immutable validated types and metric formulas**

`NormalizedRect` validates `x >= 0`, `y >= 0`, `width > 0`, `height > 0`, `x + width <= 1`, and `y + height <= 1` with a `1e-9` tolerance. `load_manifest` resolves every relative image against the manifest directory and rejects any resolved path outside it. Consent must contain `anonymized: true`, `authorizedForLocalEvaluation: true`, and a non-empty ISO date.

Match regions greedily by descending IoU. Report:

- `regionRecall = matched ground-truth count / ground-truth count` at IoU `>= 0.5`;
- `meanMatchedIou` over matched pairs;
- `contentCutRate = sum(ground-truth area not covered by its matched prediction) / total ground-truth area`, with unmatched truth fully cut;
- `falseSplitRate = unmatched prediction count / prediction count`;
- `questionStartRecall` using same-column prediction tops within normalized `0.035` of each ground-truth anchor;
- `uncertainCount` for suggestions with confidence below `0.75`.

- [x] **Step 4: Add a safe example manifest and rerun tests**

The example manifest contains metadata and rectangle examples but references only `images/replace-with-consented-copy.png`; no real photo is committed. Expected: all schema/metric tests PASS.

---

### Task 2: Implement the OpenCV document and whitespace baseline

**Files:**
- Create: `labs/question-region-bakeoff/requirements.txt`
- Create: `labs/question-region-bakeoff/question_bakeoff/opencv_baseline.py`
- Create: `labs/question-region-bakeoff/tests/test_opencv_baseline.py`

**Interfaces:**
- Consumes: decoded BGR `numpy.ndarray` and an `engine_version` constant.
- Produces: `detect_page_quad(image)`, `warp_page(image, quad)`, `estimate_skew_degrees(image)`, `detect_columns(binary)`, `suggest_question_regions(image) -> list[Suggestion]`, and `analyze_image(path) -> Analysis`.

- [x] **Step 1: Write failing synthetic-image tests**

Generate images in memory containing: a single page boundary, three vertically separated ink blocks, two columns separated by a stable gutter, a component crossing a weak gap, and an almost blank page. Assert ordered normalized regions, no boundary through foreground pixels, two-column x ranges, finite confidence, and blank-page uncertainty.

- [x] **Step 2: Install only the disposable lab dependency and verify failure**

Install into ignored `.tools/question-bakeoff-python`:

```powershell
python -m pip install --target .tools/question-bakeoff-python -r labs/question-region-bakeoff/requirements.txt
$env:PYTHONPATH='labs/question-region-bakeoff;.tools/question-bakeoff-python'
python -m unittest labs/question-region-bakeoff/tests/test_opencv_baseline.py -v
```

Expected: FAIL because `opencv_baseline` does not exist.

- [x] **Step 3: Implement deterministic preprocessing and suggestions**

Use grayscale, Gaussian blur, Canny, largest convex four-corner contour, and perspective warp for page detection. Use Otsu inverse threshold plus foreground `minAreaRect` for skew estimates, clamped to `[-15, 15]`. Detect a two-column gutter only when a central low-ink run is at least 3% of page width and each side contains at least 15% of total ink. Within each column, smooth horizontal ink density, group active runs, merge gaps below 2% page height, pad by 1%, and expand boundaries to include connected components. Return the whole page as one low-confidence suggestion when reliable splitting evidence is absent.

- [x] **Step 4: Run OpenCV and schema tests**

Expected: all synthetic tests PASS. Record OpenCV engine/version in every analysis result.

---

### Task 3: Produce auditable reports and overlays

**Files:**
- Create: `labs/question-region-bakeoff/question_bakeoff/report.py`
- Create: `labs/question-region-bakeoff/question_bakeoff/cli.py`
- Create: `labs/question-region-bakeoff/tests/test_cli_report.py`

**Interfaces:**
- Consumes: validated manifest and `analyze_image` results.
- Produces: `python -m question_bakeoff.cli validate <manifest>`, `run <manifest> --output <directory>`, `report.json`, `index.html`, and per-sample `overlays/<id>.png`.

- [x] **Step 1: Write the failing end-to-end CLI test**

Create a temporary synthetic image and manifest, invoke `cli.main([...])`, and assert zero exit code, deterministic JSON schema, engine metadata, per-sample metrics, aggregate metrics, threshold verdict, runtime milliseconds, and an overlay whose dimensions equal the decoded image. Verify an existing output directory is replaced only when it contains the lab-owned `.question-bakeoff-output` marker.

- [x] **Step 2: Run the CLI test and verify it fails**

Expected: FAIL because `question_bakeoff.cli` does not exist.

- [x] **Step 3: Implement atomic, path-safe reporting**

Write into an adjacent temporary directory, generate overlays with ground truth in green, suggestions in amber/red by confidence, and anchors as circles, then rename into place. HTML must embed only escaped relative links and report data; it must not embed source images, absolute paths, or arbitrary manifest text as HTML. A failed run deletes its temporary output and leaves an existing report untouched.

- [x] **Step 4: Run all lab tests**

Run:

```powershell
$env:PYTHONPATH='labs/question-region-bakeoff;.tools/question-bakeoff-python'
python -m unittest discover -s labs/question-region-bakeoff/tests -v
```

Expected: all tests PASS.

---

### Task 4: Add a reproducible Windows entry point and documentation

**Files:**
- Create: `scripts/question-bakeoff.ps1`
- Create: `labs/question-region-bakeoff/README.md`
- Modify: `.gitignore`
- Modify: `docs/superpowers/plans/2026-07-22-question-crop-and-extraction.md`

**Interfaces:**
- Consumes: `scripts/question-bakeoff.ps1 -InstallDependencies -- validate <manifest>` or `-- run <manifest> --output <directory>`.
- Produces: an explicit consent-first local workflow and a decision report that cannot be confused with production acceptance.

- [x] **Step 1: Add runner contract tests or self-check mode**

The PowerShell runner must locate `$env:QUESTION_BAKEOFF_PYTHON`, `py -3.12`, or `python`; install pinned dependencies only with `-InstallDependencies`; prepend only the lab and ignored dependency directory to `PYTHONPATH`; and forward all remaining arguments. `--self-check` prints Python, NumPy, and OpenCV versions plus the engine identifier.

- [x] **Step 2: Document fixture preparation and privacy**

Document how to copy—not link—60 anonymized images, remove EXIF/GPS, mark consent, label rectangles/anchors, validate, run, inspect overlays, and interpret each threshold. State prominently that synthetic test success is not a model decision and that a failed gate leaves automatic splitting out of the product.

- [x] **Step 3: Update the roadmap status and ignore generated data**

Ignore `labs/question-region-bakeoff/data/`, `labs/question-region-bakeoff/output/`, Python caches, and local virtual environments. Update the roadmap to mark the benchmark harness available while the 60-image evaluation and RapidOCR comparison remain pending.

- [x] **Step 4: Run final gates and inspect the report**

Run lab tests, `scripts/question-bakeoff.ps1 --self-check`, `git diff --check`, `pnpm lint`, and the existing Rust capture tests. Generate one report from a temporary synthetic fixture and visually inspect its overlay and HTML; do not commit the generated report.

- [x] **Step 5: Create a local baseline commit**

Commit only the lab, runner, roadmap, ignore rule, and this plan with:

```powershell
git commit -m "chore: add question region bakeoff lab"
```
