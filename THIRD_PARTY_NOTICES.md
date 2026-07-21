# Third-party notices

## HEIC/HEIF mobile capture

The mobile capture page loads the following decoder only when a user selects a HEIC or HEIF image. Its unmodified vendored source is kept separately under `src-tauri/mobile/vendor/`, is served only by the local collector, and is never fetched from a CDN.

- `heic2any` 0.0.4 — MIT License — <https://github.com/alexcorvi/heic2any>
- Bundled `libheif` / `libheif-js` decoder — GNU Lesser General Public License 3.0 — <https://github.com/catdad-experiments/libheif-js> and <https://github.com/strukturag/libheif>

The complete license texts distributed with this repository are in `third-party-licenses/` and are configured as Windows bundle resources. The vendored source can be inspected or updated independently before rebuilding the application.
