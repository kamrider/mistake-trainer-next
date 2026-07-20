# Atomic Legacy Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing read-only legacy scan into a safe, resumable-in-UI import that migrates members, paired images, metadata, review history, due dates, and frozen export groups without ever mutating the selected Electron data directory.

**Architecture:** A Rust-only `LegacyImportManager` owns the selected source path behind an opaque, expiring candidate ID. The parser produces a bounded import plan, then one filesystem staging area and one SQL transaction create encrypted assets and new-domain entities; any copy, validation, or commit failure removes staged/final blobs and rolls the database back. A v10 import ledger records entity ownership so a completed import can be inspected and explicitly rolled back without touching unrelated current data.

**Tech Stack:** Rust 1.97, rusqlite/SQLCipher, AES-GCM assets, serde, SHA-256, UUIDv7, `time = 0.3.53` with `parsing`, Tauri 2.11 typed commands, Vue 3 strict TypeScript, Vitest/Testing Library.

## Global Constraints

- The selected legacy directory is read-only: capture its complete bounded fingerprint before import and require the same fingerprint after success or failure.
- Vue receives only opaque candidate/import/entity IDs and summary data; absolute paths, database handles, encryption keys, and source filenames never cross the command boundary.
- Import limits remain `512` members, `100,000` metadata records, `64 MiB` per source asset, and `8 GiB` total source assets; truncated scans cannot be imported.
- A candidate expires after 30 minutes and is consumed by one successful import. Import always re-parses and revalidates the source immediately before staging.
- Existing current data is never overwritten. Legacy members create uniquely named profiles; duplicate image plaintext reuses an existing account asset.
- Pair groups create one problem from every `mistake` image plus every `answer` image sharing `pairId`; an unpaired mistake becomes an incomplete question-only problem; orphan answers are skipped and reported.
- Legacy `success` maps to `good`, `fail` maps to `again`, and every event stores `legacy-proficiency-v1` / `legacy-import-v1`. The old `nextTrainingDate` becomes the initial due time.
- A frozen legacy question is included in a per-profile `旧版冻结批次` export snapshot; generated DOCX/folders are never created during migration.
- All ordinary UI motion uses only `transform` and `opacity`, uses 120/180/240 ms tokens, and becomes immediate under `prefers-reduced-motion: reduce`.

---

### Task 1: Parse a bounded, importable legacy plan

**Files:**
- Modify: `src-tauri/src/modules/legacy.rs`
- Modify: `src-tauri/tests/legacy_scan.rs`
- Create: `src-tauri/tests/legacy_import_plan.rs`

**Interfaces:**
- Produces: `LegacyImportPlan`, `LegacyMemberPlan`, `LegacyProblemPlan`, `LegacyAssetPlan`, `LegacyReviewPlan`, `build_legacy_import_plan(root: &Path)`, and `legacy_tree_fingerprint(root: &Path)`.
- Consumes later: `LegacyImportManager::prepare` stores the plan source behind an opaque candidate ID; the importer consumes only the validated plan.

- [x] **Step 1: Write failing parser tests**

Create fixtures that use the real Electron fields `type`, `pairId`, `subject`, `tags`, `notes`, `answerTimeLimit`, `trainingRecords`, `nextTrainingDate`, and `isFrozen`. Assert that one question plus two answers becomes one ordered problem, unpaired questions remain importable, orphan answers become report issues, invalid dates are reported without panicking, and all source paths remain private.

```rust
let plan = build_legacy_import_plan(root).unwrap();
assert_eq!(plan.members[0].problems[0].question_assets.len(), 1);
assert_eq!(plan.members[0].problems[0].answer_assets.len(), 2);
assert_eq!(plan.members[0].problems[0].reviews[0].rating, LegacyRating::Good);
assert_eq!(plan.members[0].problems[0].due_at_utc_ms, Some(expected_due));
assert!(!serde_json::to_string(&plan.public_report()).unwrap().contains(root.to_string_lossy().as_ref()));
```

- [x] **Step 2: Run the focused parser tests and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_import_plan`

Expected: FAIL because the import-plan types and parser do not exist.

- [x] **Step 3: Expand strict legacy DTOs and build the plan**

Add `time = { version = "=0.3.53", default-features = false, features = ["parsing"] }`. Deserialize optional legacy values without accepting arbitrary nesting, parse ISO timestamps with `OffsetDateTime::parse(value, &Rfc3339)` and convert with `unix_timestamp_nanos() / 1_000_000`, group records deterministically by member then pair/order, and retain source `PathBuf` only in private plan types.

```rust
pub struct LegacyProblemPlan {
    pub source_problem_key: String,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
    pub time_limit_seconds: Option<i32>,
    pub question_assets: Vec<LegacyAssetPlan>,
    pub answer_assets: Vec<LegacyAssetPlan>,
    pub reviews: Vec<LegacyReviewPlan>,
    pub due_at_utc_ms: Option<i64>,
    pub stability_days: f64,
    pub difficulty: f64,
    pub frozen: bool,
}
```

- [x] **Step 4: Run scan and plan tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_scan --test legacy_import_plan`

Expected: PASS, including the existing path traversal, junction, oversize, corruption, and source-fingerprint cases.

- [x] **Step 5: Create a local checkpoint**

```bash
git add src-tauri/src/modules/legacy.rs src-tauri/tests/legacy_scan.rs src-tauri/tests/legacy_import_plan.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: plan bounded legacy imports"
```

### Task 2: Add the v10 import ledger and backup contract

**Files:**
- Create: `src-tauri/migrations/0010_legacy_import_ledger.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produces tables `legacy_imports` and `legacy_import_entities` and index `legacy_import_entities_import_idx`.
- Consumes later: atomic import writes one receipt plus every created/reused entity relation; rollback reads only rows owned by that import.

- [x] **Step 1: Write failing v9→v10 and backup tests**

Assert migration preserves every existing table/asset and creates:

```sql
CREATE TABLE legacy_imports (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT NOT NULL,
  source_fingerprint TEXT NOT NULL,
  member_count INTEGER NOT NULL,
  problem_count INTEGER NOT NULL,
  asset_count INTEGER NOT NULL,
  review_count INTEGER NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('completed','rolled_back')),
  created_at_utc_ms INTEGER NOT NULL,
  rolled_back_at_utc_ms INTEGER
) STRICT;
```

`legacy_import_entities` stores `import_id`, `entity_type`, `entity_id`, and `created_by_import` with a unique `(import_id, entity_type, entity_id)` key. Backup schema v10 must require both tables and the index while v1–v9 backups remain restorable and migrate on startup.

- [x] **Step 2: Run migration and backup tests and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test database_schema --test backup_store`

Expected: FAIL at schema version and missing ledger assertions.

- [x] **Step 3: Implement the additive migration and validation**

Wire starting versions 0–9 through migration 10, set `PRAGMA user_version = 10`, update current backup schema to 10, and validate exact ledger columns/index without weakening validation for older supported packages.

```rust
const CURRENT_SCHEMA_VERSION: i64 = 10;
const LEGACY_IMPORT_COLUMNS: &[&str] = &[
    "id", "account_id", "source_fingerprint", "member_count", "problem_count",
    "asset_count", "review_count", "status", "created_at_utc_ms", "rolled_back_at_utc_ms",
];
```

- [x] **Step 4: Run focused tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test database_schema --test backup_store`

Expected: PASS.

- [x] **Step 5: Create a local checkpoint**

```bash
git add src-tauri/migrations/0010_legacy_import_ledger.sql src-tauri/src/infrastructure/database.rs src-tauri/src/modules/backup.rs src-tauri/tests/database_schema.rs src-tauri/tests/backup_store.rs
git commit -m "feat: track reversible legacy imports"
```

### Task 3: Implement atomic encrypted import and verification

**Files:**
- Modify: `src-tauri/src/modules/legacy.rs`
- Create: `src-tauri/tests/legacy_import_store.rs`

**Interfaces:**
- Produces: `import_legacy_plan(connection, blob_root, key, account_id, plan, now, progress) -> LegacyImportReceipt`.
- Produces: `rollback_legacy_import(connection, blob_root, account_id, import_id, now) -> LegacyRollbackReceipt`.
- Consumes: v10 ledger, existing AES-GCM asset format, problem/review/export tables, and outbox schema.

- [x] **Step 1: Write failure-first store tests**

Cover normal multi-member import, multi-image pairs, question-only cards, plaintext SHA-256 deduplication, duplicate profile-name suffixes, metadata/tags/time limits, success/fail history mapping, old due dates, frozen snapshot membership, outbox creation, deterministic counts, source fingerprint before/after, second-use rejection, rollback, and injected failure after blob staging/move.

```rust
let before = legacy_tree_fingerprint(source).unwrap();
let receipt = import_legacy_plan(&mut db, assets, &key, account, plan, 500, |_| {}).unwrap();
assert_eq!(legacy_tree_fingerprint(source).unwrap(), before);
assert_eq!(receipt.problem_count, 2);
assert_eq!(db.query_row("SELECT COUNT(*) FROM review_events", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
```

- [x] **Step 2: Run the store test and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_import_store`

Expected: FAIL because import/rollback services are absent.

- [x] **Step 3: Implement staged assets and one SQL transaction**

Encrypt new plaintext into `<blob_root>/.legacy-import/<import-id>/`, validate image decoding and media type, insert/reuse account assets, create profiles/problems/problem-assets/review-events/schedules/frozen snapshots/outbox/ledger rows in one transaction, move staged blobs to final shard paths, verify the source fingerprint again, then commit. On any error, rollback SQL and remove every staging/final path created by this import.

```rust
pub fn import_legacy_plan(
    connection: &mut Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    plan: LegacyImportPlan,
    now_utc_ms: i64,
    mut progress: impl FnMut(LegacyImportProgress),
) -> Result<LegacyImportReceipt, LegacyImportError> {
    let import_id = Uuid::now_v7().to_string();
    let staging = blob_root.join(".legacy-import").join(&import_id);
    // prepare encrypted blobs, begin transaction, insert entities, move blobs,
    // re-fingerprint the source, commit, and compensate paths on every error branch.
    import_plan_transactionally(connection, blob_root, key, account_id, plan, now_utc_ms, &staging, &mut progress)
}
```

- [x] **Step 4: Implement ownership-safe rollback**

Refuse foreign/already-rolled-back imports. Delete only entities with `created_by_import = 1`; preserve reused assets and any imported entity that gained non-import references, returning it in `preserved_entity_count`. Remove only blob paths whose asset row was safely deleted, mark the receipt `rolled_back`, and write tombstone/outbox operations only when necessary for entities that could already have synced.

```rust
pub struct LegacyRollbackReceipt {
    pub import_id: String,
    pub removed_problem_count: i32,
    pub removed_profile_count: i32,
    pub removed_asset_count: i32,
    pub preserved_entity_count: i32,
    pub rolled_back_at_utc_ms: f64,
}
```

- [x] **Step 5: Run import store tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_import_store`

Expected: PASS with zero surviving rows/blobs after injected failures.

- [x] **Step 6: Create a local checkpoint**

```bash
git add src-tauri/src/modules/legacy.rs src-tauri/tests/legacy_import_store.rs
git commit -m "feat: import legacy libraries atomically"
```

### Task 4: Add opaque candidate management and typed commands

**Files:**
- Modify: `src-tauri/src/commands/legacy.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/shared/api/bindings.ts`
- Create: `src-tauri/tests/legacy_command.rs`
- Modify: `src-tauri/tests/command_contract.rs`
- Modify: `src-tauri/tests/bindings_contract.rs`

**Interfaces:**
- Produces commands `legacy_scan`, `legacy_import`, `legacy_import_list`, and `legacy_rollback` using `AppResult<T>`.
- Produces public types `LegacyImportCandidate`, `LegacyImportReceipt`, `LegacyImportSummary`, `LegacyRollbackReceipt`, and raw Tauri event payload `legacy_import_progress`.

- [x] **Step 1: Write failing command/security tests**

Assert candidate IDs are opaque UUIDv7 values, expire after 30 minutes, a second scan replaces the first, foreign import IDs return the same missing error, input JSON has no path/account/profile/key fields, and serialized errors never contain source paths, SQL, or image names.

- [x] **Step 2: Run command and binding tests and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_command --test command_contract --test bindings_contract`

- [x] **Step 3: Implement `LegacyImportManager` and commands**

```rust
pub struct LegacyImportManager {
    candidate: Mutex<Option<PreparedLegacyCandidate>>,
}

pub struct LegacyImportProgress {
    pub candidate_id: String,
    pub phase: LegacyImportPhase,
    pub completed: i32,
    pub total: i32,
}
```

Hold `runtime.lock_profile_transition()` through import/rollback, use transition-before-connection lock order, spawn blocking work, emit bounded progress without paths, consume a candidate only after success, and map cancellation to `Ok(None)`.

- [x] **Step 4: Register commands and regenerate bindings**

Run: `pnpm bindings:generate`

Expected: generated clients contain all four commands and no runtime identity/path inputs.

- [x] **Step 5: Run command/binding tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test legacy_command --test command_contract --test bindings_contract`

Expected: PASS.

- [x] **Step 6: Create a local checkpoint**

```bash
git add src-tauri/src/commands/legacy.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs src/shared/api/bindings.ts src-tauri/tests/legacy_command.rs src-tauri/tests/command_contract.rs src-tauri/tests/bindings_contract.rs
git commit -m "feat: expose safe legacy import commands"
```

### Task 5: Build the guided migration UI and restrained motion

**Files:**
- Create: `src/modules/legacy/components/LegacyImportPanel.vue`
- Create: `src/modules/legacy/components/LegacyImportDialog.vue`
- Create: `src/modules/legacy/components/LegacyImportResult.vue`
- Create: `src/modules/legacy/components/LegacyImportPanel.test.ts`
- Create: `src/modules/legacy/components/LegacyImportDialog.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `legacyScan`, `legacyImport`, `legacyImportList`, `legacyRollback`, and `legacy_import_progress`.
- Produces: a settings-page migration workflow with explicit source safety, confirmation, progress, result, and rollback states.

- [ ] **Step 1: Write failing UI state-machine tests**

Cover cancellation, safe candidate report, truncated/zero-problem import disabled, confirmation focus trap/Escape restore, import progress, success counts, failure preserving the candidate for retry, past receipt list, rollback confirmation, double-submit prevention, profile refresh notice, and reduced motion.

- [ ] **Step 2: Run focused Vue tests and verify failure**

Run: `pnpm exec vitest run src/modules/legacy/components/LegacyImportPanel.test.ts src/modules/legacy/components/LegacyImportDialog.test.ts src/app/views/SettingsView.test.ts`

- [ ] **Step 3: Implement the migration state machine**

Render five explicit states: `idle`, `candidate`, `confirming`, `importing`, and `completed`. Show member/problem/image/review/frozen counts and issues before confirmation; never show a local path. Require one checkbox reading `我确认：导入只复制数据，不会修改旧目录`, then focus the cancel action first. During import show phase plus a real determinate progress bar. After success show created profiles/problems/images/reviews and a `前往题库验收` action.

```ts
type LegacyUiState =
  | { kind: 'idle' }
  | { kind: 'candidate'; candidate: LegacyImportCandidate }
  | { kind: 'confirming'; candidate: LegacyImportCandidate }
  | { kind: 'importing'; candidate: LegacyImportCandidate; progress: LegacyImportProgress }
  | { kind: 'completed'; receipt: LegacyImportReceipt }
```

- [ ] **Step 4: Implement rollback and animation**

Past completed imports show an overflow action `撤销这次导入`; confirmation explains preserved reused/modified data. Progress fill, result seal, and dialog sheet animate only `transform`/`opacity` for 180/240 ms; reduced motion renders the final state immediately. Every target is at least 44×44 px.

```css
.legacy-progress__fill { transform: scaleX(var(--progress)); transform-origin: left; transition: transform var(--motion-standard) var(--ease-standard); }
.legacy-result-enter-active,.legacy-dialog-enter-active { transition: opacity var(--motion-page) var(--ease-standard),transform var(--motion-page) var(--ease-standard); }
@media (prefers-reduced-motion: reduce) { .legacy-progress__fill,.legacy-result-enter-active,.legacy-dialog-enter-active { transition: none; } }
```

- [ ] **Step 5: Run focused UI tests**

Run: `pnpm exec vitest run src/modules/legacy/components/LegacyImportPanel.test.ts src/modules/legacy/components/LegacyImportDialog.test.ts src/app/views/SettingsView.test.ts`

Expected: PASS.

- [ ] **Step 6: Create a local checkpoint**

```bash
git add src/modules/legacy src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: guide legacy data migration"
```

### Task 6: Document, audit, and verify the complete slice

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/library.md`
- Create: `docs/windows-legacy-import-acceptance.md`
- Modify: `docs/superpowers/plans/2026-07-20-atomic-legacy-import.md`

**Interfaces:**
- Produces: release evidence for source immutability, atomic rollback, accessibility, performance, and final local Git checkpoint.

- [ ] **Step 1: Document migration boundaries and Windows acceptance**

Document candidate expiry, source fingerprinting, pair mapping, incomplete/orphan rules, legacy rating/version mapping, frozen snapshot mapping, ownership-safe rollback, and path/error redaction. Acceptance covers normal, duplicate, missing, corrupt, oversized, symlink/junction, interrupted, retry, rollback, 512-member/100k-record limit, and source tree hash equality.

- [ ] **Step 2: Run all quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

Expected: all pass; initial JS remains below 300 KB gzip and the settings/legacy lazy work remains below 120 KB gzip.

- [ ] **Step 3: Perform browser and real-Tauri QA**

Use a real copy of the old Electron fixture. Verify 1280×820 and 390×844 layouts, keyboard-only confirmation/rollback, focus restoration, reduced motion, no horizontal overflow, progress accuracy, library visibility after import, app restart persistence, and unchanged source fingerprint.

- [ ] **Step 4: Request independent review and fix every Critical/Important finding**

Review scope: path containment, source immutability, transaction/filesystem compensation, cross-account isolation, dedup ownership, rollback safety, date/rating mapping, DTO secrecy, stale async UI responses, focus behavior, and motion constraints. Rerun affected focused suites after fixes.

- [ ] **Step 5: Mark the plan and create the final local checkpoint**

```bash
git add docs/architecture.md docs/plans/library.md docs/windows-legacy-import-acceptance.md docs/superpowers/plans/2026-07-20-atomic-legacy-import.md
git commit -m "docs: verify atomic legacy migration"
git status --short
```

Expected: clean worktree. Do not push without explicit authorization for the resulting SHA.
