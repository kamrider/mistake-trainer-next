# Capture Asset Staging Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent capture ingestion failures from deleting pre-existing blobs or leaking staged ciphertext by giving each newly staged capture asset explicit, linear filesystem ownership.

**Architecture:** Extend the existing `capture_asset_repository.rs` filesystem boundary with a non-cloneable `StagedCaptureAsset`. It owns deterministic local paths, temporary ciphertext cleanup, guarded promotion, rollback of only files promoted by this instance, and an explicit post-transaction commit transition; `capture_inbox.rs` retains image validation, deduplication, encryption, SQL orchestration, and DTO behavior.

**Tech Stack:** Rust 2024, rusqlite/SQLCipher, encrypted capture blobs, RAII cleanup, tempfile unit tests, Vitest source contracts

## Global Constraints

- Preserve `ingest_capture_item`, all command/binding signatures, `CaptureInboxError` variants and stable command error mapping, capture limits, image validation, deduplication by account/plaintext hash, UUIDv7 identifiers, deterministic `blobs/<shard>/<uuid>.mtb` paths, database rows, batch revision behavior, and idempotent `client_upload_id` behavior.
- Keep encryption and plaintext hashing in `capture_inbox.rs`; the repository receives only the UUID asset ID and encrypted bytes.
- `StagedCaptureAsset` must not implement `Clone`. Its `moved_to_final` flag becomes true only immediately after a successful rename, and its `committed` flag becomes true only immediately after the SQL transaction commits.
- Before promotion, reject an existing final target. Dropping an uncommitted owner always removes its temporary file and removes the final file only if that owner successfully promoted it.
- A staging write failure removes any partially written temporary file. A transaction-acquisition, SQL, parent-directory, promotion, or commit failure must leave no owner-created blob behind.
- A pre-existing final file must survive promotion failure byte-for-byte, even when its path matches the staged asset ID.
- Do not add dependencies, change schema, stage, commit, or modify unrelated dirty files, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.
- Format only `capture_asset_repository.rs`, `capture_inbox.rs`, and the review-added `capture_inbox_store.rs` regression with direct Rustfmt.

---

### Task 1: Baseline And Failing Ownership Contract

**Files:**
- Create: `tests/capture-asset-staging-ownership.test.ts`
- Test: `src-tauri/src/modules/capture_asset_repository.rs`
- Test: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/tests/capture_inbox_store.rs`

**Interfaces:**
- Consumes: existing capture asset repository, `ingest_capture_item`, and current tuple-based staging code.
- Produces: a red source contract that fixes the intended ownership and orchestration boundary.

- [x] **Step 1: Record the unchanged baseline**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib capture_asset_repository
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store
```

Expected: both existing targets pass before the new contract or production changes.

- [x] **Step 2: Add the source contract**

Create `tests/capture-asset-staging-ownership.test.ts` with assertions equivalent to:

```ts
expect(repository).toContain('pub(crate) struct StagedCaptureAsset')
expect(repository).toContain('moved_to_final: bool')
expect(repository).toContain('committed: bool')
expect(repository).toContain('pub(crate) fn stage_encrypted_capture_asset(')
expect(repository).toContain('pub(crate) fn promote(&mut self)')
expect(repository).toContain('pub(crate) fn mark_committed(&mut self)')
expect(repository).toContain('impl Drop for StagedCaptureAsset')
expect(repository).toContain('if self.moved_to_final && !self.committed')
expect(orchestrator).not.toContain('Option<(String, PathBuf, PathBuf, String)>')
expect(orchestrator).not.toContain('std::fs::rename(staged_path, final_path)')
expect(orchestrator).not.toContain('std::fs::remove_file(final_path)')
```

Also assert the four lifecycle unit-test names from Task 2 and that the repository remains free of SQL, async, image decoding, plaintext hashing, and encryption.

- [x] **Step 3: Run the contract red**

Run:

```powershell
npm test -- --run tests/capture-asset-staging-ownership.test.ts
```

Expected: FAIL because the ownership type and lifecycle methods do not yet exist and the orchestrator still contains tuple-based unconditional cleanup.

### Task 2: Linear Staged Capture Asset

**Files:**
- Modify: `src-tauri/src/modules/capture_asset_repository.rs`
- Modify: `src-tauri/src/modules/capture_inbox.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) fn stage_encrypted_capture_asset(
    blob_root: &Path,
    asset_id: String,
    encrypted: &[u8],
) -> Result<StagedCaptureAsset, CaptureInboxError>

impl StagedCaptureAsset {
    pub(crate) fn asset_id(&self) -> &str;
    pub(crate) fn relative_path(&self) -> &str;
    pub(crate) fn promote(&mut self) -> Result<(), CaptureInboxError>;
    pub(crate) fn mark_committed(&mut self);
}
```

- Consumes: `CaptureInboxError`, `blob_root`, a UUIDv7 string created by the parent, and ciphertext created by `encrypt_asset`.

- [x] **Step 1: Add lifecycle unit tests**

Add repository tests named:

```rust
rollback_preserves_a_preexisting_unowned_capture_blob
rollback_removes_a_capture_blob_promoted_by_this_owner
commit_keeps_the_promoted_capture_blob
dropping_before_promotion_removes_the_staged_capture_blob
```

The first test stages a known UUID, precreates the matching final path with sentinel bytes, asserts `promote` fails, drops the owner, and asserts the sentinel is unchanged. The second promotes then drops without commit and asserts both staged and final paths are absent. The third promotes, marks committed, drops, and asserts final ciphertext remains. The fourth drops before promotion and asserts the `.capture.tmp` file is absent.

- [x] **Step 2: Implement the ownership object**

Add a non-`Clone` structure:

```rust
#[derive(Debug)]
pub(crate) struct StagedCaptureAsset {
    asset_id: String,
    relative_path: String,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
    committed: bool,
}
```

`stage_encrypted_capture_asset` creates `.staging`, writes `<asset-id>.capture.tmp`, removes a partial file if `write` fails, and initializes both flags to false. `promote` creates the shard directory, returns an `AlreadyExists` I/O error if the target exists, renames, and immediately sets `moved_to_final = true`. `mark_committed` asserts the file was promoted and sets `committed = true`. `Drop` always removes the staged path and removes the final path only under `moved_to_final && !committed`.

- [x] **Step 3: Delegate ingestion orchestration**

In `capture_inbox.rs`, import `stage_encrypted_capture_asset` and `StagedCaptureAsset`, replace the staging tuple with `Option<StagedCaptureAsset>`, keep UUID generation and encryption in the parent, obtain SQL values through `asset_id()` and `relative_path()`, call `promote()` inside the transaction, and call `mark_committed()` immediately after `transaction.commit()?`. Remove explicit failure cleanup so early `?` returns are covered by the owner drop.

- [x] **Step 4: Format only the three target Rust files**

Run:

```powershell
rustfmt --edition 2024 src-tauri/src/modules/capture_asset_repository.rs src-tauri/src/modules/capture_inbox.rs src-tauri/tests/capture_inbox_store.rs
```

Expected: only those three files are formatted.

- [x] **Step 5: Run focused green tests**

Run:

```powershell
npm test -- --run tests/capture-asset-staging-ownership.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib capture_asset_repository
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store
```

Expected: source contract, four lifecycle tests, and the complete capture inbox store target pass.

### Task 3: Adjacent And Full Regression

**Files:**
- Verify: `src-tauri/src/modules/capture_asset_repository.rs`
- Verify: `src-tauri/src/modules/capture_inbox.rs`

**Interfaces:**
- Consumes: final ownership-aware staging implementation.
- Produces: evidence that capture commands, crop/organizer/recognition flows, and the complete product remain compatible.

- [x] **Step 1: Run adjacent capture targets**

Run the existing `capture_inbox_command`, `capture_recognition`, and relevant library capture tests. Expected: all non-environmental tests pass; only the already documented real-corpus/OCR runtime tests may remain ignored.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, then the complete Rust suite. Expected: exit code 0 for both.

- [x] **Step 3: Run frontend and static gates serially**

Run complete Vitest, `vue-tsc --noEmit`, and ESLint with zero warnings. Expected: exit code 0 for each.

### Task 4: Ownership Review, Hygiene, And Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-capture-asset-staging-ownership.md`

**Interfaces:**
- Consumes: final diff and verification output.
- Produces: an auditable record of red/green evidence and every filesystem transition.

- [x] **Step 1: Review every lifecycle transition**

Review encryption failure, staging-directory failure, partial write failure, transaction-acquisition failure, asset-row failure, pre-existing target, parent-directory failure, successful promotion, item/batch SQL failure, commit failure, successful commit, known-asset deduplication, idempotent replay, and post-commit DTO read. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Check target trailing whitespace/final newlines, run global `git diff --check`, confirm the staged index remains empty, and confirm only the repository, orchestrator, source contract, and this plan belong to the batch.

- [x] **Step 3: Record exact evidence**

Check all steps and replace the pending record below with baseline/red/green totals, exact full-suite totals, file line counts, fixed scenarios, preserved invariants, review verdict, and hygiene results.

## Self-Review

- Spec coverage: the plan covers pre-existing target preservation, early-return staging cleanup, transaction rollback, successful retention, structural responsibility, adjacent behavior, full gates, and workspace preservation.
- Placeholder scan: no TBD, TODO, “implement later,” undefined error handling, or unspecified test request remains.
- Type consistency: the parent-created `String` asset ID flows into `stage_encrypted_capture_asset`; SQL borrows `asset_id()` and `relative_path()`; `promote` and `mark_committed` require mutable ownership; `Drop` handles every uncommitted exit.

## Verification Record

- Baseline: repository unit target passed 4/4; `capture_inbox_store` passed 14/14 before production changes.
- Red evidence: the new ownership source contract failed 0/3 because the owner type, lifecycle transitions, and orchestration delegation were absent.
- Green evidence: ownership source contract passed 3/3; repository unit target passed 8/8; final `capture_inbox_store` passed 15/15, including the exact transaction-acquisition failure regression 1/1.
- Fixed failures: rollback can no longer delete a pre-existing unowned blob; transaction acquisition and later SQL/commit failures clean staged ciphertext; partial staging writes are removed; only the owner that successfully promoted a blob may roll it back.
- Adjacent capture evidence: `capture_inbox_command` passed 2/2; `capture_recognition` passed 24 tests with 2 documented tests ignored; capture-prefixed library tests passed 36/36.
- Strict Rust evidence: all-target/all-feature Clippy with `-D warnings` passed. The complete Rust suite discovered 122 library tests (119 passed, 3 ignored), and every integration target passed, including the final 15/15 inbox store target.
- Frontend/static evidence: complete Vitest passed 121 files and 678 tests; TypeScript checking passed; ESLint passed with zero warnings. One earlier `App.test.ts` navigation timing failure was isolated as non-product flakiness: its focused target passed 6/6 and subsequent complete runs passed.
- Architecture result: `capture_inbox.rs` retains validation, plaintext hashing, encryption, deduplication, SQL transaction orchestration, idempotency, and DTO behavior. `capture_asset_repository.rs` exclusively owns staged/final filesystem transitions through a non-cloneable RAII owner.
- Lifecycle review: encryption, directory creation, partial write, transaction acquisition, row insertion, pre-existing target, promotion, later SQL, commit, deduplication, idempotent replay, and post-commit DTO-read transitions were reviewed. The review found one Important evidence gap for transaction acquisition; the integration regression was added and passed. No Critical or Important finding remains.
- Preserved invariants: public commands/bindings/errors, schema, limits, image validation, UUIDv7 IDs, deterministic blob paths, account/plaintext-hash deduplication, batch revisions, and `client_upload_id` behavior remain unchanged.
- File sizes: `capture_asset_repository.rs` 292 lines; `capture_inbox.rs` 500 lines; `capture_inbox_store.rs` 1,346 lines; ownership source contract 101 lines; this implementation record 223 lines.
- Hygiene: target files have final newlines and no trailing whitespace; global `git diff --check` exits 0 apart from existing line-ending notices; the staged index is empty. Unrelated dirty files, especially `recognition_visual_split.rs`, were not modified, staged, or reverted.
