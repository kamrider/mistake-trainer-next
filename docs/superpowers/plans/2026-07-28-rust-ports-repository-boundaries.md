# Rust Ports and Repository Boundary Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the capture, sync, and backup module responsibilities by moving external I/O contracts and repository helpers behind named boundaries without changing user-visible behavior.

**Architecture:** Define cloud push/pull ports in `application::ports`, keep Supabase as an infrastructure adapter with compatibility re-exports, and extract capture-asset and backup-package repository helpers into focused child modules. Existing public use-case functions remain facades so command and integration-test contracts stay stable.

**Tech Stack:** Rust 2024, Tauri 2, rusqlite/SQLCipher, reqwest, serde, thiserror, cargo test/clippy/rustfmt.

## Global Constraints

- Do not edit `src-tauri/src/infrastructure/recognition_visual_split.rs`; it contains user work.
- Preserve all existing public command signatures, Specta bindings, database schema, backup format, sync wire format, stable error codes, and security budgets.
- Compatibility re-exports may remain in `infrastructure::supabase` for existing callers, but production sync use cases must import ports from `application::ports::sync`.
- Do not stage or commit the dirty worktree.
- Do not implement deferred pre-launch policy/operations work.

---

## Chunk 1: Application Sync Ports

### Task 1: Move transport contracts out of the Supabase adapter

**Files:**
- Create: `src-tauri/src/application/ports/mod.rs`
- Create: `src-tauri/src/application/ports/sync.rs`
- Modify: `src-tauri/src/application/mod.rs`
- Modify: `src-tauri/src/infrastructure/supabase.rs`
- Modify: `src-tauri/src/modules/sync_push.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`

- [x] Add the transport error, push/pull DTOs, and `CloudPushTransport`/`CloudPullTransport` traits under `application::ports::sync`.
- [x] Re-export those types from `infrastructure::supabase` so existing integration tests and auth code remain source-compatible.
- [x] Remove duplicate definitions from the Supabase adapter.
- [x] Change sync push/pull use cases to depend on `application::ports::sync`, not `infrastructure::supabase`.
- [x] Add a module-level contract test proving the retryability classification remains stable.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --test sync_push --test sync_pull --test supabase_client`.

---

## Chunk 2: Capture Asset Repository

### Task 2: Extract encrypted capture-asset I/O

**Files:**
- Create: `src-tauri/src/modules/capture_asset_repository.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Test: module tests in `src-tauri/src/modules/capture_asset_repository.rs`
- Modify: `src-tauri/src/modules/mod.rs`

- [x] Move relative-path validation, bounded encrypted reads, idempotent removal, and media-type mapping into the repository module.
- [x] Re-export the existing helper names from `capture_inbox` so recognition and capture callers do not change.
- [x] Add tests for traversal rejection, oversized file rejection, missing-file removal, and supported formats.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib capture_asset_repository::tests`, then `cargo test --manifest-path src-tauri/Cargo.toml --test capture_inbox_store --test capture_inbox_command`.

---

## Chunk 3: Backup Package Repository

### Task 3: Extract package filesystem and integrity helpers

**Files:**
- Create: `src-tauri/src/modules/backup_package_repository.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Test: module tests in `src-tauri/src/modules/backup_package_repository.rs`
- Modify: `src-tauri/src/modules/mod.rs`

- [x] Move safe relative-path handling, Windows reserved-name checks, contained-file verification, bounded reads, copy/hash, and byte hashing into the repository module.
- [x] Keep backup manifest models and orchestration in `backup.rs`; expose only a crate-internal repository API.
- [x] Preserve all size limits and map repository failures to the existing `BackupError` variants.
- [x] Add direct boundary tests for traversal, reserved Windows names, size budgets, and deterministic hashes.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib backup_package_repository::tests`, then `cargo test --manifest-path src-tauri/Cargo.toml --test backup_store`.

---

## Chunk 4: Architecture and Regression Gates

### Task 4: Enforce dependency direction and compatibility

**Files:**
- Create: `scripts/rust-boundary-contract.ps1`
- Modify: `package.json`
- Modify: `.github/workflows/ci.yml`
- Test: `tests/repository-contract.test.ts`

- [x] Add a deterministic contract script that rejects sync use-case imports from `infrastructure::supabase`, capture asset filesystem helpers inside `capture_inbox.rs`, and backup path/hash helper implementations inside `backup.rs`.
- [x] Lock compatibility re-exports and reject duplicate sync transport definitions in the adapter.
- [x] Add `contract:rust-boundaries` to package scripts and run it before Rust compilation in CI.
- [x] Run `pnpm contract:rust-boundaries` and `pnpm exec vitest run tests/repository-contract.test.ts`.
- [x] Run `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`.
- [x] Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`.
- [x] Run `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`.

## Self-Review

- [x] Verify file counts/line counts show real code moved out of the three large responsibility areas.
- [x] Verify all compatibility re-exports are intentional and enforced by the boundary contract.
- [x] Verify error codes, backup format, sync payloads, and capture safety limits are unchanged through focused unit and integration suites.
- [x] Verify the user-owned recognition file has no diff introduced by this work.
