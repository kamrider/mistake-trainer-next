# Sync Conflict Merge Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate deterministic three-way merge policy from SQLite-backed conflict orchestration so sync behavior can be tested without a database and the 1610-line conflict module has a enforceable responsibility boundary.

**Architecture:** Add `sync_conflict_merge.rs` for `FieldConflict`, `MergeAction`, entity content projections, and pure profile/problem/export merge functions. `sync_conflicts.rs` remains responsible for loading local versions and snapshots, recording conflicts, applying resolutions, and transactions; its three wrapper functions delegate to the pure module.

**Tech Stack:** Rust 2024, serde/serde_json, rusqlite transaction wrappers, Cargo integration tests, PowerShell architecture contract.

## Global Constraints

- Preserve all existing merge semantics, field names, revision increments, timestamps, and conflict payloads.
- Do not alter database schemas, migrations, remote protocol types, or cloud-provider behavior.
- Do not modify OCR, recognition, capture, backup, licensing, privacy, support, account deletion, updater recovery, or SLA behavior.
- Do not stage or commit the existing dirty worktree.
- The pure module must not import `rusqlite`, `Transaction`, or database helpers.

---

### Task 1: Lock the architecture boundary

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Requires `sync_conflict_merge.rs` to contain `merge_problem_versions`, `merge_profile_versions`, and `merge_export_versions`.
- Rejects `merge_field` and `FieldDecision` definitions in `sync_conflicts.rs`.

- [x] Add `Require-Pattern` checks for all three pure merge entry points.
- [x] Add `Reject-Pattern` checks that prevent the field truth table returning to the SQLite orchestration module.
- [x] Run `npm run contract:rust-boundaries`; expect failure because the pure module does not exist.

### Task 2: Implement pure merge policy with direct unit tests

**Files:**
- Create: `src-tauri/src/modules/sync_conflict_merge.rs`
- Modify: `src-tauri/src/modules/mod.rs`

**Interfaces:**

```rust
pub(crate) fn merge_problem_versions(
    local: Option<&WireProblemAggregate>,
    base: Option<&WireProblemAggregate>,
    remote: &WireProblemAggregate,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProblemAggregate>, serde_json::Error>;

pub(crate) fn merge_profile_versions(
    local: Option<&WireProfile>,
    base: Option<&WireProfile>,
    remote: &WireProfile,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProfile>, serde_json::Error>;

pub(crate) fn merge_export_versions(
    local: Option<&WireExportSnapshot>,
    base: Option<&WireExportSnapshot>,
    remote: &WireExportSnapshot,
) -> Result<MergeAction<WireExportSnapshot>, serde_json::Error>;
```

- [x] Move `FieldConflict`, `MergeAction`, `FieldDecision`, and `merge_field` into the new module.
- [x] Move `profile_content`, `problem_content`, and `export_content` into the new module.
- [x] Adapt the three entity algorithms to accept already-loaded `local` and `base` values.
- [x] Move the field truth-table test and add direct profile revision/timestamp and problem-conflict tests.
- [x] Register `pub(crate) mod sync_conflict_merge;` in `modules/mod.rs`.

### Task 3: Reduce SQLite orchestration to loading and delegation

**Files:**
- Modify: `src-tauri/src/modules/sync_conflicts.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`

**Interfaces:**
- `merge_remote_problem/profile/export` keep their existing signatures for callers.
- `sync_pull.rs` imports `FieldConflict`, `MergeAction`, and content projections from `sync_conflict_merge`.

- [x] Replace each merge wrapper body with local/snapshot loading followed by one pure function call.
- [x] Remove the moved types, field decision algorithm, content projections, and old unit test from `sync_conflicts.rs`.
- [x] Update `sync_pull.rs` imports without changing its application flow.
- [x] Run the sync conflict and pull tests:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_conflicts --test sync_pull
```

Expected: all focused Rust integration tests pass.

### Task 4: Verify and review

- [x] Run the Rust boundary contract and `cargo check`.
- [x] Run all frontend tests, type checking, lint, and production build because generated bindings and module wiring share the release gate.
- [x] Review the diff for SQL leakage into the pure module, changed field names, changed revision/timestamp behavior, widened visibility, unrelated edits, or reduced error fidelity.
- [x] Confirm `sync_conflicts.rs` line count decreased materially and the new contract prevents regression.
