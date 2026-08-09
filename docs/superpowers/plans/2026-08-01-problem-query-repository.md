# Problem Query Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate read-only problem listing/detail/preview reconstruction from problem mutation transactions so the library query repository is explicit while all existing problem APIs, result shapes, search behavior, and preview safety remain unchanged.

**Architecture:** Keep `problems.rs` as the public facade, DTO/error owner, and mutation service for create/update/status changes. Move summary/detail SQL, search escaping, bounded encrypted asset reads, preview decoding/downscaling, data-URL construction, and query-only safety limits into a private `problem_query_repository.rs` child; the facade exposes same-signature wrappers that delegate to three `pub(super)` repository operations.

**Tech Stack:** Rust 2024, rusqlite, base64, image, Vitest source contracts, existing Rust problem integration tests

## Global Constraints

- Do not change any public function signature, struct/enum field, serde/specta shape, error variant, search escaping rule, SQL text/predicate/group/order, account/profile boundary, asset ordering, preview dimensions, data-URL media type, size limit, decryption behavior, or best-effort list-preview fallback.
- Do not modify `src-tauri/src/modules/mod.rs`, commands, `product_check.rs`, or existing Rust tests; declare `#[path = "problem_query_repository.rs"] mod query_repository;` inside `problems.rs`.
- Keep `list_problem_summaries`, `list_problem_summaries_with_previews`, and `get_problem_detail` at their current public paths and signatures as facade wrappers.
- Keep `create_problem`, `update_problem`, `change_problem_status`, persistence helpers, encryption, cleanup, and mutation constants in `problems.rs`.
- The child may expose only the three facade-called operations as `pub(super)`; query row types, decrypt/read helpers, preview conversion, and media mapping remain private.
- Preserve the dirty worktree; do not stage or commit, and do not modify any pre-existing dirty Rust file.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/problem-query-repository-boundary.test.ts`
- Test: `src-tauri/tests/problem_query.rs`
- Test: `src-tauri/tests/problem_detail.rs`

**Interfaces:**
- Consumes: `src-tauri/src/modules/problems.rs` and `src-tauri/src/modules/problem_query_repository.rs` as source text.
- Produces: an architecture contract proving query/preview dependencies stay outside the mutation facade.

- [x] **Step 1: Add the failing source contract**

Assert `problems.rs` privately declares the child, retains same-signature public wrappers, and delegates to `query_repository`. Assert the facade no longer imports base64, `Cursor`, `Read`, `Component`, or `decrypt_asset`, and no longer defines query-only constants/helpers. Assert the child contains the three repository operations, both read SQL flows, search escaping, bounded decrypt/read, preview resize/data-URL helpers, and five query safety constants; assert create/update/status/persist/cleanup mutation functions are absent.

- [x] **Step 2: Run the structure test and verify red**

Run: `npm test -- --run tests/problem-query-repository-boundary.test.ts`

Expected: FAIL because the private query repository and facade delegation do not exist.

- [x] **Step 3: Run existing query/detail characterization suites**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test problem_detail`

Expected: profile/status/search/preview listing and detail/path-traversal behavior pass before extraction.

### Task 2: Extract The Read-Only Query Repository

**Files:**
- Create: `src-tauri/src/modules/problem_query_repository.rs`
- Modify: `src-tauri/src/modules/problems.rs`

**Interfaces:**
- Produces: `pub(super) fn list_problem_summaries(&Connection, ProblemListQuery) -> Result<Vec<ProblemSummary>, ProblemUseCaseError>`.
- Produces: `pub(super) fn list_problem_summaries_with_previews(&Connection, &Path, &[u8; 32], ProblemListQuery) -> Result<Vec<ProblemSummary>, ProblemUseCaseError>`.
- Produces: `pub(super) fn get_problem_detail(&Connection, &Path, &[u8; 32], ProblemDetailQuery) -> Result<ProblemDetail, ProblemUseCaseError>`.
- Consumes: parent query/response DTOs, `ProblemStatusFilter::as_str`, and `ProblemUseCaseError`.

- [x] **Step 1: Create the private query repository**

Move `list_problem_summaries_internal`, the current `get_problem_detail` body, `read_decrypted_asset`, `make_preview`, `make_preview_with_dimension`, `media_type_for`, and the five query-only safety constants into `problem_query_repository.rs`. Add three `pub(super)` entry functions: the two list variants call the shared private internal function with/without preview storage, and detail retains its exact SQL/decryption flow. Import base64, image cursor/read/path types, rusqlite, `decrypt_asset`, `MAX_CAPTURE_FILE_BYTES`, and parent DTO/error types.

- [x] **Step 2: Reduce the facade to wrappers and mutations**

Add:

```rust
#[path = "problem_query_repository.rs"]
mod query_repository;
```

Keep the three public functions in `problems.rs`, replacing their bodies with direct calls to the matching child operations. Remove query-only imports, constants, and helper definitions. Keep mutation code and its imports/constants unchanged.

- [x] **Step 3: Format only the two target Rust files**

Run: `rustfmt --edition 2024 src-tauri/src/modules/problems.rs src-tauri/src/modules/problem_query_repository.rs`

Expected: only the selected facade and new child receive Rust formatting writes.

- [x] **Step 4: Run focused structure and behavior tests**

Run: `npm test -- --run tests/problem-query-repository-boundary.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test problem_detail`

Expected: architecture contract plus list/search/preview/detail/path-boundary suites pass unchanged.

### Task 3: Mutation Compatibility And Full Regression

**Files:**
- Modify only Task 1 or Task 2 files if verification reveals a regression.

**Interfaces:**
- Consumes: problem lifecycle/runtime/product checks, complete Rust crate, and frontend architecture suite.
- Produces: mutation, product, lint, type, and full regression evidence.

- [x] **Step 1: Run adjacent problem mutation and product suites**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_store --test problem_lifecycle --test runtime_state --test product_check`

Expected: create/update/status/asset persistence and installed-product checks remain compatible with the facade.

- [x] **Step 2: Run Rust lint and complete tests**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml`

Expected: all targets compile without warnings and every non-ignored Rust test passes.

- [x] **Step 3: Run frontend contract and complete quality gates**

Run: `npm test -- --run tests/problem-query-repository-boundary.test.ts`

Run: `npm test -- --run`

Run: `npm run typecheck`

Run: `npm run lint`

Expected: source/full tests, Vue type checking, and zero-warning lint all pass.

### Task 4: Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-problem-query-repository.md`

**Interfaces:**
- Consumes: target diffs, tests/lints, file timestamps, status, and final review.
- Produces: checked plan and exact verification evidence.

- [x] **Step 1: Perform final code review**

Review facade/query ownership, visibility, public signature identity, SQL/search identity, account/profile enforcement, list-preview best-effort behavior, detail fail-closed behavior, path traversal rejection, byte and pixel limits, media/data-URL identity, mutation isolation, source-contract robustness, and overlap with existing changes. Fix every Critical or Important finding.

- [x] **Step 2: Verify patch hygiene and scope**

Run: `git diff --check -- src-tauri/src/modules/problems.rs src-tauri/src/modules/problem_query_repository.rs tests/problem-query-repository-boundary.test.ts docs/superpowers/plans/2026-08-01-problem-query-repository.md`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm only the previously clean `problems.rs` and new child were Rust-formatted, with no collateral writes.

- [x] **Step 3: Record evidence without committing**

Check every completed step and append red/green totals, problem suite totals, Rust lint/full evidence, frontend totals, file/line reduction, API/SQL/preview invariants, review verdict, hygiene, index, and exact scope. Do not stage or commit.

## Verification Record

- Red phase: the new source contract failed 2/2 assertions before extraction because the private repository and delegation did not exist.
- Characterization baseline: `problem_detail` passed 2/2 and `problem_query` passed 4/4 before extraction.
- Focused green phase: the source contract passed 2/2; `problem_detail` passed 2/2 and `problem_query` passed 4/4 after extraction.
- Adjacent compatibility: `problem_lifecycle` passed 5/5, `problem_store` 3/3, `product_check` 4/4, and `runtime_state` 5/5 (17/17 total).
- Rust quality gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited 0. Three environment-dependent OCR model/corpus probes remained explicitly ignored.
- Frontend quality gates: Vitest passed 109/109 files and 644/644 tests; `vue-tsc --noEmit` and ESLint with zero warnings both passed.
- File shape: `problems.rs` reduced from 860 to 598 lines; the private repository is 300 lines. The public facade now owns DTOs/errors and mutations, while query SQL, bounded decrypt/read, search escaping, preview decoding/downscaling, and data-URL construction have one private owner.
- Invariants reviewed: all three public signatures and response/error types are unchanged; list/detail SQL, account/profile filters, wildcard escaping, asset ordering, preview limits, byte/pixel caps, media mapping, list best-effort fallback, detail fail-closed behavior, and path traversal rejection are preserved.
- Review verdict: no Critical or Important findings. The source contract checks private visibility, facade delegation, query SQL/dependencies/limits, preview construction, and mutation isolation.
- Scope and index: only the previously clean `problems.rs` plus the new repository, source contract, and this plan belong to this batch. Existing dirty and untracked files were preserved. Nothing was staged or committed.
