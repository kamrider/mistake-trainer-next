# Library Recovery Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make interrupted startup, missing storage, damaged local data, backup restoration, and fresh-start recovery deterministic and testable in the installed Windows product.

**Architecture:** Keep `LibraryAccessGate` as the startup truth source. Recovery actions are single-flight and journaled; backup restore is staged and applied only during restart. Every failure preserves the original database, storage pointer, and encrypted assets.

**Tech Stack:** Rust, SQLCipher/SQLite, Tauri 2, Vue 3, Vitest, tempfile failure injection.

## Global Constraints

- Never hot-swap an open database.
- Never overwrite the last known-good database before backup validation and restart staging complete.
- Fresh start requires explicit confirmation and preserves recoverable old data under an owned quarantine path.
- Recovery actions must remain keyboard accessible and idempotent under double activation.

---

### Task 1: Complete deterministic recovery-state tests

**Files:**
- Modify: `src-tauri/tests/backup_restore_startup.rs`
- Modify: `src-tauri/tests/storage_location.rs`
- Modify: `src-tauri/tests/storage_migration.rs`
- Modify: `src/app/App.test.ts`
- Modify: `src/app/recovery-single-flight.test.ts`

**Interfaces:**
- Consumes: `LibraryAccessState` and `LibraryRecoveryReason` from the access command.
- Produces: one available action set for healthy, locked, storage-missing, local-data-missing, and damaged states.

- [ ] **Step 1: Add a recovery decision table test**

```ts
expect(actionsFor('healthy')).toEqual([])
expect(actionsFor('storage_missing')).toEqual(['reconnect'])
expect(actionsFor('local_data_missing')).toEqual(['restore_backup', 'start_fresh'])
expect(actionsFor('locked')).toEqual(['retry'])
```

- [ ] **Step 2: Add Rust restart failure injection**

Cover interrupted staging, invalid package hash, wrong key, destination disappearance, stale journal replay, and a second process observing the same recovery journal.

- [ ] **Step 3: Run focused tests**

Run: `pnpm exec vitest run src/app/App.test.ts src/app/recovery-single-flight.test.ts`

Expected: PASS with one invocation for repeated clicks.

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_restore_startup --test storage_location --test storage_migration`

Expected: PASS; every injected failure leaves the source hash and pointer unchanged.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/backup_restore_startup.rs src-tauri/tests/storage_location.rs src-tauri/tests/storage_migration.rs src/app/App.test.ts src/app/recovery-single-flight.test.ts
git commit -m "test: complete library recovery state coverage"
```

### Task 2: Improve recovery receipts and user guidance

**Files:**
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src/app/LibraryAccessScreen.vue`
- Modify: `src/app/BackupRestoreDialog.vue`
- Modify: `src/app/App.vue`
- Test: `src/app/LibraryAccessScreen.test.ts`
- Test: `src/app/BackupRestoreDialog.test.ts`

**Interfaces:**
- Consumes: the staged restore receipt and recovery reason.
- Produces: receipt fields `operationId`, `sourceLabel`, `completedAtUtcMs`, `recoveredProfileCount`, and `recoveredProblemCount`, without absolute source paths.

- [ ] **Step 1: Add receipt and copy assertions**

```ts
expect(screen.getByRole('status')).toHaveTextContent('恢复完成')
expect(screen.getByText(/学习档案/)).toBeVisible()
expect(screen.queryByText(/C:\\\\Users\\/)).not.toBeInTheDocument()
```

- [ ] **Step 2: Extend the typed receipt**

```rust
pub struct BackupRestoreReceipt {
    pub operation_id: String,
    pub source_label: String,
    pub completed_at_utc_ms: f64,
    pub recovered_profile_count: i32,
    pub recovered_problem_count: i32,
}
```

Populate counts after the restored database passes schema validation and before the receipt is atomically written.

- [ ] **Step 3: Regenerate bindings and run UI tests**

Run: `pnpm bindings:check`

Expected: generated bindings match the committed TypeScript file.

Run: `pnpm exec vitest run src/app/LibraryAccessScreen.test.ts src/app/BackupRestoreDialog.test.ts src/app/App.test.ts`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/modules/backup.rs src-tauri/src/commands/backup.rs src/app/LibraryAccessScreen.vue src/app/BackupRestoreDialog.vue src/app/App.vue src/app/LibraryAccessScreen.test.ts src/app/BackupRestoreDialog.test.ts src/shared/api/bindings.ts
git commit -m "feat: explain completed library recovery"
```

### Task 3: Run installed recovery acceptance

**Files:**
- Create: `docs/windows-library-recovery-acceptance.md`

**Interfaces:**
- Consumes: an installed build with a disposable learner profile and verified backup.
- Produces: hashes and PASS/FAIL evidence for every destructive boundary.

- [ ] **Step 1: Exercise non-destructive failures**

Disconnect the configured external storage, rename the library directory, lock the database from a second process, interrupt migration, and present a corrupt backup. Verify retry/reconnect copy and that no healthy data is replaced.

- [ ] **Step 2: Exercise successful recovery**

Restore a verified backup, allow automatic restart, verify profile/problem/asset counts, open representative question and answer images, and create a new backup.

- [ ] **Step 3: Exercise fresh start**

Confirm the explicit phrase, verify a new library starts, and verify the previous data remains in the owned quarantine location described by the UI.

- [ ] **Step 4: Record and commit evidence**

```bash
git add docs/windows-library-recovery-acceptance.md
git commit -m "test: record installed library recovery evidence"
```
