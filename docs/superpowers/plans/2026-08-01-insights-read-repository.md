# Insights Read Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate dashboard, learning-report, and settings overview persistence from the public insights use-case facade without changing any query, projection, time-window, or serialization behavior.

**Architecture:** Keep `insights.rs` as the public DTO/error/API owner. Add one private `insights_read_repository.rs` child that owns all SQL, bounded-count conversion, daily projection, streak calculation, and the three complete read operations; each public facade function delegates directly to the matching `pub(super)` repository operation.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher, serde, specta, Vitest source contracts, Rust integration tests

## Global Constraints

- Preserve all public function signatures, DTO fields, serde casing, Specta types, error variants, command/binding behavior, SQL text, query order, timezone validation, UTC/local day boundaries, 14-day report range, 30-day remembered-rate range, streak semantics, count saturation, subject ordering, and account/profile predicates.
- Keep `DashboardOverview`, `ReportSummary`, `SettingsOverview`, `DailyActivity`, `SubjectActivity`, and `InsightsError` in `insights.rs`.
- Move every SQL statement and private read-model helper out of `insights.rs`; expose exactly three `pub(super)` repository functions and keep the child module private.
- Do not edit commands, bindings, migrations, frontend code, or existing Rust tests.
- Format only the selected Rust facade and new repository. Preserve the dirty worktree; do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Behavior Baseline

**Files:**
- Create: `tests/insights-read-repository-boundary.test.ts`
- Test: `src-tauri/tests/insights_store.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private child declaration, three stable public wrappers, exactly three `pub(super)` repository operations, DTO/error ownership in the facade, SQL/helper ownership in the repository, and account/profile scoping.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/insights-read-repository-boundary.test.ts`

Expected: FAIL because the private repository and delegation do not exist.

- [x] **Step 3: Run current insight characterization tests**

Run: `.\scripts\cargo-msvc.cmd test --quiet --manifest-path src-tauri\Cargo.toml --test insights_store`

Expected: both existing dashboard/report/settings integration cases pass before extraction.

### Task 2: Extract The Read Repository

**Files:**
- Create: `src-tauri/src/modules/insights_read_repository.rs`
- Modify: `src-tauri/src/modules/insights.rs`

- [x] **Step 1: Move the three read operations without semantic edits**

Move `DAY_MS`, `REPORT_DAYS`, `dashboard_overview`, `report_summary`, `settings_overview`, `scalar`, both streak helpers, and `bounded_i32` into the child. Import facade-owned DTOs and `InsightsError`; preserve every statement, branch, cast, and ordering expression.

- [x] **Step 2: Keep stable public facade wrappers**

Declare the child with `#[path = "insights_read_repository.rs"] mod read_repository;` and retain the three public signatures as direct calls to `read_repository::{dashboard_overview, report_summary, settings_overview}`.

- [x] **Step 3: Format only target Rust files and run focused green tests**

Run direct `rustfmt --edition 2024` for the two target Rust files, then rerun the source contract and `insights_store`; expect 3 structural assertions and 2 integration cases to pass.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent database and command-contract suites**

Run database-schema and command/bindings contract integrations to verify schema compatibility and unchanged public command registration.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, followed by the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, TypeScript typechecking, and ESLint serially.

### Task 4: Review, Hygiene, And Verification Record

- [x] **Step 1: Review semantic identity**

Compare the extraction against the pre-change facade for signatures, DTO/error ownership, timezone bounds, date calculations, SQL text/order, tenant predicates, subject grouping, streak rules, and numeric saturation. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks and `git diff --check`; confirm the staged index remains empty and only the facade, repository, source contract, and plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check completed steps and append red/green totals, regression commands, line counts, invariant review, and scope/hygiene results.

## Verification Record

- Red source contract: 2 of 3 assertions passed and 1 failed before extraction, confirming the private repository and facade delegation were absent.
- Baseline characterization: both `insights_store` integration cases passed before extraction.
- Focused green: source contract 3/3 and `insights_store` 2/2 passed after extraction.
- Adjacent regression: command contract 8/8 and database schema 15/15 passed.
- Strict Rust gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited successfully, including 113/113 library tests with 3 environment-dependent OCR probes ignored as designed.
- Frontend gates: Vitest passed 116 files and 663 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Resulting boundaries: `insights.rs` is 103 lines and owns public DTOs/errors/API; `insights_read_repository.rs` is 312 lines and exposes exactly three `pub(super)` read operations.
- Preserved invariants: public signatures and serialization, timezone validation, UTC/local day boundaries, 14-day report, 30-day remembered rate, streak rules, tenant predicates, SQL/order expressions, subject projection, numeric saturation, and failure mapping.
- Review verdict: no Critical or Important findings; production changes are a semantic-equivalent ownership move.
- Scope and hygiene: only the facade, new private repository, structural contract, and this plan belong to the batch; target whitespace checks and `git diff --check` pass, and the staged index remains empty.
