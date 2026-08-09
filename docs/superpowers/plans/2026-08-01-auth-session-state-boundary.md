# Auth Session State Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make cloud authentication session transitions atomic and internally consistent without changing public APIs, credential durability, account binding, restore behavior, sign-out ordering, or status semantics.

**Architecture:** Keep `auth_sync.rs` as the public status/runtime/manager facade and the owner of network transport plus durable secret transactions. Add a private `auth_session_state.rs` child with one `RwLock<CloudSessionStateSnapshot>` that owns the active session, verification email, offline flag, redacted debug output, status projection, and synchronous state transitions; no lock is held across an await.

**Tech Stack:** Rust 2024, Tokio, Supabase auth transport, Windows credential storage abstraction, Vitest source contracts, Rust integration tests

## Global Constraints

- Preserve `AuthStatusKind`, `AuthStatus`, `AuthSyncManager`, `CloudAuthRuntime`, public methods, visibility, return types, error variants, and serialization.
- Preserve status priority exactly: connected, then offline, then verification required, then signed out.
- Preserve transition semantics: verified connection clears verification/offline; verification only records email; retryable restore only marks offline; authentication rejection clears session/offline but retains verification; disconnect clears all volatile state.
- Preserve credential transaction order and rollback: validate/bind user, rotate refresh token, restore the previous refresh token if first binding fails, and never mutate memory state on a durable-write failure.
- Preserve disconnect ordering: snapshot access token, clear durable refresh token, clear volatile state, then perform bounded best-effort remote revocation.
- Preserve redacted manager/session debug output and redacted status email hints; never expose access/refresh tokens or the raw remote user ID.
- The child must be synchronous, own no transport or credential-store concerns, and expose no public API outside its parent module.
- Do not edit commands, bindings, frontend code, existing Rust tests, migrations, sync payloads, or cloud protocol code.
- Format only the selected facade and new child. Preserve the dirty worktree; do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/auth-session-state-boundary.test.ts`
- Test: `src-tauri/tests/auth_sync.rs`
- Test: `src-tauri/src/modules/auth_sync.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private child and facade delegation, one-lock snapshot ownership, transition/status/debug ownership, absence of async/transport/credential concerns in the child, and continued facade ownership of durable credential transactions and bounded revocation.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/auth-session-state-boundary.test.ts`

Expected: FAIL because the session-state child and delegation do not exist.

- [x] **Step 3: Run current authentication characterization tests**

Run the `auth_sync` integration target. Expect all ten account-binding, verification, restore, disconnect, rollback, redaction, and revocation-order cases to pass before extraction.

### Task 2: Extract Atomic Volatile Session State

**Files:**
- Create: `src-tauri/src/modules/auth_session_state.rs`
- Modify: `src-tauri/src/modules/auth_sync.rs`

- [x] **Step 1: Move volatile state behind one lock**

Move `ActiveCloudSession` and its redacted debug implementation into the child. Add one snapshot containing session, verification email, and offline flag, guarded by one poison-recovering `RwLock`.

- [x] **Step 2: Centralize projection and transitions**

Move status priority, redacted email projection, manager debug formatting, connect, verification-required, offline, authentication-rejected, disconnect, access-token read, and sync-session snapshot behavior into synchronous child methods.

- [x] **Step 3: Delegate from the stable facade**

Replace the three facade locks with `CloudSessionState`, delegate all volatile reads/transitions, and leave transport calls, secret reads/writes, binding validation, rollback, and two-second best-effort revocation in their original order.

- [x] **Step 4: Format only target Rust files and run focused green tests**

Run direct `rustfmt --edition 2024` for the facade and child, then rerun the source contract and ten-case authentication integration target.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent runtime, command, and sync-store contracts**

Run the `runtime_state`, `command_contract`, and `sync_store` integration targets to verify runtime construction, command serialization, and sync session consumers remain compatible.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, followed by the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, TypeScript typechecking, and ESLint serially.

### Task 4: Concurrency/Security Review, Hygiene, And Record

- [x] **Step 1: Review semantic identity and lock safety**

Compare pre/post behavior for status priority, transition semantics, poison recovery, debug redaction, secret transaction/rollback, disconnect local-first ordering, timeout behavior, and absence of locks across await. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks and `git diff --check`; confirm the staged index remains empty and only the facade, child, source contract, and plan belong to this batch.

- [x] **Step 3: Record exact evidence**

Check completed steps and append red/green totals, regression commands, line counts, preserved invariants, review verdict, and hygiene results.

## Verification Record

- Red source contract: 2 of 3 assertions passed and 1 failed before extraction, confirming the private state child and single-lock delegation were absent.
- Baseline characterization: the existing `auth_sync` integration target passed 10/10 before extraction.
- Focused green: the source contract passed 3/3 and `auth_sync` passed 10/10 after extraction.
- Adjacent regression: `runtime_state` passed 5/5, `command_contract` passed 8/8, and `sync_store` passed 6/6.
- Strict Rust gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited successfully, including 110/113 library tests passed with 3 environment-dependent probes ignored as designed, and every integration target passed.
- Frontend gates: Vitest passed 118 files and 669 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Resulting boundaries: `auth_sync.rs` is 205 lines and owns the stable public facade, transport awaits, durable credential transaction/rollback, backend persistence, and bounded revocation; `auth_session_state.rs` is 172 lines and owns one `RwLock<CloudSessionStateSnapshot>` plus every volatile projection and transition.
- Preserved invariants: public API/serialization, status priority, targeted transition semantics, poison recovery, account binding, refresh-token rollback, memory mutation only after durable writes, redacted debug/status hints, local-first disconnect, two-second best-effort remote revocation, and sync session snapshots.
- Concurrency/security review: no Critical or Important findings; the child is synchronous and contains no await, transport, Tokio, or secret-store dependency, so no session lock crosses an asynchronous boundary.
- Scope and hygiene: only the facade, new private child, structural contract, and this plan belong to the batch; target trailing-whitespace/final-newline checks and global `git diff --check` pass, and the staged index remains empty.
