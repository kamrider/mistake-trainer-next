# Installed Product Golden Path Implementation Plan

> **For Codex:** Execute this plan in the current session with review checkpoints. Subagent execution is unavailable in this session, so implement and verify each task locally.

**Goal:** Make the packaged Windows application prove a meaningful offline learning workflow before an installer can pass release smoke testing.

**Architecture:** Add an explicit, non-interactive product-check mode to the installed executable. The mode owns a generated child directory under a caller-provided scratch root, uses the same production SQLCipher, asset, problem, review, and backup modules as the desktop application, writes a fixed-schema sanitized JSON report, and removes only its generated child directory. Extend the existing NSIS smoke script to invoke and validate this report before launching the GUI.

**Tech Stack:** Rust 2024, SQLCipher/rusqlite, image, serde, PowerShell, GitHub Actions

---

### Task 1: Define the sanitized product-check contract

**Files:**
- Create: `src-tauri/src/modules/product_check.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Test: `src-tauri/tests/product_check.rs`

- [x] Write a failing integration test for the successful fixed-schema report.
- [x] Write a failing integration test for an unavailable scratch root and path redaction.
- [x] Implement the report types, fixed failure codes, exclusive child-workspace ownership, and create-new report writing.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --test product_check`.

### Task 2: Exercise the real local learning lifecycle

**Files:**
- Modify: `src-tauri/src/modules/product_check.rs`
- Test: `src-tauri/tests/product_check.rs`

- [x] Initialize a new encrypted local library with an ephemeral in-memory secret store.
- [x] Assert the database does not expose a plaintext SQLite header.
- [x] Create valid question and answer images through the production problem use case and read them back through previews/detail.
- [x] Start a manual review session and submit a real FSRS rating.
- [x] Create and validate an encrypted backup package.
- [x] Reopen the library with the same credentials and verify the problem and review event remain readable.
- [x] Remove only the generated product-check workspace and report cleanup failure through a fixed code.

### Task 3: Expose the installed-binary entry point

**Files:**
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/tests/product_check.rs`

- [x] Accept `--windows-product-check <absolute-report-path> <absolute-scratch-root>`.
- [x] Reject missing or relative paths without starting Tauri.
- [x] Return zero only for a ready product-check report; preserve distinct failure and invocation exit codes.
- [x] Run Rust formatting and the focused product-check test.

### Task 4: Gate the Windows installer smoke

**Files:**
- Modify: `scripts/windows-installer-smoke.ps1`

- [x] Create a dedicated product-check report path and scratch directory under the smoke root.
- [x] Invoke the installed executable in product-check mode.
- [x] Validate schema version, readiness, empty failure codes, and all required lifecycle checks.
- [x] Keep the existing GUI stability, single-instance, startup-failure, and uninstall assertions.

### Task 5: Verify the remediation batch

**Files:**
- Modify: `docs/superpowers/plans/2026-07-28-installed-product-golden-path.md`

- [x] Run the focused Rust product-check integration test.
- [x] Run the existing startup-safety Rust integration test.
- [x] Run all Rust tests that do not require packaging.
- [x] Run `pnpm test:coverage`, `pnpm lint`, `pnpm typecheck`, and `pnpm build`.
- [x] Run the GitHub Actions pin contract and `git diff --check`.
- [x] Review the final diff for security boundaries, cleanup containment, report redaction, and user-change preservation.
