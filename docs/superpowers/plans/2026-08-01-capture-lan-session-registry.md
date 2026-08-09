# Capture LAN Session Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make LAN capture session registration, expiration, replacement cleanup, stopping, and final-owner shutdown atomic so stale cleanup cannot stop a newer phone-capture session.

**Architecture:** Keep `capture_lan.rs` as the public manager, network bootstrap, QR/token generation, server runtime, batch validation, address discovery, and temporary-file cleanup owner. Add a private `capture_lan_session_registry.rs` child that owns `ActiveSession`, one `Arc<Mutex<Option<ActiveSession>>>`, the weak cleanup handle, redacted registry lifecycle, status projection, and all compare-and-remove operations; the parent never manipulates the lifecycle lock directly.

**Tech Stack:** Rust 2024, Tokio watch channels, Axum, rusqlite/SQLCipher, TCP listener threads, Vitest source contracts, Rust unit and integration tests

## Global Constraints

- Preserve `CaptureLanAddress`, `CaptureLanSession`, `CaptureLanContext`, `CaptureLanError`, `CaptureLanManager`, all public methods, serialization, error mapping, timeout values, status fields, and debug redaction.
- Preserve address discovery/selection, collecting-batch validation, next-sequence query, listener binding, two-worker runtime, 256-bit random token, SHA-256 token hash, QR payload, server route behavior, watchdog interval, upload concurrency, temporary-root cleanup, and best-effort shutdown signaling.
- Make expiration check-and-remove one mutex critical section; never unlock after deciding an old session is expired and then call an unqualified stop operation.
- Make server completion and thread-spawn failure remove only the matching session ID, so cleanup from an old server cannot delete a replacement session.
- Keep watch-channel sends outside the registry mutex after ownership has been taken.
- Keep the registry child synchronous and free of TCP binding, runtime construction, QR/token generation, database queries, Axum routing, filesystem cleanup, and async functions.
- Update only the one moved lifecycle type reference in `capture_lan_api_tests.rs`; preserve all existing API behavior tests.
- Format only the selected facade, child, and affected Rust test file. Preserve the dirty worktree; do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/capture-lan-session-registry-boundary.test.ts`
- Test: `src-tauri/src/modules/capture_lan.rs`
- Test: `src-tauri/src/modules/capture_lan_api_tests.rs`
- Test: `src-tauri/tests/profile_command.rs`

**Interfaces:**
- Consumes: existing `CaptureLanManager::{start,status,stop}`, `run_server`, `ServerState`, watch shutdown channel, and profile-switch/delete LAN-stop behavior.
- Produces: a source contract fixing lifecycle ownership and compare-and-remove semantics before extraction.

- [x] **Step 1: Add the failing source contract**

Assert the private registry module, facade delegation, absence of lifecycle lock/type ownership in the facade, synchronous child responsibility, matching-ID cleanup on server exit and spawn failure, and stable network/bootstrap ownership in the facade.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/capture-lan-session-registry-boundary.test.ts`

Expected: FAIL because the registry child and delegation do not exist.

- [x] **Step 3: Run current LAN and profile characterization tests**

Run: `cargo test --lib capture_lan` through the MSVC wrapper, then run the `profile_command` integration target. Record the exact passing totals before extraction.

### Task 2: Extract Atomic Session Registry

**Files:**
- Create: `src-tauri/src/modules/capture_lan_session_registry.rs`
- Modify: `src-tauri/src/modules/capture_lan.rs`
- Modify: `src-tauri/src/modules/capture_lan_api_tests.rs`

**Interfaces:**
- Produces: `CaptureLanSessionRegistry::{ensure_startable,install,status,stop,downgrade,remove_if_session,shutdown_if_last_owner}` and `WeakCaptureLanSessionRegistry::remove_if_session` with parent-only visibility.
- Consumes: `ServerState::{activity_snapshot,expires_at,is_expired}`, `CaptureLanSession`, `CaptureLanError`, and `watch::Sender<bool>`.

- [x] **Step 1: Move active-session ownership into the child**

Move `ActiveSession` and the active `Arc<Mutex<Option<_>>>` into `CaptureLanSessionRegistry`. Add a weak wrapper so the async server receives no private mutex/type details.

- [x] **Step 2: Implement atomic lifecycle operations**

Implement `ensure_startable` as a single check-and-take critical section, `install` as compare-and-insert, `status` as atomic expire-or-project, `stop` as take-and-signal, matching-ID removal for strong and weak handles, and last-owner shutdown. Drop the lock before every watch-channel send.

- [x] **Step 3: Add stale-cleanup and stop-signal unit tests**

Create child unit tests with an in-memory `ServerState`: prove an expired session can be replaced and a delayed cleanup for its ID leaves the replacement intact; prove a second live install returns `AlreadyActive` and explicit stop signals shutdown exactly once.

- [x] **Step 4: Delegate the public manager and server cleanup**

Replace the facade active mutex with the registry, delegate start preflight/install/status/stop/Drop, pass the weak registry to `run_server`, and replace spawn-failure plus server-exit cleanup with matching-ID removal. Update the shutdown API test to construct the new weak wrapper.

- [x] **Step 5: Format only target Rust files and run focused green tests**

Run direct `rustfmt --edition 2024` for the facade, child, and affected API test file. Rerun the source contract, filtered LAN library tests, and `profile_command` integration target.

### Task 3: Adjacent And Full Regression

**Interfaces:**
- Consumes: final registry implementation and unchanged public LAN manager/API contracts.
- Produces: evidence that capture commands, runtime ownership, and the full product remain compatible.

- [x] **Step 1: Run adjacent capture/runtime/command contracts**

Run `capture_inbox_command`, `runtime_state`, and `command_contract` integration targets and record exact totals.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, followed by the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, `vue-tsc --noEmit`, and ESLint with zero warnings.

### Task 4: Concurrency/Security Review, Hygiene, And Record

**Interfaces:**
- Consumes: final implementation diff and all verification output.
- Produces: reviewed, scoped batch with exact evidence in this plan.

- [x] **Step 1: Review lifecycle semantics and resource cleanup**

Compare pre/post behavior for start races, expiration, replacement, stale server exit, thread-spawn failure, explicit stop, final-owner Drop, status projection, poison handling, shutdown signaling, runtime/listener ownership, watchdog, and temporary-directory cleanup. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target trailing-whitespace/final-newline checks and global `git diff --check`; confirm the staged index remains empty and only the facade, child, affected API test, source contract, and plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check completed steps and replace the pending verification record with red/green totals, regression commands, line counts, fixed race description, preserved invariants, review verdict, and hygiene results.

## Verification Record

- Red source contract: 2 of 3 assertions passed and 1 failed before extraction, confirming the private registry and facade delegation were absent.
- Baseline characterization: filtered LAN library tests passed 11/11 and `profile_command` passed 5/5 before extraction.
- Focused green: the source contract passed 3/3, filtered LAN library tests passed 13/13 after adding two lifecycle cases, and `profile_command` remained 5/5.
- Race regression: the new stale-cleanup test proves both strong thread-spawn cleanup and weak server-exit cleanup for an old session ID leave a replacement session active; the stop test proves a live session receives shutdown and repeated stop is idempotent.
- Adjacent regression: `capture_inbox_command` passed 2/2, `runtime_state` passed 5/5, and `command_contract` passed 8/8.
- Strict Rust gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited successfully, including 112/115 library tests passed with 3 environment-dependent probes ignored as designed, and every integration target passed.
- Frontend gates: Vitest passed 119 files and 672 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Resulting boundaries: `capture_lan.rs` is 423 lines and owns the stable public manager, network/bootstrap/security/database/server/filesystem work; `capture_lan_session_registry.rs` owns the active-session mutex, atomic lifecycle operations, weak cleanup handle, projection, and focused concurrency tests.
- Fixed concurrency defect: expiration no longer checks under one lock acquisition and later invokes an unqualified stop; check-and-take is one critical section, and all delayed cleanup compares the original session ID before removal.
- Preserved invariants: public API/serialization/errors, manager debug redaction, timeout values, address and batch validation, next sequence, listener/runtime/token/hash/QR construction, upload semaphore, watchdog, Axum shutdown, temporary-root cleanup, poison behavior, status fields, explicit stop, and last-owner Drop shutdown.
- Concurrency/security review: no Critical or Important findings; shutdown sends happen after the lifecycle mutex guard is released, registry code is synchronous, and network/async/security/database responsibilities remain outside it.
- Scope and hygiene: this batch touches the LAN facade lifecycle sections, new registry, one moved weak-handle reference in the already-untracked API test file, the source contract, and this plan; target trailing-whitespace/final-newline checks and global `git diff --check` pass, unrelated dirty files remain untouched, and the staged index remains empty.
