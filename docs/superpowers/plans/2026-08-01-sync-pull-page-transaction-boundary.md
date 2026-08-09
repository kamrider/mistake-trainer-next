# Sync Pull Page Transaction Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the complete single-page pull transaction from the sync network facade into one private child without changing transport behavior, merge semantics, asset ownership, cursor durability, or public APIs.

**Architecture:** Add `sync_pull_transaction.rs` as a private direct child of `sync_pull.rs`. The facade retains remote paging, download validation, encryption, staging, and cleanup; the transaction child owns asset promotion, entity merges, conflict/tombstone persistence, schedule rebuilds, cursor advancement, commit, and post-commit orphan deletion.

**Tech Stack:** Rust 2024, rusqlite, serde_json, Vitest source contracts, Cargo integration tests, PowerShell architecture contracts.

## Global Constraints

- Preserve `pull_until_current` and all public types, signatures, error variants/messages, transport calls, page limits, account validation, cursor semantics, merge policy, conflict/outbox/snapshot ordering, tombstone behavior, and asset cleanup behavior.
- Keep `sync_pull_transaction.rs`, `sync_pull_asset_staging.rs`, and `sync_pull_decoder.rs` private direct children of `sync_pull.rs`.
- Keep asynchronous transport, download validation, hashing/encryption, staging creation, and page cleanup in the facade; keep filesystem promotion ownership in the staging child; keep all SQLite page mutation in the transaction child.
- Preserve the invariant that the pull cursor advances in the same transaction as the page data and orphaned blobs are removed only after a successful commit.
- Do not change recognition/OCR code, migrations, device-migration UX, update recovery, licensing, privacy, support, account deletion, or SLA work.
- Preserve all pre-existing dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Format only `src-tauri/src/modules/sync_pull.rs` and `src-tauri/src/modules/sync_pull_transaction.rs` with direct `rustfmt --edition 2024`; do not run repository-wide Cargo formatting.
- Do not stage or commit.

---

### Task 1: Lock The Page Transaction Boundary

**Files:**

- Create: `tests/sync-pull-transaction-boundary.test.ts`
- Verify: `tests/sync-pull-asset-staging-ownership.test.ts`
- Verify: `src-tauri/tests/sync_pull.rs`

**Interfaces:**

- Consumes: the current private `apply_page` transaction and helpers in `sync_pull.rs`.
- Produces: a failing source contract requiring a private transaction child and a transport-only facade.

- [x] **Step 1: Record the unchanged baseline**

Run:

```powershell
npm test -- --run tests/sync-pull-asset-staging-ownership.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull
```

Expected: the source contract passes 3/3 and the integration suite passes 10/10 before extraction.

- [x] **Step 2: Add the failing transaction source contract**

Require:

- `sync_pull.rs` privately declares `sync_pull_transaction.rs`, imports `apply_page`, and no longer defines it.
- The child owns `apply_page`, the SQLite transaction/commit, profile/problem/export/tombstone merge helpers, upserts, schedule rebuild, cursor advancement, and post-commit orphan cleanup.
- The facade retains `pull_until_current`, transport/download calls, validation, encryption, staging, and page cleanup.
- The transaction child contains no asynchronous or cloud transport/download implementation.

Run:

```powershell
npm test -- --run tests/sync-pull-transaction-boundary.test.ts
```

Expected: FAIL because the child does not yet exist and the facade still owns `apply_page`.

---

### Task 2: Extract The Transaction Without Behavioral Change

**Files:**

- Create: `src-tauri/src/modules/sync_pull_transaction.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`
- Modify: `tests/sync-pull-asset-staging-ownership.test.ts`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**

- `sync_pull_transaction::apply_page` remains visible only to its parent module.
- The child consumes `StagedAsset`, `DecodedChange`, existing wire types, merge/conflict helpers, and `validate_uuid`.
- The facade invokes the child through the unchanged call site and retains all success/error cleanup branches.

- [x] **Step 1: Move the complete page transaction**

Move `apply_page` and its merge/upsert/tombstone/orphan helpers verbatim into the child. Shrink parent imports without changing error conversion or public behavior.

- [x] **Step 2: Update ownership contracts**

Change the existing staging contract so database promotion and commit tokens are required in the transaction child, while transport/validation/encryption and cleanup remain in the facade. Extend `rust-boundary-contract.ps1` to prevent transaction helpers or cloud transport from crossing back over the boundary.

- [x] **Step 3: Format only touched Rust files**

Run direct `rustfmt --edition 2024` against the two touched Rust modules.

- [x] **Step 4: Prove the focused boundary**

Run:

```powershell
npm test -- --run tests/sync-pull-transaction-boundary.test.ts tests/sync-pull-asset-staging-ownership.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull
```

Expected: source contracts pass 6/6 and sync pull integration passes 10/10.

---

### Task 3: Verify Adjacent Sync And Architecture Behavior

- [x] **Step 1: Run the architecture contract**

```powershell
powershell -ExecutionPolicy Bypass -File scripts\rust-boundary-contract.ps1
```

- [x] **Step 2: Run staging and adjacent sync suites**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib asset_staging::tests
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_push
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema
```

Expected: staging child tests pass 3/3; existing adjacent suite totals remain unchanged.

---

### Task 4: Run Commercial-Quality Gates And Review

- [x] **Step 1: Run strict Rust gates**

```powershell
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml
```

- [x] **Step 2: Run full frontend/source gates**

```powershell
npm test -- --run
npm run typecheck
npm run lint
```

Expected after the new three-case source contract: 125 Vitest files and 690 tests pass.

- [x] **Step 3: Perform local code review**

Inspect the diff for transaction atomicity, asset promotion/rollback ownership, conflict/outbox/snapshot ordering, affected schedule rebuilds, cursor-before-commit ordering, post-commit orphan deletion, tombstone handling, account scoping, error mapping, and accidental scope expansion.

- [x] **Step 4: Record hygiene and evidence**

Record focused, adjacent, and full results below. Confirm no staged changes, no generated build artifacts, no edits to `recognition_visual_split.rs`, and no excluded launch-readiness work.

## Verification Record

- Baseline: existing asset-staging source contract passed 3/3; `sync_pull` integration passed 10/10.
- Red contract: the new three-case transaction contract failed at the missing private child declaration before source extraction.
- Focused: transaction plus staging source contracts passed 6/6; `sync_pull` integration passed 10/10.
- Adjacent: Rust architecture boundary contract passed; asset-staging ownership passed 3/3; `sync_store` passed 6/6; `sync_push` passed 7/7; `database_schema` passed 15/15.
- Full Rust: strict all-target/all-feature Clippy passed with `-D warnings`; the full test command passed, including 127 library tests discovered (124 passed, 3 environment-dependent tests ignored) and every integration target.
- Full frontend/source: Vitest passed 125 files and 690 tests; `vue-tsc --noEmit` and ESLint with zero warnings passed.
- Local review: no findings. `sync_pull.rs` is 241 lines and retains paging, download validation, encryption, staging, cleanup, and UUID validation. `sync_pull_transaction.rs` is 709 lines and preserves asset promotion before row insertion, merge/conflict/outbox/snapshot ordering, affected-schedule rebuilds, cursor write before commit, and orphan deletion after commit.
- Hygiene: targeted `git diff --check` passed; no files were staged; no build artifacts entered status; the pre-existing `recognition_visual_split.rs` modification was not touched; excluded launch-readiness work was not changed.
