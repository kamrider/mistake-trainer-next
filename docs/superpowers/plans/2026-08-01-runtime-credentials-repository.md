# Runtime Credentials Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate local-library credential storage, generation, validation, and lock state from database/profile runtime orchestration without changing public imports or fail-closed startup behavior.

**Architecture:** Keep `runtime.rs` as the stable public facade, `RuntimeError` owner, `LibraryRuntime` state coordinator, encrypted-database bootstrapper, and active-profile owner. Add a private `runtime_credentials.rs` repository that owns `SecretStore`, the keyring adapter, credential names, new-library credential creation, existing-library credential validation, restore credential loading, key encoding, and the strict lock marker; the facade re-exports public types and delegates existing functions.

**Tech Stack:** Rust 2024, keyring, getrandom, UUID v7, rusqlite/SQLCipher, Vitest source contracts, Rust runtime/startup integration tests

## Global Constraints

- Preserve every public and `pub(crate)` path, signature, type, field, error variant/code, secret name, read/write order, and debug redaction.
- Preserve fail-closed behavior: existing data with any missing required credential must never generate a replacement; malformed database/asset/account/device credentials and malformed lock state retain their exact errors.
- Preserve 32-byte lowercase hexadecimal keys, operating-system CSPRNG generation, UUID v7 account/device creation, device write behavior, and restore credential ordering.
- Keep filesystem creation, encrypted database open/migrations, preferred/default profile selection, default profile creation, preference persistence, runtime locks, and active-profile mutation in `runtime.rs`.
- Declare the repository privately; do not modify startup, commands, storage migration, auth sync, tests, bindings, or dependencies.
- Format only the selected facade and new repository; preserve the dirty worktree and do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Characterization Baseline

**Files:**
- Create: `tests/runtime-credentials-repository-boundary.test.ts`
- Test: `src-tauri/tests/runtime_state.rs`
- Test: `src-tauri/tests/backup_restore_startup.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private repository, stable re-exports/wrappers, credential-only ownership, runtime-only ownership, and fail-closed security tokens.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/runtime-credentials-repository-boundary.test.ts`

Expected: FAIL because the private credential repository and delegations do not exist.

- [x] **Step 3: Run current runtime characterization tests**

Run the runtime unit test, `runtime_state`, and `backup_restore_startup` before extraction. Expect 1 unit plus 12 integration cases to pass.

### Task 2: Extract Credential Repository

**Files:**
- Create: `src-tauri/src/infrastructure/runtime_credentials.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`

- [x] **Step 1: Move credential types and exact helper behavior**

Move `SecretStore`, `KeyringSecretStore`, `RestoreCredentials`, secret constants, lock functions, restore loading, required-secret loading, random key generation, and key decoding into the child. Add one repository operation returning validated local startup credentials in the original read/write order.

- [x] **Step 2: Preserve the runtime facade and orchestration**

Privately declare the child, publicly re-export `SecretStore`/`KeyringSecretStore`, crate-re-export `RestoreCredentials`, retain stable function wrappers, and replace inline startup credential logic with one child call. Leave database/profile/runtime state code unchanged.

- [x] **Step 3: Format and run focused green tests**

Format only both target Rust files. Run the structure contract, runtime unit test, `runtime_state`, and `backup_restore_startup`; all must pass.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent access, storage, auth, and product suites**

Run command/unit access tests plus `auth_sync`, `storage_location`, `storage_migration`, and `product_check` to cover every major credential consumer.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, then the complete Rust suite.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, typecheck, and lint serially to avoid the known one-second navigation timeout under concurrent quality gates.

### Task 4: Security Review, Hygiene, And Record

- [x] **Step 1: Review credential semantic identity**

Review API/visibility identity, secret names and access order, missing/malformed behavior, no-overwrite guarantees, key/UUID generation, restore fields, debug redaction, database/profile isolation, source contract, and overlap with dirty files. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks, `git diff --check`, and confirm the staged index remains empty.

- [x] **Step 3: Record exact verification evidence**

Check completed steps and append red/green totals, regression results, line counts, preserved invariants, review verdict, and exact batch scope without staging or committing.

## Verification Record

- Red phase: the source contract failed 1/3 before extraction because the private credentials repository and stable re-exports did not exist; the other two checks were guarded until the child existed.
- Characterization baseline: the runtime lock unit passed 1/1 and `runtime_state` plus `backup_restore_startup` passed 12/12 before extraction, covering identity/key persistence, profile persistence, malformed key rejection, missing-secret fail-closed behavior, lock/unlock recovery, encrypted assets, restore swap/rollback, and crash recovery.
- Focused green phase: the source contract passed 3/3, the runtime lock unit passed 1/1, and the same runtime/startup integrations passed 12/12 after extraction.
- Adjacent compatibility: access command units passed 3/3; `auth_sync` 10/10, `product_check` 4/4, `storage_location` 5/5, and `storage_migration` 11/11 (33/33 total), preserving every major credential consumer and the original runtime import path.
- Rust quality gates: all-target Clippy passed with `-D warnings`; the complete Rust suite exited 0 with 113/113 library unit tests and every non-ignored integration test passing. Three environment-dependent OCR runtime/corpus probes remained explicitly ignored.
- Frontend quality gates: an automatic continuation invalidated the first running Vitest result handle, so that run was not used as evidence. The authoritative serial rerun passed 114/114 files and 657/657 tests; serial `vue-tsc --noEmit` and ESLint with zero warnings passed.
- File shape: `runtime.rs` reduced from 445 to 313 lines. The 185-line private credentials repository now has one owner for secret-store/keyring access, strict lock markers, new-library credential creation, existing-library validation, restore credential loading, CSPRNG key generation, hexadecimal encoding/decoding, and UUID identity validation.
- Preserved invariants: `SecretStore`, `KeyringSecretStore`, `LIBRARY_LOCK_STATE`, `RestoreCredentials`, and all function paths/signatures; secret names and asset → database → account → device startup order; database → asset → account restore order; no replacement credentials for existing data missing asset/database/account secrets; current device-ID behavior; malformed-value errors/codes; 32-byte lowercase-hex keys from the OS random source; UUID v7 creation and UUID validation; keyring NoEntry semantics; runtime debug redaction; encrypted database/migration/profile orchestration; and active-profile locking are unchanged.
- Review verdict: no Critical or Important findings. Local review was used because this task did not authorize a reviewer subagent. The source contract limits the repository to four operations, keeps database/profile lifecycle out, locks fail-closed tokens, and verifies runtime redaction remains facade-owned.
- Hygiene and scope: target trailing-whitespace and `git diff --check` checks passed; the staged index is empty. Only the previously clean runtime facade plus the new credentials repository, architecture contract, and this plan belong to this batch. Existing dirty/untracked files were preserved; nothing was staged or committed.
