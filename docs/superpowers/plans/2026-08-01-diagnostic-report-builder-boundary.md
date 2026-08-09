# Diagnostic Report Builder Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate privacy-sensitive diagnostic report construction from destination validation and atomic file delivery without changing the exported schema, warning policy, failure behavior, or public API.

**Architecture:** Keep `diagnostics.rs` as the public context/receipt/error/export owner and the owner of destination validation plus atomic file writing. Add a private `diagnostic_report_builder.rs` child that owns the complete serializable report schema, fixed aggregate queries, compatibility/startup warning projection, and one `pub(super)` build operation; the parent serializes the opaque report and reads its bounded warning count through a narrow method.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher, serde_json, UUID v7, atomic filesystem rename, Vitest source contracts, Rust integration tests

## Global Constraints

- Preserve public types, signatures, error variants/codes, report schema version 3, application metadata, storage-kind values, compatibility/startup projections, warning order/codes, fixed aggregate table/filter list, integrity behavior, filename format, report ID generation, pretty JSON, receipt fields, lock scope, destination validation, create-new temporary file, flush/sync, no-overwrite check, rename, and failed-write cleanup.
- Keep `DiagnosticStorageKind`, `DiagnosticContext`, `DiagnosticExportReceipt`, `DiagnosticError`, `export_diagnostic_report`, `ensure_destination_directory`, and `write_report_atomically` in `diagnostics.rs`.
- Move all private report-schema structs/enums, `REPORT_SCHEMA_VERSION`, `APPLICATION_NAME`, `build_report`, `count`, and `non_negative` into the child. The child must not perform filesystem operations.
- Keep diagnostic output aggregate-only: do not query or serialize account IDs, profile IDs, names, subjects, notes, tags, paths, filenames, device IDs, sync payloads, or conflict values.
- Do not edit commands, bindings, migrations, frontend code, existing Rust tests, startup safety, or Windows compatibility.
- Format only the selected facade and new child. Preserve the dirty worktree; do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/diagnostic-report-builder-boundary.test.ts`
- Test: `src-tauri/tests/diagnostics.rs`
- Test: `src-tauri/src/modules/diagnostics.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private child and facade delegation, report-schema/query/warning ownership, aggregate-only field policy, and continued facade ownership of destination validation, atomic writing, cleanup, and the no-overwrite unit test.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/diagnostic-report-builder-boundary.test.ts`

Expected: FAIL because the report builder child and delegation do not exist.

- [x] **Step 3: Run current diagnostic characterization tests**

Run the `diagnostics` integration target and the focused library no-overwrite test. Expect the three export/privacy cases and one atomic-write case to pass before extraction.

### Task 2: Extract Privacy-Sensitive Report Construction

**Files:**
- Create: `src-tauri/src/modules/diagnostic_report_builder.rs`
- Modify: `src-tauri/src/modules/diagnostics.rs`

- [x] **Step 1: Move report schema and builder without semantic edits**

Move the schema/application/library/integrity/sync/warning types, constants, `build_report`, `count`, and `non_negative` into the child. Make `DiagnosticReport` visible only to its parent, expose `warning_count()` as a bounded `u32`, and preserve every serialized name, query, warning branch, and field order.

- [x] **Step 2: Keep stable export orchestration and atomic writer**

Privately declare the child, replace the inline builder call with `report_builder::build_report`, replace direct warning-vector access with `report.warning_count()`, and leave destination validation, lock duration, JSON serialization, writer, receipt, and test unchanged.

- [x] **Step 3: Format only target Rust files and run focused green tests**

Run direct `rustfmt --edition 2024` for the two target files, then rerun the source contract, diagnostic integration target, and no-overwrite unit test.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent command and database contracts**

Run command-contract and database-schema integration targets to verify stable receipt serialization and schema compatibility.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, followed by the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, TypeScript typechecking, and ESLint serially.

### Task 4: Privacy/Atomicity Review, Hygiene, And Record

- [x] **Step 1: Review semantic identity and privacy boundary**

Compare pre/post code for public surface, error mapping, lock scope, fixed aggregates, absence of user content, schema/warning order, filename/receipt values, serialization, destination validation, no-overwrite behavior, flush/sync/rename sequence, and cleanup. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks and `git diff --check`; confirm the staged index remains empty and only the facade, builder, source contract, and plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check completed steps and append red/green totals, regression commands, line counts, preserved invariants, review verdict, and hygiene results.

## Verification Record

- Red source contract: 2 of 3 assertions passed and 1 failed before extraction, confirming the private builder and facade delegation were absent.
- Baseline characterization: diagnostic integration passed 3/3 and the no-overwrite/temporary-cleanup unit case passed before extraction.
- Focused green: source contract 3/3, diagnostic integration 3/3, and atomic-writer unit test passed after extraction.
- Adjacent regression: command contract 8/8 and database schema 15/15 passed.
- Strict Rust gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited successfully, including 113/113 library tests with 3 environment-dependent OCR probes ignored as designed.
- Frontend gates: Vitest passed 117 files and 666 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Resulting boundaries: `diagnostics.rs` is 171 lines and owns public export/file delivery; `diagnostic_report_builder.rs` is 183 lines and exposes one top-level `pub(super)` build operation plus an opaque warning-count accessor.
- Preserved invariants: schema version and field order, aggregate-only privacy policy, compatibility/startup warning order, database lock scope, report ID/filename/receipt, pretty JSON, error mapping, destination validation, create-new temporary file, flush/sync, no-overwrite check, rename, and failed-write cleanup.
- Review verdict: no Critical or Important findings; production changes preserve behavior while separating privacy projection from filesystem delivery.
- Scope and hygiene: only the facade, new private builder, structural contract, and this plan belong to the batch; target whitespace checks and `git diff --check` pass, and the staged index remains empty.
