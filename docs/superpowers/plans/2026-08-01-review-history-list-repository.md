# Review History List Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate review-history list filtering, pagination, summary projection, and subject facets from detail audit projection without changing the public API or persisted-data behavior.

**Architecture:** Keep `review_history.rs` as the public DTO/error owner and detail-query facade. Add a private `review_history_list_repository.rs` child that owns list validation, cursor encoding, list/count/facet SQL, and note previews; the facade preserves the public list function as a direct delegation.

**Tech Stack:** Rust 2024, rusqlite, serde/base64 cursor encoding, Vitest source contracts, existing review-history integration tests

## Global Constraints

- Preserve every public signature, DTO shape, error variant, SQL join/filter/order, cursor wire format, limit/range/search/subject validation, wildcard escaping, note preview, version flag, rating mapping, count, and subject-facet behavior.
- Preserve account/profile scoping in every list, count, facet, and detail query; never expose raw device IDs.
- Keep detail audit facts and current-schedule projection in `review_history.rs`.
- Declare the child privately; do not modify commands, bindings, existing Rust tests, or other modules.
- Format only the selected facade and new child; preserve the dirty worktree and do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/review-history-list-repository-boundary.test.ts`
- Test: `src-tauri/tests/review_history_store.rs`

- [x] **Step 1: Add the failing source contract**

Assert private child declaration and public delegation, list-query ownership in the child, detail/audit ownership in the facade, exactly one child operation, and preservation of account/profile joins and shared persisted-rating validation.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/review-history-list-repository-boundary.test.ts`

Expected: FAIL because the private list repository and delegation do not exist.

- [x] **Step 3: Run existing history characterization tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_history_store`

Expected: 4/4 pagination, validation, profile-scope, detail, and corrupt-link cases pass before extraction.

### Task 2: Extract List Repository Ownership

**Files:**
- Create: `src-tauri/src/modules/review_history_list_repository.rs`
- Modify: `src-tauri/src/modules/review_history.rs`

- [x] **Step 1: Move list-only behavior into the private child**

Move the list implementation, filter SQL, validated query/cursor types, cursor codec, query validation, LIKE escaping, note preview, and rating-to-filter mapping without semantic edits.

- [x] **Step 2: Preserve a stable public facade**

Keep `pub fn list_review_history` at the same module path and delegate directly. Keep detail projection, shared persisted-rating parsing, count bounding, public DTOs/errors, and the event-id bound in the facade.

- [x] **Step 3: Format and run focused green tests**

Run: `rustfmt --edition 2024 src-tauri/src/modules/review_history.rs src-tauri/src/modules/review_history_list_repository.rs`

Run the source contract and `review_history_store`; both must pass.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent command/review suites**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_history_command --test command_contract --test review_store`

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, then the complete Rust suite.

- [x] **Step 3: Run frontend and static gates**

Run complete Vitest, typecheck, and lint.

### Task 4: Review, Hygiene, And Record

- [x] **Step 1: Review data isolation and semantic identity**

Review public API identity, validation bounds, cursor format/order, wildcard escaping, pagination, account/profile joins, count/facets, detail audit ownership, persisted-rating corruption behavior, and source-contract robustness. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks, `git diff --check`, and confirm the staged index remains empty.

- [x] **Step 3: Record exact verification evidence**

Check completed steps and append red/green totals, regression results, line counts, preserved invariants, review verdict, and exact batch scope without staging or committing.

## Verification Record

- Red phase: the new source contract failed 1/3 before extraction because the private list repository and facade delegation did not exist; the remaining two checks were intentionally guarded until the child existed.
- Characterization baseline: `review_history_store` passed 4/4 before extraction, covering deterministic pagination, bounded/malformed query rejection, filters, profile isolation, audit facts, device redaction, and corrupt cross-profile links.
- Focused green phase: the source contract passed 3/3 and `review_history_store` passed 4/4 after extraction.
- Adjacent compatibility: `command_contract` passed 8/8, `review_history_command` 1/1, and `review_store` 14/14 (23/23 total), preserving public input/error contracts and review-event compatibility.
- Rust quality gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited 0 with 113/113 library unit tests and every non-ignored integration test passing. Three environment-dependent OCR runtime/corpus probes remained explicitly ignored.
- Frontend quality gates: Vitest passed 112/112 files and 651/651 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- File shape: `review_history.rs` reduced from 509 to 244 lines. The 288-line private list repository now has one owner for list validation, range/rating/subject/search filters, escaped LIKE matching, cursor serialization, stable pagination, note previews, total count, and subject facets.
- Preserved invariants: public signatures and DTO/error shapes, cursor camel-case JSON and URL-safe unpadded base64 encoding, query bounds, range arithmetic, rating mapping, list/count/facet filters, event-time/id descending order, page lookahead, note preview, version flags, persisted-state corruption handling, and every account/profile join are unchanged. Detail audit projection and raw-device redaction remain facade-owned.
- Review verdict: no Critical or Important findings. Local review was used because this task did not authorize a reviewer subagent; the source contract requires exactly one child operation and independently locks list-policy, detail-audit, and tenant-isolation ownership.
- Hygiene and scope: target trailing-whitespace and `git diff --check` checks passed; the staged index is empty. Only the previously clean facade plus the new list repository, architecture contract, and this plan belong to this batch. Existing dirty/untracked files were preserved; nothing was staged or committed.
