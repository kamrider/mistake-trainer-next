# Sync Conflict Center Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make concurrent edits converge safely by three-way merging different fields, recording only true same-field conflicts, and giving users an auditable conflict center in Settings.

**Architecture:** Persist the last canonical remote payload for each mutable synchronized entity in `sync_entity_snapshots`. During pull, compare base snapshot, current local entity, and incoming remote entity field-by-field: remote-only changes apply locally, local-only changes are re-enqueued above the remote revision, equal concurrent changes converge, and differing concurrent changes become `sync_conflicts`. While an entity has open conflicts its ordinary editor is locked; resolving the final field creates one new canonical revision only when the chosen result differs from the remote snapshot.

**Tech Stack:** Rust, rusqlite/SQLCipher, serde_json, Tauri Specta typed commands, Vue 3, Vitest/Testing Library, existing Supabase revision/change-sequence contract.

## Global Constraints

- Review events remain append-only union data and never enter the conflict center.
- Assets remain immutable and deduplicate by plaintext SHA-256; asset bytes never enter snapshots or conflicts.
- Mutable entities covered in this increment are `learner_profile`, `problem`, and `export_snapshot`.
- Problem merge fields are exactly `subject`, `tags`, `note`, `status`, `timeLimitSeconds`, and `assets`; timestamps and revisions are merge metadata, not user choices.
- A missing base snapshot with unequal local and remote values is treated conservatively as a conflict, never as proof that either side is authoritative.
- Pull cursor advancement, snapshot write, local merge, conflict write, and outbox enqueue happen in one SQL transaction.
- One open conflict per account/entity/field is enforced by a partial unique index.
- Ordinary profile/problem editing is rejected while that entity has unresolved conflicts.
- Conflict commands take opaque IDs and are scoped to the current account and active profile; foreign IDs return the same not-found result.
- A resolved conflict is retained for audit with `resolution`, `resolved_value_json`, and `resolved_at_utc_ms`; it is not deleted by the UI.
- All source values are validated with the same length/range/status rules as ordinary editing before they are written.
- Backup schema v13 must include snapshots, added conflict columns, and their indexes; v12 backups remain restorable and migrate forward.
- Motion uses transform/opacity only, follows existing 120/180/240 ms tokens, and is removed under `prefers-reduced-motion`.

## File Map

- Create: `src-tauri/migrations/0013_sync_merge_state.sql` — snapshots, conflict audit columns, and indexes.
- Modify: `src-tauri/src/infrastructure/database.rs` — migrate v12 to v13.
- Modify: `src-tauri/src/modules/backup.rs` — validate v13 backup structure and account boundaries.
- Create: `src-tauri/src/modules/sync_conflicts.rs` — canonical local payloads, three-way merge, list, resolve, and edit guards.
- Modify: `src-tauri/src/modules/sync_pull.rs` — route mutable entities through three-way merge and clean snapshots on tombstones.
- Modify: `src-tauri/src/modules/problems.rs` — reject edits/status changes with open conflicts.
- Modify: `src-tauri/src/modules/profiles.rs` — reject rename with open conflicts and clean snapshots on deletion.
- Modify: `src-tauri/src/modules/mod.rs` — export the new module.
- Modify: `src-tauri/src/commands/sync.rs` — add list/resolve typed commands.
- Modify: `src-tauri/src/bindings.rs` and `src-tauri/src/lib.rs` — register commands and regenerate TS.
- Create: `src-tauri/tests/sync_conflicts.rs` — merge and resolution integration tests.
- Modify: `src-tauri/tests/sync_pull.rs` — snapshot, auto-merge, conflict, and tombstone cases.
- Modify: `src-tauri/tests/database_schema.rs` and `src-tauri/tests/backup_store.rs` — v13 migration/backup gates.
- Create: `src/modules/sync/components/SyncConflictCenter.vue` — user-facing conflict cards and actions.
- Create: `src/modules/sync/components/SyncConflictCenter.test.ts` — interaction/accessibility tests.
- Modify: `src/app/views/SettingsView.vue` and `src/app/views/SettingsView.test.ts` — load and refresh the center.
- Modify: `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md` — reserve migration 0014 for future question suggestions.

---

### Task 1: Add v13 merge-state storage and backup validation

**Files:**
- Create: `src-tauri/migrations/0013_sync_merge_state.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produces `sync_entity_snapshots(account_id, profile_id, entity_type, entity_id, revision, payload_json, updated_at_utc_ms)` and audited conflict resolution columns.

- [ ] **Step 1: Write failing migration tests**

```rust
#[test]
fn v12_migrates_to_v13_without_changing_existing_conflicts() {
    let mut db = v12_database_with_open_conflict();
    run_migrations(&mut db).unwrap();
    assert_eq!(user_version(&db), 13);
    assert_eq!(scalar(&db, "SELECT count(*) FROM sync_conflicts"), 1);
    assert_eq!(scalar(&db, "SELECT count(*) FROM sync_entity_snapshots"), 0);
    assert_eq!(column_names(&db, "sync_conflicts"), vec![
        "id", "account_id", "profile_id", "entity_type", "entity_id", "field_name",
        "local_value_json", "remote_value_json", "base_revision", "created_at_utc_ms",
        "resolved_at_utc_ms", "resolution", "resolved_value_json",
    ]);
}
```

Also assert the partial unique index rejects a second unresolved row for the same account/entity/field but permits a new row after the old one is resolved.

- [ ] **Step 2: Run the schema tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema`

Expected: FAIL because schema 13 and `sync_entity_snapshots` do not exist.

- [ ] **Step 3: Add the migration**

```sql
CREATE TABLE sync_entity_snapshots (
    account_id TEXT NOT NULL,
    profile_id TEXT,
    entity_type TEXT NOT NULL CHECK(entity_type IN ('learner_profile', 'problem', 'export_snapshot')),
    entity_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json) AND json_type(payload_json) = 'object'),
    updated_at_utc_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id, entity_type, entity_id)
) STRICT;

CREATE INDEX sync_entity_snapshots_profile_idx
ON sync_entity_snapshots(account_id, profile_id, entity_type, entity_id);

ALTER TABLE sync_conflicts ADD COLUMN resolution TEXT
    CHECK(resolution IS NULL OR resolution IN ('local', 'remote'));
ALTER TABLE sync_conflicts ADD COLUMN resolved_value_json TEXT
    CHECK(resolved_value_json IS NULL OR json_valid(resolved_value_json));

CREATE UNIQUE INDEX sync_conflicts_open_field_idx
ON sync_conflicts(account_id, entity_type, entity_id, field_name)
WHERE resolved_at_utc_ms IS NULL;
```

Update `run_migrations` to accept at most 13, apply the file only from version 12, and atomically set `user_version = 13`.

- [ ] **Step 4: Extend backup validation**

For schema 13 require the exact snapshot columns/index, the two new conflict columns/index, and reject snapshots whose `account_id` differs from the backup account. For schema <=12 reject the presence of any v13-only artifact.

- [ ] **Step 5: Run schema and backup tests**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: all tests PASS.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/migrations src-tauri/src/infrastructure/database.rs src-tauri/src/modules/backup.rs src-tauri/tests/database_schema.rs src-tauri/tests/backup_store.rs
git commit -m "feat: persist synchronized entity bases"
```

### Task 2: Implement deterministic three-way merge during pull

**Files:**
- Create: `src-tauri/src/modules/sync_conflicts.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`
- Modify: `src-tauri/tests/sync_pull.rs`

**Interfaces:**
- Consumes: `WireProfile`, `WireProblemAggregate`, and `WireExportSnapshot`.
- Produces:

```rust
pub(crate) enum MergeAction<T> {
    ApplyRemote(T),
    ApplyMergedAndEnqueue(T),
    ApplyPartialWithConflicts { value: T, conflicts: Vec<FieldConflict> },
}

pub(crate) struct FieldConflict {
    pub field_name: &'static str,
    pub local_value: serde_json::Value,
    pub remote_value: serde_json::Value,
    pub base_revision: i64,
}

pub(crate) fn merge_remote_problem(
    tx: &Transaction<'_>, account_id: &str, remote: &WireProblemAggregate, now_utc_ms: i64,
) -> Result<MergeAction<WireProblemAggregate>, SyncConflictError>;
```

Equivalent functions exist for profile and export snapshot.

- [ ] **Step 1: Write pure three-way merge tests**

```rust
#[test]
fn different_fields_auto_merge_and_advance_above_remote_revision() {
    let base = problem(1, "数学", "旧笔记", vec!["函数"]);
    let local = problem(2, "物理", "旧笔记", vec!["函数"]);
    let remote = problem(2, "数学", "云端笔记", vec!["函数"]);
    let action = merge_problem_values(&base, &local, &remote).unwrap();
    let merged = require_reenqueue(action);
    assert_eq!(merged.subject, "物理");
    assert_eq!(merged.note, "云端笔记");
    assert_eq!(merged.revision, 3);
}

#[test]
fn same_field_different_values_creates_one_conflict() {
    let action = merge_problem_values(
        &problem(1, "数学", "旧", vec![]),
        &problem(2, "数学", "本机", vec![]),
        &problem(2, "数学", "云端", vec![]),
    ).unwrap();
    assert_eq!(conflict_fields(action), ["note"]);
}
```

Cover equal concurrent values, tags as a canonical ordered array, nullable time limit, assets as one atomic ordered field, no-base unequal values, remote-only changes, local-only changes, invalid status, and duplicate incoming changes.

- [ ] **Step 2: Run focused sync pull tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull`

Expected: FAIL because snapshots and merge functions are absent.

- [ ] **Step 3: Implement the field rule once**

```rust
fn merge_field(base: &Value, local: &Value, remote: &Value) -> FieldDecision {
    match (local == base, remote == base, local == remote) {
        (_, _, true) => FieldDecision::Value(remote.clone()),
        (true, false, false) => FieldDecision::Value(remote.clone()),
        (false, true, false) => FieldDecision::Value(local.clone()),
        (false, false, false) => FieldDecision::Conflict {
            local: local.clone(),
            remote: remote.clone(),
        },
        (true, true, false) => unreachable!("values equal base but differ from each other"),
    }
}
```

If no base exists and local differs from remote, synthesize conflicts for every differing merge field. Never compare `updatedAtUtcMs`, `createdAtUtcMs`, or `revision` as user content.

- [ ] **Step 4: Integrate the transaction**

For each mutable remote entity: load the snapshot and canonical local wire value; calculate the action; upsert the received remote payload into `sync_entity_snapshots`; apply the chosen/partial value locally; insert conflicts with `INSERT ... ON CONFLICT DO UPDATE` against the open-field index; enqueue exactly one canonical upsert when the merged value contains local-only changes and has no conflict. Do not enqueue while any conflict is open.

- [ ] **Step 5: Handle tombstones**

If a remote tombstone arrives and local equals its last snapshot, apply it normally and delete the snapshot. If local differs from the snapshot, create an `__deleted__` conflict whose remote value is `true`, keep the local entity, update the remote snapshot to the tombstone metadata, and prevent its stale outbox from pushing until resolution.

- [ ] **Step 6: Run sync pull tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull`

Expected: all tests PASS, including cursor rollback on any merge/snapshot/conflict write failure.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/modules src-tauri/tests/sync_pull.rs
git commit -m "feat: three-way merge synchronized edits"
```

### Task 3: Add account-scoped conflict list and atomic resolution

**Files:**
- Modify: `src-tauri/src/modules/sync_conflicts.rs`
- Modify: `src-tauri/src/modules/problems.rs`
- Modify: `src-tauri/src/modules/profiles.rs`
- Modify: `src-tauri/src/commands/sync.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/sync_conflicts.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictSummary {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub entity_label: String,
    pub field_name: String,
    pub local_value: serde_json::Value,
    pub remote_value: serde_json::Value,
    pub created_at_utc_ms: i64,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSyncConflictsInput {
    pub entity_type: String,
    pub entity_id: String,
    pub choice: SyncConflictChoice,
}

pub fn sync_conflict_list() -> AppResult<Vec<SyncConflictSummary>>;
pub fn sync_conflict_resolve(input: ResolveSyncConflictFieldInput) -> AppResult<Vec<SyncConflictSummary>>;
pub fn sync_conflict_resolve_entity(input: ResolveSyncConflictsInput) -> AppResult<Vec<SyncConflictSummary>>;
```

- [ ] **Step 1: Write failing resolution tests**

Test single local choice, single remote choice, atomic all-local/all-remote, mixed per-field choices, deletion keep/delete, invalid JSON type, duplicate resolution, foreign account/profile IDs, stale conflict IDs, final revision/enqueued payload, and no outbox when the resolved entity equals the remote snapshot.

- [ ] **Step 2: Run the new integration test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_conflicts`

Expected: FAIL because list/resolve use cases do not exist.

- [ ] **Step 3: Implement atomic resolution**

Resolve selected fields inside one transaction, validate each chosen value, update conflict audit columns, and check remaining open rows for the entity. On the final resolution, compare the complete local wire value with the saved remote snapshot: if different, set revision to `max(local.revision, snapshot.revision) + 1` and enqueue one upsert; if equal, set revision to the snapshot revision and enqueue nothing.

- [ ] **Step 4: Guard ordinary editing**

At the start of `update_problem`, `change_problem_status`, and `rename_profile`, query for an open conflict matching account/entity. Return a stable `ConflictPending` error and user message directing the user to Settings. This prevents an ordinary edit from accidentally pushing unresolved local values.

- [ ] **Step 5: Register and regenerate bindings**

Run: `pnpm bindings:generate`

Expected generated commands: `syncConflictList`, `syncConflictResolve`, and `syncConflictResolveEntity`; generated values use `JsonValue`, never `any`.

- [ ] **Step 6: Run Rust and binding tests**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_conflicts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri src/shared/api/bindings.ts
git commit -m "feat: resolve true sync conflicts safely"
```

### Task 4: Build the animated Settings conflict center

**Files:**
- Create: `src/modules/sync/components/SyncConflictCenter.vue`
- Create: `src/modules/sync/components/SyncConflictCenter.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes generated commands from Task 3.
- Emits `changed` after a successful resolution so Settings reloads overview counts.

- [ ] **Step 1: Write failing component tests**

```ts
it('shows both values and resolves all fields atomically', async () => {
  api.syncConflictList.mockResolvedValue({ ok: true, data: [noteConflict, subjectConflict] })
  render(SyncConflictCenter)
  expect(await screen.findByText('本机版本')).toBeVisible()
  expect(screen.getByText('云端版本')).toBeVisible()
  await user.click(screen.getByRole('button', { name: '这道题全部采用本机版本' }))
  expect(api.syncConflictResolveEntity).toHaveBeenCalledWith({
    entityType: 'problem', entityId: 'problem-1', choice: 'local',
  })
})
```

Also test per-field choices, JSON arrays/tags, empty/null values, error retention, busy disabling, focus after card removal, no-conflict state, reduced motion, and entity labels without exposing opaque IDs.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `node node_modules\vitest\vitest.mjs run src/modules/sync/components/SyncConflictCenter.test.ts src/app/views/SettingsView.test.ts`

Expected: FAIL because the component and command mocks are absent.

- [ ] **Step 3: Implement the UI**

Group rows by entity. Each paper card shows subject/profile/export label, field label, “本机版本” and “云端版本” columns, and two explicit choice buttons. Provide card-level “全部采用本机” and “全部采用云端”; never default-select a destructive choice. Resolve success collapses the card with 180 ms translate/opacity motion and moves focus to the next card or the resolved status message.

- [ ] **Step 4: Integrate Settings**

Place the center immediately below the four overview cards when `unresolvedConflictCount > 0`, and show a compact “同步内容没有待处理冲突” state when connected and the count is zero. Remove the roadmap-only conflict-center claim because the feature is now real. Reload both the conflict list and `settingsOverview` after resolution or manual sync.

- [ ] **Step 5: Run frontend quality gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Expected: all PASS; initial JS remains below 300 KB gzip.

- [ ] **Step 6: Commit**

```powershell
git add src/modules/sync src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: add the sync conflict center"
```

### Task 5: Complete two-device acceptance and documentation

**Files:**
- Modify: `docs/architecture/sync-conflicts.md` if the directory exists; otherwise create `docs/sync-conflicts.md`.
- Modify: `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md`.

**Interfaces:**
- Documents the exact merge truth table, recovery path, audit fields, and migration numbering.

- [ ] **Step 1: Run deterministic two-database scenarios**

Exercise: different-field problem edits auto-merge; same-note edits create one conflict; identical edits converge; profile rename conflict; remote delete versus local note edit; conflict resolved local then pulled on the other device; conflict resolved remote with no extra push; long offline chain; replayed pull page; and application restart with open conflicts.

- [ ] **Step 2: Verify invariants directly in both databases**

Assert equal final canonical values/revisions after the final sync, no duplicate open conflicts, no stuck `processing` operation, monotonic pull cursors, preserved review-event union, unchanged asset hashes, and retained audit rows.

- [ ] **Step 3: Run final gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
```

Expected: every command PASS.

- [ ] **Step 4: Reserve the next migration number**

Change the future automatic-question-region plan from `0013_capture_region_suggestions.sql` to `0014_capture_region_suggestions.sql` so two plans cannot claim the same schema version.

- [ ] **Step 5: Commit**

```powershell
git add docs
git commit -m "docs: record sync conflict convergence rules"
```
