# Third-party notices

## HEIC/HEIF mobile capture

The mobile capture page loads the following decoder only when a user selects a HEIC or HEIF image. Its unmodified vendored source is kept separately under `src-tauri/mobile/vendor/`, is served only by the local collector, and is never fetched from a CDN.

- `heic2any` 0.0.4 — MIT License — <https://github.com/alexcorvi/heic2any>
- Bundled `libheif` / `libheif-js` decoder — GNU Lesser General Public License 3.0 — <https://github.com/catdad-experiments/libheif-js> and <https://github.com/strukturag/libheif>

The complete license texts distributed with this repository are in `third-party-licenses/` and are configured as Windows bundle resources. The vendored source can be inspected or updated independently before rebuilding the application.

## Optional local OCR model downloads

The Windows settings page can, only after a local hardware preflight and explicit user
confirmation, download fixed PP-OCRv6 ONNX model files from the versioned
`RapidAI/RapidOCR` 3.9.2 repository on ModelScope. Model files are not part of the
installer, encrypted library, sync payload, or backup. The x64 installer includes the
hash-pinned Microsoft ONNX Runtime 1.20.1 CPU libraries so inference remains offline
after the optional model is installed.

- PaddleOCR / PP-OCRv6 model lineage — Apache License 2.0 —
  <https://github.com/PaddlePaddle/PaddleOCR>
- RapidOCR model conversion and distribution project — Apache License 2.0 —
  <https://github.com/RapidAI/RapidOCR>
- `ppocr-rs` 0.7.3 local adapter — Apache License 2.0 —
  <https://github.com/dariofinardi/PaddleOCR-OCR-rs>
- Microsoft ONNX Runtime 1.20.1 CPU runtime — MIT License —
  <https://github.com/microsoft/onnxruntime>

The Apache License 2.0 text distributed with the application is in
`third-party-licenses/Apache-2.0.txt`. The complete Microsoft license and third-party
notices are installed next to the other application licenses. OpenCV is shown only as a
future component and is not downloaded or distributed by this version.
