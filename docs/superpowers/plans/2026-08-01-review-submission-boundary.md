# Review Submission Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate atomic review submission and deterministic FSRS schedule rebuilding from review queue/exam session orchestration without changing public review APIs or learning behavior.

**Architecture:** Keep `review.rs` as the public DTO/error/version owner and queue/manual/exam/focus lifecycle facade. Add a private `review_submission.rs` child that owns review-event insertion, deterministic event replay, schedule-state upsert, sync outbox creation, active-session advancement, interval-focus triggering, and the shared schedule rebuild used by sync pull; the facade delegates through two `pub(super)` operations.

**Tech Stack:** Rust 2024, rusqlite transactions, FSRS 6.6.1, serde JSON outbox payloads, Vitest source contracts, existing Rust review integration tests

## Global Constraints

- Preserve every public function signature, input/output field, serde/specta shape, error variant, algorithm/parameter version, rating mapping, desired retention, interval rounding, due-time calculation, event ordering, SQL predicate/order, and transaction boundary.
- Preserve problem/account/profile/status enforcement, active-session/current-card enforcement, exam grading counters, focus gating, schedule/event/outbox/session atomicity, sync replay identity, and append-only event semantics.
- Keep `submit_review` and `rebuild_schedule_for_problem` at their existing module paths and visibility; keep `ALGORITHM_VERSION` and `PARAMETER_VERSION` owned by `review.rs` for review-history compatibility.
- Keep queue listing, session resume/cleanup, manual/exam start, exam navigation/grading, focus initialization, queue SQL, and DTO/error ownership in `review.rs`.
- Declare the child privately inside `review.rs`; do not modify `modules/mod.rs`, commands, sync pull, review history, or any existing Rust test.
- Format only the selected facade and new child; preserve the dirty worktree and do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/review-submission-boundary.test.ts`
- Test: `src-tauri/tests/review_store.rs`

**Interfaces:**
- Consumes: `review.rs` and the proposed private submission child as source text.
- Produces: an architecture contract separating scheduling persistence from queue/exam lifecycle ownership.

- [x] **Step 1: Add the failing source contract**

Assert the facade privately declares `review_submission.rs`, retains same-path `submit_review` and `rebuild_schedule_for_problem` wrappers, and delegates both calls. Assert the child exposes exactly two `pub(super)` operations; owns stored events, outbox payload, FSRS replay/rating helpers, event/schedule/outbox/session SQL, and interval-focus triggering; and excludes queue/manual/exam lifecycle operations.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/review-submission-boundary.test.ts`

Expected: FAIL because the private submission module and delegations do not exist.

- [x] **Step 3: Run existing review characterization tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store`

Expected: all due/manual/exam/resume/focus/submission/schedule/account-boundary cases pass before extraction.

### Task 2: Extract Submission And Schedule Ownership

**Files:**
- Create: `src-tauri/src/modules/review_submission.rs`
- Modify: `src-tauri/src/modules/review.rs`

**Interfaces:**
- Produces: `pub(super) fn submit_review(&mut Connection, SubmitReview) -> Result<ReviewSubmission, ReviewUseCaseError>`.
- Produces: `pub(super) fn rebuild_schedule_for_problem(&Transaction<'_>, &str, &str, i64) -> Result<(), ReviewUseCaseError>`.
- Consumes: facade-owned input/output/error/version types and `FsrsRating`; review-focus interval trigger.

- [x] **Step 1: Create the private submission repository**

Move `StoredEvent`, `ReviewEventPayload`, the current `submit_review` body, `rebuild_schedule`, the current `rebuild_schedule_for_problem` body, `rating_label`, `parse_rating`, and scheduling-only constants/imports into `review_submission.rs`. Retain the exact insert/query/upsert/outbox/session order and commit point.

- [x] **Step 2: Reduce the facade to stable wrappers and queue lifecycle**

Add:

```rust
#[path = "review_submission.rs"]
mod submission;
```

Keep the public `submit_review` signature and `pub(crate)` sync-rebuild signature in `review.rs`, replacing their bodies with direct child delegation. Remove submission-only imports/constants/types/helpers; leave queue, exam, and focus initialization code unchanged.

- [x] **Step 3: Format only the two target Rust files**

Run: `rustfmt --edition 2024 src-tauri/src/modules/review.rs src-tauri/src/modules/review_submission.rs`

Expected: only the selected facade and new child receive formatting writes.

- [x] **Step 4: Run focused structure and review behavior tests**

Run: `npm test -- --run tests/review-submission-boundary.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store`

Expected: the boundary contract and complete review store suite pass unchanged.

### Task 3: Sync Compatibility And Full Regression

**Files:**
- Modify only Task 1 or Task 2 files if verification exposes a regression.

**Interfaces:**
- Consumes: library command, sync pull, review history, complete Rust crate, and frontend source/interaction contracts.
- Produces: cross-module and commercial quality-gate evidence.

- [x] **Step 1: Run adjacent review and sync suites**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test library_command --test sync_pull --test review_history_store`

Expected: exam command behavior, remote event schedule replay, and algorithm-version history flags remain compatible.

- [x] **Step 2: Run strict Rust lint and complete Rust tests**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml`

Expected: zero Rust warnings and every non-ignored Rust test passes.

- [x] **Step 3: Run frontend contracts and static gates**

Run: `npm test -- --run`

Run: `npm run typecheck`

Run: `npm run lint`

Expected: all source/interaction tests, Vue types, and zero-warning lint pass.

### Task 4: Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-review-submission-boundary.md`

**Interfaces:**
- Consumes: target source/diff, validation output, file status, and review findings.
- Produces: checked plan and exact verification record without staging or committing.

- [x] **Step 1: Perform final code review**

Review signature/visibility identity, transaction statement order, active-session check, event ordering, FSRS reducer identity, rating mapping, schedule upsert, outbox payload, exam counters, focus triggering, cross-account protection, queue isolation, source-contract robustness, and overlap with dirty files. Fix every Critical or Important finding.

- [x] **Step 2: Verify patch hygiene and scope**

Run targeted trailing-whitespace checks and `git diff --check`, then `git diff --cached --name-only`.

Expected: target files are clean, the staged index is empty, and only the previously clean facade plus three new files belong to this batch.

- [x] **Step 3: Record evidence without committing**

Check every completed step and append red/green totals, focused/adjacent/full results, file-line reduction, preserved transaction/scheduling invariants, review verdict, hygiene, index, and exact scope. Do not stage or commit.

## Verification Record

- Red phase: the new source contract failed 2/2 before extraction because the private submission module and facade delegations did not exist.
- Characterization baseline: `review_store` passed 14/14 before extraction, covering queue/manual/exam/resume behavior, focus lifecycle, cross-account rejection, atomic event/schedule/outbox persistence, and deterministic schedule replay.
- Focused green phase: the source contract passed 2/2 and `review_store` passed 14/14 after extraction.
- Adjacent compatibility: `library_command` passed 2/2, `review_history_store` 4/4, and `sync_pull` 8/8 (14/14 total), preserving exam answer isolation, version flags, and remote deterministic schedule rebuild.
- Rust quality gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited 0. Three environment-dependent OCR runtime/corpus probes remained explicitly ignored.
- Frontend quality gates: Vitest passed 111/111 files and 648/648 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- File shape: `review.rs` reduced from 827 to 592 lines. The 264-line private submission repository now has one owner for review-event persistence, FSRS replay, schedule upsert, sync outbox creation, session advancement, exam counters, and interval-focus triggering.
- Preserved invariants: public/cross-module signatures, algorithm and parameter versions, desired retention, elapsed-day and interval rules, rating mapping, problem/account/profile/status checks, active-current-card checks, event ordering, statement order, one-transaction atomicity, focus gating, sync replay identity, and queue/exam lifecycle behavior are unchanged.
- Review verdict: no Critical or Important findings. The source contract requires exactly two child operations, isolates queue/exam ownership, locks scheduling constants to the child, and verifies event → schedule → outbox → session → focus → commit source order. Its import-vs-call locator was corrected during review and the contract/full suite were rerun green.
- Hygiene and scope: target trailing-whitespace and `git diff --check` checks passed; the staged index is empty. Only the previously clean facade plus the new submission repository, architecture contract, and this plan belong to this batch. Existing dirty/untracked files were preserved; nothing was staged or committed.
