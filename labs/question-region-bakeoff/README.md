# Question Region Bake-off Lab

This is a disposable, offline evaluation lab—not an application feature. It answers one
question before any automatic splitter or OCR model is shipped: **does the candidate avoid
cutting question content on representative, consented schoolwork?**

The lab never opens Mistake Trainer's database or encrypted assets. It accepts only a separate
fixture directory whose manifest explicitly confirms anonymization and authorization. OpenCV,
NumPy, reports, and copied photos stay outside the Tauri installer.

## 1. Prepare and label images without installing the OCR runtime

The visual annotator uses only Python's standard library. After placing authorized copies under
`data/images/`, start it with:

```powershell
.\scripts\question-bakeoff.ps1 annotate labs/question-region-bakeoff/data
```

It opens a browser page served only on `127.0.0.1` with a one-time random token. The page makes no
third-party requests and never changes image files. Drag to mark complete question/answer regions,
switch to **标记题号** to click visible question-number anchors, and use `N`/`P` to move between
images. Progress auto-saves to the ignored `annotations.draft.json`; an evaluable `manifest.json`
is written only after every image has a region and both anonymization and authorization boxes are
checked. Closing and reopening the tool resumes the draft.

Use `--no-open` when copying the printed local URL into a different browser:

```powershell
.\scripts\question-bakeoff.ps1 annotate `
  labs/question-region-bakeoff/data --no-open
```

## 2. Install and verify the isolated runtime

Use Python 3.12. Dependencies are pinned and installed only under the already ignored `.tools/`
directory:

```powershell
$env:QUESTION_BAKEOFF_PYTHON='C:\path\to\python.exe' # omit when py -3.12 works
.\scripts\question-bakeoff.ps1 -InstallDependencies -SelfCheck
```

Expected output contains Python, NumPy, OpenCV, Pillow, RapidOCR `3.9.2`, ONNX Runtime
`1.27.0`, and `opencv-whitespace`, `ppocrv6-small-anchor`, and
`ppocrv6-medium-anchor`. RapidOCR is the isolated runtime host; the actual OCR
models compared here are PP-OCRv6 small and medium.
This does not modify `package.json`, `Cargo.toml`, the app bundle, or Windows-wide Python.
The lab dependencies remain development-only: OpenCV is Apache-2.0, the Python wheel wrapper is
MIT, NumPy uses BSD-3-Clause, and Pillow uses HPND/MIT-CMU terms. They are not redistributed by
the signed application.

## 3. Prepare 60 consented, anonymized copies

Create this ignored tree; copy source photos into it—never link the original directory:

```text
labs/question-region-bakeoff/data/
├── manifest.json
└── images/
    ├── sample-0001.png
    └── ...
```

Before copying each image:

1. Obtain permission to use it for local evaluation.
2. Remove names, school/class identifiers, faces, QR codes, and other visible personal data.
3. Decode and re-save the approved copy as PNG or JPEG so EXIF/GPS metadata is removed. OpenCV's
   `imdecode` + `imencode` path used by the lab does not copy metadata, but visual identifiers
   still require human inspection.
4. Use neutral sequential filenames; keep the originals outside this repository.
5. Delete the ignored `data/` directory when the evaluation is no longer needed.

The first 60 images should cover at least: single questions, adjacent questions, two columns,
skew/perspective, shadows, formulas, geometry, chemistry, handwriting over print, question-only
pages, and answer pages. Synthetic images and repeated variants do not count toward 60.

## 4. Label ground truth

Prefer the visual annotator above. If editing JSON manually, copy
`fixtures/manifest.example.json` to `data/manifest.json`. For every copied image add:

- a safe stable `id`;
- `layout` and subject/layout `tags`;
- one normalized rectangle per complete question or answer region;
- the normalized top-left question-number anchor where one is visible.

Coordinates are fractions of the source copy: `x = left / imageWidth`, `y = top / imageHeight`,
`width = cropWidth / imageWidth`, and `height = cropHeight / imageHeight`. Rectangles must stay
inside `[0, 1]`. A ground-truth region must include formulae, figures, tables, units, labels, and
all answer work that belongs to it; a visually plausible crop that loses one exponent or diagram
label is wrong.

Validate before running:

```powershell
.\scripts\question-bakeoff.ps1 validate labs/question-region-bakeoff/data/manifest.json
```

Validation rejects missing consent, absolute/escaping paths, duplicate IDs, missing images,
non-finite coordinates, and out-of-bounds rectangles.

## 5. Run all three engines and inspect every overlay

```powershell
.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-opencv `
  --engine opencv-whitespace

.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-ppocr-small `
  --engine ppocrv6-small-anchor

.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output-ppocr-medium `
  --engine ppocrv6-medium-anchor
```

Open both `index.html` reports side by side. Green rectangles are human ground truth; amber
rectangles are suggestions at or above the review threshold; red rectangles are explicitly
uncertain. Blue outlines are page-quadrilateral observations. The source filename and absolute
path are never written into `report.json` or HTML.

Inspect every overlay and record one or more failure labels per image:

- `cut_formula`
- `cut_figure`
- `merged_questions`
- `false_option_split`
- `column_order`
- `handwriting_confusion`
- `pen_mark_confusion`
- `printed_anchor_miss`
- `no_anchor`
- `cold_start`
- `warm_latency`
- `peak_working_set`

A crop that loses an exponent, unit, charge, graph label, diagram edge, or answer-work stroke is a
content-cut failure even when the remaining crop looks plausible.

The output directory has an ownership marker. The runner replaces only a previous report carrying
that exact marker; it refuses to delete or overwrite an arbitrary directory. A failed run keeps the
last successful report intact.

## 6. Interpret the metrics

- `questionStartRecall`: detected region starts within 3.5% page height of a labeled anchor.
- `contentCutRate`: labeled content area not covered by its matched suggestion. Target `< 0.5%`.
- `falseSplitRate`: unmatched extra suggestions divided by all suggestions. Target `< 3%`.
- `regionRecall`: labeled regions with a unique prediction at IoU `>= 0.5`.
- `uncertainCount`: suggestions below confidence `0.75`; these must remain visibly reviewable.

The 60-image run is only an early bake-off. A production decision requires at least 300 images,
question-start recall `>= 95%`, content-cut rate `< 0.5%`, false split rate `< 3%`, manual review
of every uncertain case, and responsive performance on the 4-core/8 GB Windows reference PC.

Passing synthetic tests does **not** pass either gate. Continue from 60 to 300 images only when
at least one PP-OCRv6 tier materially improves question-start recall over `opencv-whitespace`,
does not regress content-cut rate, and keeps p95 warm one-image latency below two seconds on the
reference PC. Prefer small for question splitting unless medium improves question-start recall by
at least two percentage points. Transcription needs separately labeled character/word accuracy;
region metrics cannot prove printed or handwritten OCR quality.

If the real dataset fails, automatic splitting stays out of the product and the existing
non-destructive manual crop remains canonical. Unlimited-OCR and PaddleOCR-VL remain optional
heavyweight comparison engines; they are not bundled into the Windows v1 installer.

## 7. Local runtime evidence

The ignored `.tools/question-bakeoff-python/rapidocr/models` directory currently contains these
RapidOCR 3.9.2 PP-OCRv6 ONNX files:

| Model | Bytes | SHA-256 |
| --- | ---: | --- |
| `PP-OCRv6_det_small.onnx` | 9,929,594 | `090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f` |
| `PP-OCRv6_rec_small.onnx` | 21,234,383 | `6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884` |
| `PP-OCRv6_det_medium.onnx` | 62,119,454 | `92078b7355007ccfffcd4c8cd441a3afd4538904d06881b29a155e1e679907c2` |
| `PP-OCRv6_rec_medium.onnx` | 76,629,984 | `eef444829dbbe18d7fea59a3f6eb75647518d2b3a9568d27c92e42940204894b` |

On the development PC, one synthetic two-question image with a red pen circle produced:

| Engine | Cached cold start | Warm run | Suggested regions |
| --- | ---: | ---: | ---: |
| `ppocrv6-small-anchor` | 731.1 ms | 188.9 ms | 1 uncertain full-source fallback |
| `ppocrv6-medium-anchor` | 912.8 ms | 411.0 ms | 2 |

This is only a runtime-integrity observation. One synthetic image is not accuracy evidence and
must not be used to select a production model. Model files are not committed and may be removed
without affecting the application.
