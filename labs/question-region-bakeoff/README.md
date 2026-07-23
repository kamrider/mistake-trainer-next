# Question Region Bake-off Lab

This is a disposable, offline evaluation lab—not an application feature. It answers one
question before any automatic splitter or OCR model is shipped: **does the candidate avoid
cutting question content on representative, consented schoolwork?**

The lab never opens Mistake Trainer's database or encrypted assets. It accepts only a separate
fixture directory whose manifest explicitly confirms anonymization and authorization. OpenCV,
NumPy, reports, and copied photos stay outside the Tauri installer.

## 1. Install and verify the isolated runtime

Use Python 3.12. Dependencies are pinned and installed only under the already ignored `.tools/`
directory:

```powershell
$env:QUESTION_BAKEOFF_PYTHON='C:\path\to\python.exe' # omit when py -3.12 works
.\scripts\question-bakeoff.ps1 -InstallDependencies -SelfCheck
```

Expected output contains Python, NumPy, OpenCV, Pillow, `opencv-whitespace`, and engine version `1.0.0`.
This does not modify `package.json`, `Cargo.toml`, the app bundle, or Windows-wide Python.
The lab dependencies remain development-only: OpenCV is Apache-2.0, the Python wheel wrapper is
MIT, NumPy uses BSD-3-Clause, and Pillow uses HPND/MIT-CMU terms. They are not redistributed by
the signed application.

## 2. Prepare 60 consented, anonymized copies

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

## 3. Label ground truth

Copy `fixtures/manifest.example.json` to `data/manifest.json`. For every copied image add:

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

## 4. Run and inspect every overlay

```powershell
.\scripts\question-bakeoff.ps1 run `
  labs/question-region-bakeoff/data/manifest.json `
  --output labs/question-region-bakeoff/output
```

Open `labs/question-region-bakeoff/output/index.html`. Green rectangles are human ground truth;
amber rectangles are suggestions at or above the review threshold; red rectangles are explicitly
uncertain. Blue outlines are page-quadrilateral observations. The source filename and absolute
path are never written into `report.json` or HTML.

The output directory has an ownership marker. The runner replaces only a previous report carrying
that exact marker; it refuses to delete or overwrite an arbitrary directory. A failed run keeps the
last successful report intact.

## 5. Interpret the metrics

- `questionStartRecall`: detected region starts within 3.5% page height of a labeled anchor.
- `contentCutRate`: labeled content area not covered by its matched suggestion. Target `< 0.5%`.
- `falseSplitRate`: unmatched extra suggestions divided by all suggestions. Target `< 3%`.
- `regionRecall`: labeled regions with a unique prediction at IoU `>= 0.5`.
- `uncertainCount`: suggestions below confidence `0.75`; these must remain visibly reviewable.

The 60-image run is only an early bake-off. A production decision requires at least 300 images,
question-start recall `>= 95%`, content-cut rate `< 0.5%`, false split rate `< 3%`, manual review
of every uncertain case, and responsive performance on the 4-core/8 GB Windows reference PC.

Passing synthetic tests does **not** pass either gate. If the real dataset fails, automatic splitting
stays out of the product and the existing non-destructive manual crop remains canonical. RapidOCR
and larger OCR/VL candidates are compared only after this baseline produces a trustworthy report.
