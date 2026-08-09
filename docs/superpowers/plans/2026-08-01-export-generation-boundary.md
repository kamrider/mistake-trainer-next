# Export Generation Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate export snapshot persistence from filesystem/DOCX generation so the database repository and document renderer have explicit, independently guarded responsibilities while every existing `modules::exports` API and generated artifact remains unchanged.

**Architecture:** Keep `exports.rs` as the public facade and snapshot repository for create/list/candidate/delete/restore operations. Move preparation queries, bounded asset loading, decryption, destination validation, atomic folder/DOCX writing, image validation, and output naming into a private `exports_generation.rs` child module; re-export `generate_export` and expose the crate-visible preparation flow through an opaque facade-owned `PreparedExport` wrapper and same-signature delegates.

**Tech Stack:** Rust 2024, rusqlite, docx-rs, image, Vitest source contracts, existing Rust export integration tests

## Global Constraints

- Do not change any public or crate-visible function signature, type name, enum serialization, error variant, user-visible output name, generated folder/file layout, DOCX paragraph ordering, image scaling, size limit, SQL text/predicate/order, path validation, atomic temporary-file cleanup, or error mapping.
- Do not modify `src-tauri/src/modules/mod.rs` or `src-tauri/src/commands/exports.rs`; declare the generator with `#[path = "exports_generation.rs"] mod generation;` inside `exports.rs`.
- Keep `exports::generate_export`, `exports::prepare_export`, `exports::write_prepared_export`, and `exports::PreparedExport` available at their current visibility; the latter three use facade delegates so internal generator visibility does not leak.
- Keep snapshot mutation/query functions and `candidate_from_row` in `exports.rs`; they must not depend on docx-rs, image decoding, filesystem handles, or asset decryption.
- Preserve the dirty worktree; do not stage or commit, and do not modify any pre-existing dirty Rust file.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/export-generation-boundary.test.ts`
- Test: `src-tauri/tests/export_store.rs`
- Test: `src-tauri/tests/export_generation.rs`

**Interfaces:**
- Consumes: `src-tauri/src/modules/exports.rs` and `src-tauri/src/modules/exports_generation.rs` as source text.
- Produces: an architecture contract proving persistence stays in the facade and file/DOCX generation stays in the private child.

- [x] **Step 1: Add the failing source contract**

Assert the facade privately declares `exports_generation.rs`, re-exports the existing four generation interfaces, contains snapshot repository functions, and no longer imports docx-rs, filesystem file handles, image cursor/read/write helpers, or `decrypt_asset`. Assert the child file contains `generate_export`, `prepare_export`, `write_prepared_export`, `load_snapshot`, `load_export_problems`, path/decryption helpers, folder/DOCX renderers, and all generation safety constants; assert snapshot create/list/delete/restore functions are absent from the child.

- [x] **Step 2: Run the structure test and verify red**

Run: `npm test -- --run tests/export-generation-boundary.test.ts`

Expected: FAIL because the private generation module and facade re-exports do not exist.

- [x] **Step 3: Run both existing export characterization suites**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_store --test export_generation`

Expected: snapshot persistence and generated folder/DOCX behavior pass before extraction.

### Task 2: Extract The Filesystem And DOCX Generator

**Files:**
- Create: `src-tauri/src/modules/exports_generation.rs`
- Modify: `src-tauri/src/modules/exports.rs`

**Interfaces:**
- Produces: `pub fn generate_export(...) -> Result<GeneratedExportSummary, ExportError>`.
- Produces internally: `pub(super) fn prepare_export(...) -> Result<generation::PreparedExport, ExportError>` and `pub(super) fn write_prepared_export(...)`.
- Produces through the facade: `pub(crate) struct PreparedExport(generation::PreparedExport)` plus same-signature `pub(crate)` preparation/write delegates.
- Consumes: parent types `ExportError`, `ExportLayout`, and `GeneratedExportSummary`.

- [x] **Step 1: Create the private generator child**

Move `StoredConfiguration`, `StoredSnapshot`, `ExportAsset`, `ExportProblem`, the internal prepared state, the six generation limits, `generate_export`, preparation/write operations, and every helper from `load_snapshot` through `safe_output_stem` into `exports_generation.rs`. Import docx-rs, image support, filesystem and path types, rusqlite, UUID, `decrypt_asset`, and the three parent types there. Keep helpers private, expose internal prepared state and preparation/write only to the parent with `pub(super)`, and keep `generate_export` public for facade re-export.

- [x] **Step 2: Reduce the facade to snapshot persistence and delegates**

Add:

```rust
#[path = "exports_generation.rs"]
mod generation;

pub use generation::generate_export;

pub(crate) struct PreparedExport(generation::PreparedExport);
```

Add same-signature `pub(crate)` delegates that wrap the child preparation result and unwrap it for writing. Remove all moved definitions and generator-only imports/constants from `exports.rs`. Keep `TRASH_RETENTION_MS`, DTOs, errors, snapshot SQL, candidate SQL, and mutation transaction logic unchanged.

- [x] **Step 3: Format only the two target Rust files**

Run: `rustfmt --edition 2024 src-tauri/src/modules/exports.rs src-tauri/src/modules/exports_generation.rs`

Expected: only the target facade and child receive formatting writes.

- [x] **Step 4: Run focused structural and behavior tests**

Run: `npm test -- --run tests/export-generation-boundary.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_store --test export_generation`

Expected: the contract, snapshot repository behavior, folder output, DOCX output, safety limits, and destination validation all pass.

### Task 3: Regression And Static Verification

**Files:**
- Modify only Task 1 or Task 2 files if verification reveals a regression.

**Interfaces:**
- Consumes: complete Rust crate plus frontend architecture contracts.
- Produces: compile, lint, behavior, type, and boundary evidence.

- [x] **Step 1: Run command-contract compatibility**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract`

Expected: command error contracts remain stable and path-free.

- [x] **Step 2: Run Rust lint and complete tests**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml`

Expected: all targets compile without warnings and every non-ignored Rust test passes.

- [x] **Step 3: Run frontend contract and full quality gates**

Run: `npm test -- --run tests/export-generation-boundary.test.ts`

Run: `npm test -- --run`

Run: `npm run typecheck`

Run: `npm run lint`

Expected: architecture/full tests, Vue type checking, and zero-warning lint all pass.

### Task 4: Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-export-generation-boundary.md`

**Interfaces:**
- Consumes: target diffs, test/lint output, file timestamps, status, and final review.
- Produces: checked steps and exact evidence for this batch.

- [x] **Step 1: Perform final code review**

Review facade/repository responsibility, re-export visibility, opaque prepared-state ownership, SQL identity, database-lock release compatibility, path containment, size budgets, image validation, atomic cleanup, output naming, DOCX ordering, error propagation, source-contract robustness, and overlap with user changes. Fix every Critical or Important finding.

- [x] **Step 2: Verify patch hygiene and scope**

Run: `git diff --check -- src-tauri/src/modules/exports.rs src-tauri/src/modules/exports_generation.rs tests/export-generation-boundary.test.ts docs/superpowers/plans/2026-08-01-export-generation-boundary.md`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm timestamps show no collateral Rust formatting writes and this task changed only the previously clean `exports.rs` plus its new child, contract, and plan.

- [x] **Step 3: Record evidence without committing**

Check every completed step and append red/green totals, export suite totals, Rust lint/full evidence, frontend totals, file/line reduction, API/SQL/artifact invariants, review verdict, hygiene, index, and exact scope. Do not stage or commit.

## Verification Evidence

- Red contract: `npm test -- --run tests/export-generation-boundary.test.ts` failed as expected with 1 file / 2 tests failing because the private generator and facade boundary did not exist.
- Characterization baseline: `export_generation` passed 1/1 and `export_store` passed 2/2 before extraction.
- Focused green: the architecture contract passed 1 file / 2 tests; after the final opaque-wrapper adjustment, `export_generation` passed 1/1 and `export_store` passed 2/2 with no task-code warning.
- Command compatibility: `command_contract` passed 8/8, including stable path-free public error shapes.
- Rust static/full verification: Clippy passed all targets with `-D warnings`; full `cargo test` exited 0. The library target reported 110 passed / 3 ignored, and current enumeration found 382 tests / 0 benchmarks across all targets.
- Frontend verification: full Vitest passed 108/108 files and 642/642 tests. `vue-tsc --noEmit` and ESLint with `--max-warnings 0` also exited 0.
- Size: `exports.rs` fell from 939 to 460 lines. The private generator is 519 lines; combined code is 979 lines, with the increase limited to explicit imports, module visibility, and the facade-owned opaque preparation wrapper/delegates.
- API and lock invariant: `exports::generate_export` remains public, while `exports::PreparedExport`, `exports::prepare_export`, and `exports::write_prepared_export` retain crate visibility and signatures. `commands/exports.rs` was untouched, so preparation still occurs while the connection lock is held and filesystem writing still occurs only after that guard is dropped.
- SQL/artifact invariant: snapshot and generation SQL text, predicates, ordering, six memory/image safety limits, path containment, decryption, byte-length checks, atomic temporary cleanup, safe output stems, image scaling, folder names, DOCX paragraph ordering, and error propagation were moved without semantic changes.
- Review: the first re-export design produced an unused-import warning for the opaque type. It was replaced with a facade-owned opaque wrapper and same-signature delegates; the final review found no remaining Critical or Important issue.
- Hygiene and scope: only `exports.rs` and `exports_generation.rs` had Rust write timestamps during this batch. Targeted whitespace/index checks are recorded after the final plan update; no pre-existing dirty Rust file was modified.
- Index: nothing was staged or committed. Existing unrelated modified and untracked files were preserved.
