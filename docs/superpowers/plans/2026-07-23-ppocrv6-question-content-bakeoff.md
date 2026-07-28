# PP-OCRv6 Question Content Bake-off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing local OCR experiment explicitly and audibly compare PP-OCRv6 small and medium for printed questions, pen-mark noise, answer text, and handwritten notes before any OCR runtime enters the Windows application.

**Architecture:** Keep RapidOCR 3.9.2 as the isolated ONNX Runtime host because it already ships PP-OCRv6 tiny/small/medium model resolution. Replace the ambiguous `rapidocr-anchor` identity with model-explicit analyzers, share the conservative question-anchor assembly, and record exact runtime/model metadata. OpenCV remains a non-destructive quality/preprocessing layer; UVDoc, PP-DocLayout-M, formula recognition, Markdown reconstruction, and PaddleOCR-VL remain separate later candidates.

**Tech Stack:** Python 3.12 isolated under `.tools/question-bakeoff-python`, RapidOCR 3.9.2, PP-OCRv6 small/medium ONNX models, ONNX Runtime 1.27.0, OpenCV 5.0.0.93, existing consent-first benchmark schema and report UI.

## Global Constraints

- `RapidOCR` is a runtime host, not a competing OCR model; its current default is PP-OCRv6 small.
- Do not add PaddlePaddle, PaddleX, PP-StructureV3, UVDoc, OpenVINO, or VLM weights to `package.json`, `Cargo.toml`, or the signed installer.
- Keep the existing encrypted source image canonical; OCR text and Markdown are future editable derivatives.
- Never erase circles, pen strokes, handwriting, formulas, or diagrams from the source.
- Do not force grayscale or binary preprocessing; derived variants may be compared without replacing the color source.
- PP-OCRv6 small is the fast candidate for question anchors; medium is an accuracy candidate for printed content and handwriting, not an assumed winner.
- A handwritten OCR result is never trusted automatically. Official PP-OCRv6 medium handwriting accuracy is not high enough to bypass user confirmation.
- No production command, database migration, or capture UI is allowed before the consented 60-image comparison and the existing 300-image safety gate pass.
- The 60-image set must include printed questions with blue/black/red pen circles, printed answers, handwritten answers/notes, formulas, geometry, chemistry, low contrast, perspective, shadows, and two-column pages.

## File Map

- Modify: `labs/question-region-bakeoff/question_bakeoff/rapidocr_engine.py` — explicit PP-OCRv6 analyzer factory, cached runtime, and auditable model identity.
- Modify: `labs/question-region-bakeoff/question_bakeoff/engine.py` — resolve small and medium engine names; preserve one deprecated alias during the lab transition.
- Modify: `labs/question-region-bakeoff/question_bakeoff/cli.py` — report model-explicit available engines.
- Modify: `labs/question-region-bakeoff/tests/test_rapidocr_engine.py` — analyzer configuration, caching, and model identity tests.
- Modify: `labs/question-region-bakeoff/tests/test_cli_report.py` — resolver and self-check expectations.
- Modify: `labs/question-region-bakeoff/README.md` — exact three-way comparison and printed/handwritten failure taxonomy.
- Modify: `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md` — correct the runtime/model terminology and gate inputs.

---

### Task 1: Make PP-OCRv6 model identity explicit

**Files:**
- Modify: `labs/question-region-bakeoff/question_bakeoff/rapidocr_engine.py`
- Modify: `labs/question-region-bakeoff/tests/test_rapidocr_engine.py`

**Interfaces:**
- Consumes: RapidOCR parameter keys `Det.ocr_version`, `Det.model_type`, `Rec.ocr_version`, and `Rec.model_type`.
- Produces: `make_analyzer(model_type: Literal["small", "medium"]) -> Analyzer`, `engine_name_for(model_type: str) -> str`, and version metadata containing RapidOCR, PP-OCRv6, ONNX Runtime, and model tier.

- [x] **Step 1: Write failing analyzer identity and configuration tests**

```python
class AnalyzerConfigurationTests(unittest.TestCase):
    def test_small_and_medium_use_explicit_ppocrv6_models(self) -> None:
        for model_type in ("small", "medium"):
            with self.subTest(model_type=model_type):
                factory = Mock()
                engine = Mock(return_value=FakeRapidResult())
                factory.return_value = engine
                analyzer = make_analyzer(model_type, runtime_factory=factory)
                analyzer(self.fixture)
                factory.assert_called_once_with(
                    params={
                        "Det.ocr_version": OCRVersion.PPOCRV6,
                        "Det.model_type": (
                            ModelType.SMALL
                            if model_type == "small"
                            else ModelType.MEDIUM
                        ),
                        "Rec.ocr_version": OCRVersion.PPOCRV6,
                        "Rec.model_type": (
                            ModelType.SMALL
                            if model_type == "small"
                            else ModelType.MEDIUM
                        ),
                    }
                )
                self.assertEqual(
                    analyzer(self.fixture).engine,
                    f"ppocrv6-{model_type}-anchor",
                )

    def test_analyzer_reuses_one_runtime_between_images(self) -> None:
        factory = Mock(return_value=Mock(return_value=FakeRapidResult()))
        analyzer = make_analyzer("small", runtime_factory=factory)
        analyzer(self.fixture)
        analyzer(self.fixture)
        self.assertEqual(factory.call_count, 1)
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
$env:PYTHONPATH="$PWD\labs\question-region-bakeoff;$PWD\.tools\question-bakeoff-python"
& $env:QUESTION_BAKEOFF_PYTHON -m unittest discover `
  -s labs\question-region-bakeoff\tests `
  -p 'test_rapidocr_engine.py' -v
```

Expected: FAIL because `make_analyzer` and explicit model configuration do not exist.

- [x] **Step 3: Implement the explicit cached analyzer**

```python
from collections.abc import Callable
from typing import Literal

ModelTier = Literal["small", "medium"]

def engine_name_for(model_type: ModelTier) -> str:
    return f"ppocrv6-{model_type}-anchor"

def make_analyzer(
    model_type: ModelTier,
    *,
    runtime_factory: Callable[..., Any] | None = None,
) -> Analyzer:
    if model_type not in {"small", "medium"}:
        raise ValueError(f"unsupported PP-OCRv6 model tier: {model_type}")
    runtime: Any | None = None

    def analyze(path: str | Path) -> Analysis:
        nonlocal runtime
        if runtime is None:
            from rapidocr import ModelType, OCRVersion, RapidOCR
            factory = runtime_factory
            if factory is None:
                factory = RapidOCR
            model_tier = (
                ModelType.SMALL
                if model_type == "small"
                else ModelType.MEDIUM
            )
            runtime = factory(
                params={
                    "Det.ocr_version": OCRVersion.PPOCRV6,
                    "Det.model_type": model_tier,
                    "Rec.ocr_version": OCRVersion.PPOCRV6,
                    "Rec.model_type": model_tier,
                }
            )
        return _analyze_with_runtime(
            path,
            runtime=runtime,
            engine_name=engine_name_for(model_type),
            engine_version=(
                f"rapidocr-3.9.2+ppocrv6-{model_type}"
                "+onnxruntime-1.27.0+anchor-1.1.0"
            ),
        )

    return analyze
```

Move image decoding, result adaptation, conservative region assembly, page-quad observation, and timing into `_analyze_with_runtime`. Do not duplicate anchor rules.

- [x] **Step 4: Run the focused and full lab tests**

Run:

```powershell
& $env:QUESTION_BAKEOFF_PYTHON -m unittest discover `
  -s labs\question-region-bakeoff\tests -v
```

Expected: all tests PASS.

- [x] **Step 5: Commit**

```powershell
git add labs/question-region-bakeoff/question_bakeoff/rapidocr_engine.py `
        labs/question-region-bakeoff/tests/test_rapidocr_engine.py
git commit -m "refactor: identify PP-OCRv6 benchmark models"
```

### Task 2: Register small and medium comparison engines

**Files:**
- Modify: `labs/question-region-bakeoff/question_bakeoff/engine.py`
- Modify: `labs/question-region-bakeoff/question_bakeoff/cli.py`
- Modify: `labs/question-region-bakeoff/tests/test_cli_report.py`

**Interfaces:**
- Consumes: `make_analyzer("small")` and `make_analyzer("medium")`.
- Produces: CLI engine names `ppocrv6-small-anchor` and `ppocrv6-medium-anchor`; the old `rapidocr-anchor` resolves to small only during this lab transition.

- [x] **Step 1: Write failing resolver and self-check tests**

```python
def test_resolver_exposes_model_explicit_engines(self) -> None:
    self.assertEqual(
        resolve_engine("ppocrv6-small-anchor").model_type,
        "small",
    )
    self.assertEqual(
        resolve_engine("ppocrv6-medium-anchor").model_type,
        "medium",
    )

def test_self_check_reports_model_explicit_engines(self) -> None:
    report = run_self_check()
    self.assertEqual(
        report["availableEngines"],
        [
            "opencv-whitespace",
            "ppocrv6-small-anchor",
            "ppocrv6-medium-anchor",
        ],
    )
```

Use a stable analyzer attribute `model_type` set by `make_analyzer` so the test does not initialize or download a model.

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
& $env:QUESTION_BAKEOFF_PYTHON -m unittest `
  labs/question-region-bakeoff/tests/test_cli_report.py -v
```

Expected: FAIL because the two model-explicit names are not registered.

- [x] **Step 3: Register analyzers without module-global model instances**

```python
def resolve_engine(name: str) -> Analyzer:
    if name == "opencv-whitespace":
        return analyze_image
    if name in {"rapidocr-anchor", "ppocrv6-small-anchor"}:
        return make_analyzer("small")
    if name == "ppocrv6-medium-anchor":
        return make_analyzer("medium")
    raise ValueError(f"unsupported question-region engine: {name}")
```

Return a new analyzer per benchmark run. The closure may cache one runtime during that run but must not persist decrypted images or OCR results.

- [x] **Step 4: Run self-check and all lab tests**

Run:

```powershell
.\scripts\question-bakeoff.ps1 -SelfCheck
& $env:QUESTION_BAKEOFF_PYTHON -m unittest discover `
  -s labs\question-region-bakeoff\tests -v
```

Expected: self-check lists OpenCV plus the two PP-OCRv6 tiers; all tests PASS.

- [x] **Step 5: Commit**

```powershell
git add labs/question-region-bakeoff/question_bakeoff/engine.py `
        labs/question-region-bakeoff/question_bakeoff/cli.py `
        labs/question-region-bakeoff/tests/test_cli_report.py
git commit -m "feat: compare PP-OCRv6 small and medium"
```

### Task 3: Smoke-test both real models and record deployment evidence

**Files:**
- Modify: `labs/question-region-bakeoff/README.md`
- Modify: `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md`

**Interfaces:**
- Consumes: two temporary synthetic images for runtime integrity only; no accuracy conclusions are drawn from them.
- Produces: exact model filenames, byte sizes, SHA-256 hashes, cold-start time, warm inference time, and a corrected real-image protocol.

- [x] **Step 1: Add a local model inspection command**

Document this command without committing model files:

```powershell
Get-ChildItem .tools\question-bakeoff-python\rapidocr\models\*.onnx |
  Get-FileHash -Algorithm SHA256 |
  Select-Object Path, Hash
```

The README must state that models live only in the ignored `.tools` directory and may be removed without affecting the app.

- [x] **Step 2: Run small and medium on the same synthetic two-question page**

Run:

```powershell
.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/fixtures/runtime-smoke/manifest.json `
  --output labs/question-region-bakeoff/output-smoke-small `
  --engine ppocrv6-small-anchor

.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/fixtures/runtime-smoke/manifest.json `
  --output labs/question-region-bakeoff/output-smoke-medium `
  --engine ppocrv6-medium-anchor
```

The executed smoke used a temporary, programmatically generated image with two printed questions
and one red pen circle. It contained no personal data and was deleted automatically. With cached
model files, small recorded 731.1 ms cold / 188.9 ms warm and returned one uncertain full-source
fallback; medium recorded 912.8 ms cold / 411.0 ms warm and returned two regions. This proves only
decoding, model loading, result-shape adaptation, and report generation.

- [x] **Step 3: Correct the real-image comparison protocol**

Run three outputs on the same 60 authorized images:

```powershell
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-opencv `
  --engine opencv-whitespace
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-ppocr-small `
  --engine ppocrv6-small-anchor
.\scripts\question-bakeoff.ps1 run labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-ppocr-medium `
  --engine ppocrv6-medium-anchor
```

Record separate labels for `printed_anchor_miss`, `pen_mark_confusion`, `handwriting_miss`, `formula_cut`, `figure_cut`, `false_option_split`, `column_order`, `cold_start`, `warm_latency`, and `peak_working_set`.

- [x] **Step 4: Write the go/no-go rules**

For question splitting, prefer small unless medium improves question-start recall by at least 2 percentage points without exceeding two-second p95 warm latency. For printed/handwritten content extraction, do not select a default until character/word accuracy is labeled separately; region metrics cannot prove transcription quality. Keep handwritten output editable and untrusted regardless of winner.

- [x] **Step 5: Run full lab verification and commit**

Run:

```powershell
& $env:QUESTION_BAKEOFF_PYTHON -m unittest discover `
  -s labs\question-region-bakeoff\tests -v
git diff --check
```

Expected: all tests PASS and `git diff --check` exits `0`.

```powershell
git add labs/question-region-bakeoff/README.md `
        docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md `
        docs/superpowers/plans/2026-07-23-ppocrv6-question-content-bakeoff.md
git commit -m "docs: define PP-OCRv6 model gate"
```

## Deferred Production Plans

Create separate implementation plans only after the real-image evidence exists:

1. **Conditional capture preprocessing:** blur/exposure/page-quad quality gates, perspective correction, and optional UVDoc without destructive binarization.
2. **Generic visual-block protection:** compare PP-DocLayout-M against OpenCV connected components for formulas, diagrams, tables, and geometry; neither may decide a complete question alone.
3. **Verified content extraction:** PP-OCRv6 medium text blocks, PP-FormulaNet formula blocks, preserved image blocks, editable Markdown, and confidence/model provenance.
4. **Handwriting review:** printed/handwritten block classification, editable OCR draft, low-confidence highlighting, and optional PaddleOCR-VL-1.6 fallback.
5. **Custom question detector:** train an Apache-2.0 PaddleDetection/RT-DETR model only from consented corrected regions after sufficient production-like labels exist.

## Self-Review

- Spec coverage: distinguishes printed questions, pen-mark noise, handwritten notes/answers, formulas, diagrams, and Markdown derivatives.
- Scope boundary: this plan changes only the disposable benchmark and documentation; no app/runtime/database integration occurs before the existing gate.
- Placeholder scan: no `TBD`, `TODO`, or unspecified test step remains.
- Type consistency: both registered engines are created by `make_analyzer`, expose `model_type`, and return the existing `Analysis` contract.
