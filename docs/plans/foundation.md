# Foundation plan

1. Pin Node, pnpm, Rust, Tauri and CI toolchains.
2. Establish strict TypeScript, Rust formatting/lints, test runners and generated DTO checks.
3. Validate SQLCipher, encrypted blob delivery, DOCX images, resumable upload and MSI build.
4. Establish typed AppResult errors and minimal Tauri capabilities.
5. Deliver the Paper & Ink design tokens and component gallery.
6. Keep multi-profile lifecycle operations typed and atomic: create, rename, select, and guarded
   deletion all serialize with LAN capture and preserve at least one profile.

Exit: Vue and Rust tests pass, Tauri opens on Windows, and each risk spike has a recorded decision.
