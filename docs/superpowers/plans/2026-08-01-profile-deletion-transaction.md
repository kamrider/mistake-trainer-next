# Profile Deletion Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate destructive profile deletion, active-profile replacement, dependent-data cleanup, orphan-asset collection, sync tombstones, and delete outbox writes from ordinary profile CRUD without changing behavior.

**Architecture:** Keep `profiles.rs` as the public DTO/error/API owner and the owner of create/list/rename/active-selection behavior. Add a private `profile_deletion.rs` transaction component exposing one `pub(super)` operation; the facade preserves `delete_profile` as a direct wrapper while the child owns the complete statement order and single commit.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher transactions, UUID v7, Vitest source contracts, Rust profile store/command integration tests

## Global Constraints

- Preserve every public signature, DTO/error field, name confirmation rule, account/profile scope, replacement-profile ordering, active preference behavior, deletion statement order, transaction boundary, receipt field, and error timing.
- Preserve dependent review/schedule/export/capture/problem deletion through schema cascades, shared-asset retention, orphan selection/removal, asset file receipt ordering, sync-operation cleanup/nulling, conflict/tombstone cleanup, profile and asset tombstones, revision rules, 30-day retention, profile delete timestamp, asset delete `+1ms` ordering, and one final commit.
- Keep create/list/rename/persist-active operations, profile-name validation, duplicate detection, DTO/error ownership, and row decoding in `profiles.rs`.
- Declare the child privately; do not modify commands, runtime, sync modules, migrations, existing Rust tests, or bindings.
- Format only the selected facade and new child; preserve the dirty worktree and do not stage or commit.
- Do not implement account deletion, licensing, privacy/legal policy text, support operations, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/profile-deletion-transaction-boundary.test.ts`
- Test: `src-tauri/tests/profile_store.rs`
- Test: `src-tauri/tests/profile_command.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private child, stable public wrapper, exactly one child operation, deletion-only SQL/retention ownership, CRUD isolation, tenant scoping, orphan checks, tombstone/outbox order, and final commit.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/profile-deletion-transaction-boundary.test.ts`

Expected: FAIL because the private deletion transaction and delegation do not exist.

- [x] **Step 3: Run current profile characterization tests**

Run `profile_store` and `profile_command`. Expect 12/12 cases to pass before extraction.

### Task 2: Extract Deletion Transaction

**Files:**
- Create: `src-tauri/src/modules/profile_deletion.rs`
- Modify: `src-tauri/src/modules/profiles.rs`

- [x] **Step 1: Move the entire deletion operation without semantic edits**

Move `DELETION_RETENTION_MILLIS` and the complete `delete_profile` body into the child, importing facade-owned inputs/outputs/errors/row decoder and preserving every SQL statement and branch.

- [x] **Step 2: Keep a stable public facade**

Privately declare the child and replace the public body with `deletion::delete_profile(connection, input)`. Leave all other code unchanged.

- [x] **Step 3: Format and run focused green tests**

Format only both target Rust files. Run the source contract, `profile_store`, and `profile_command`; all must pass.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent database, sync, runtime, and storage suites**

Run database schema, sync push/pull/store, runtime state, and storage migration integrations to cover cascades, tombstones, outbox upload, active profile startup, and asset deletion compatibility.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, then the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, typecheck, and lint serially.

### Task 4: Transaction Review, Hygiene, And Record

- [x] **Step 1: Review destructive transaction semantic identity**

Review API/visibility identity, validation timing, last/foreign-profile rejection, replacement selection, tenant predicates, candidate asset query, preference update, cleanup statement order, cascades, tombstone revisions/retention, outbox ordering, shared-asset NOT EXISTS guards, receipt ordering, commit point, source contract, and dirty-file overlap. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks, `git diff --check`, and confirm the staged index remains empty.

- [x] **Step 3: Record exact verification evidence**

Check completed steps and append red/green totals, regression results, line counts, preserved invariants, review verdict, and exact batch scope without staging or committing.

## Verification Record

- Red source contract: 1 of 3 assertions passed before extraction; failure confirmed the private module and delegation were absent.
- Baseline profile characterization: `profile_store` 7/7 and `profile_command` 5/5 passed before extraction.
- Focused green: source contract 3/3 and profile characterization 12/12 passed after extraction.
- Adjacent Rust regression: database schema, sync push/pull/store, runtime state, and storage migration passed 52/52.
- Strict Rust gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited successfully, including 113/113 library tests with 3 environment-dependent OCR probes ignored as designed.
- Frontend gates: Vitest passed 115 files and 660 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Resulting boundaries: `profiles.rs` is 254 lines and keeps DTOs/errors/public CRUD; `profile_deletion.rs` is 222 lines and exposes one `pub(super)` transaction operation.
- Preserved invariants: public API/error timing, exact-name confirmation, last-profile rejection, deterministic active-profile replacement, account scoping, cascade order, shared-asset guards, 30-day retention, revision behavior, profile-delete timestamp, asset-delete `+1ms` ordering, receipt ordering, and one final commit.
- Review verdict: no Critical or Important findings. The structural contract was corrected to avoid treating unrelated CRUD commits as deletion-boundary leakage.
- Scope and hygiene: only the facade, new private transaction component, structural contract, and this plan belong to the batch; target whitespace checks and `git diff --check` pass, and the staged index remains empty.
