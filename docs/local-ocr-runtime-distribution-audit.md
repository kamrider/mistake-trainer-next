# Local OCR runtime distribution audit

Date: 2026-07-29

## Decision

The fixed Windows x64 CPU runtime is approved for the review-first local question-number anchor
path. The NSIS installer includes the exact Microsoft ONNX Runtime 1.20.1 CPU libraries and their
notices. PP-OCRv6 model files remain an explicit optional install and are never part of the
encrypted library, sync payload, or backup.

This approval does not cover semantic question understanding, OCR text persistence, automatic
answer judgment, solving, or silent formal-library insertion. Those capabilities remain closed.

The proposed Windows x64 baseline is the official Microsoft CPU package
`Microsoft.ML.OnnxRuntime 1.20.1`. This matches the ONNX Runtime 1.20 API targeted by
`ort 2.0.0-rc.9`; it replaces the unrelated 1.27 development DLL that happened to be available in
the Python bake-off environment.

Primary references:

- Microsoft ONNX Runtime installation matrix:
  <https://onnxruntime.ai/docs/install/>
- Microsoft ONNX Runtime 1.20.1 release:
  <https://github.com/microsoft/onnxruntime/releases/tag/v1.20.1>
- `ort 2.0.0-rc.9` release:
  <https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.9>
- `ppocr-rs 0.7.3` source repository:
  <https://github.com/dariofinardi/PaddleOCR-OCR-rs>

## Reproducible Microsoft CPU package

The package was downloaded only for audit from:

`https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.20.1/microsoft.ml.onnxruntime.1.20.1.nupkg`

Its NuSpec identifies Microsoft as author and owner, version `1.20.1`, repository commit
`5c1b7ccbff7e5141c1da7a9d963d660e5741c319`, and an MIT license file.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| complete NuGet package | 108,611,147 | `9C32108E9A1C78B090A95C2A219F244DD5F067D4CAF68FB0F967103F3F20839C` |
| `runtimes/win-x64/native/onnxruntime.dll` | 11,569,696 | `4CB41E89B8BF30578E1DD95E9C40292D61974A4BFCD666409302C4F0C5AA8CE0` |
| `runtimes/win-x64/native/onnxruntime_providers_shared.dll` | 22,048 | `3B164F0019863266971843BACA9551F015DB7E502E2D9C4575F74C6A2BBBB78A` |
| `LICENSE` | 1,094 | `C250D6278F0B47A6439FB7592B08B58A55EB9F535AA49A1DB63211C3F982B674` |
| `ThirdPartyNotices.txt` | 345,046 | `FAC5BB85F568B38FD8F3A6AEA32737985841F23A1A43E9CE952F79474F883AA9` |

The two DLLs, Microsoft license, and complete third-party notices are application resources and
are mapped into the Windows x64 bundle by `tauri.conf.json`. Rust verifies both DLL lengths and
hashes again before dynamic loading. The original NuGet package is audit evidence only and is not
distributed.

## Rust dependency review

- `ort 2.0.0-rc.9` and `ort-sys 2.0.0-rc.9` are declared
  `MIT OR Apache-2.0`. The root crate enables only `load-dynamic`; neither
  `download-binaries` nor `copy-dylibs` is enabled.
- `ppocr-rs 0.7.3` is declared `Apache-2.0` and is described by its maintainer as Beta.
  It has no default model-download feature, so the product adapter performs no inference-time
  network request.
- `ppocr-rs` unconditionally enables the `ort` feature markers for CUDA, DirectML, CoreML, and
  XNNPACK even though the proposed product path is CPU-only. Those markers do not download provider
  binaries in the current graph, but they unnecessarily widen the compile-time API and review
  surface.
- The published `ppocr-rs 0.7.3` crate archive declares Apache-2.0 but omits the license text.
  Distribution compensates by installing the full Apache-2.0 text and recording ppocr-rs,
  PaddleOCR model lineage, and RapidOCR conversion/distribution attribution in
  `THIRD_PARTY_NOTICES.md`.
- The adapter is exact-version pinned, has default features disabled, and contains no model
  downloader. Model installation is an application-owned command with fixed versioned HTTPS
  sources, byte lengths, SHA-256 values, staging, and atomic promotion.
- `ppocr-rs` still exposes unnecessary execution-provider feature markers transitively. The
  shipped runtime contains only Microsoft CPU provider files; replacing the adapter with a
  reviewed CPU-only fork remains a hardening item, not a blocker for the bounded local anchor
  feature.

## Model and runtime integrity

The small model candidate remains separately installed and hash-pinned:

| Model | Bytes | SHA-256 |
| --- | ---: | --- |
| `PP-OCRv6_det_small.onnx` | 9,929,594 | `090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f` |
| `PP-OCRv6_rec_small.onnx` | 21,234,383 | `6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884` |

The Rust adapter independently verifies both model hashes and lengths, the exact Microsoft 1.20.1
x64 runtime DLL, and its providers shared library before dynamic loading. It extracts the
recognition dictionary from bounded ONNX metadata, disables ONNX Runtime telemetry, processes one
image at a time, and deletes the temporary dictionary.

The ignored real-runtime Rust corpus test loaded the hash-pinned Microsoft 1.20.1 CPU DLL and the
two hash-pinned small models, then processed six local real-exam images offline. The proposed
region counts were `10, 4, 1, 3, 4, 12`: all 33 manually reviewed questions on the five readable
pages were separated, while the severely degraded four-question page safely remained one
low-confidence full-page region. A second ignored integration test ran the 12-question
double-column page through the encrypted capture worker, atomic crop application, restart
recovery, and revert path.

A third ignored end-to-end test used a rendered 2025 mathematics exam page and its answer page.
The worker produced eight question crops and four answer crops, persisted four same-number pair
suggestions, and atomically created four ready capture drafts; the four questions without answers
remained unassigned. It created no formal Problem or sync outbox entry. All three tests passed.
This is useful engineering evidence, but the small development corpus cannot satisfy the
60/300-image accuracy, crop-safety, or performance gates.

## Remaining expansion blockers

- Replace or upstream a CPU-only ppocr-rs feature surface when a reviewed option is available.
- Collect the authorized 60-image development and 300-image blind evidence sets.
- Pass the documented recall, crop, split-error, warm p95, network-silence, Windows 150% DPI,
  Narrator, and 150-image responsiveness gates.
- Keep semantic OCR, automatic answer judgment, and silent formal-library insertion unavailable
  until separately specified, threat-modeled, and evidenced.
