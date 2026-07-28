# Smart Question Organizing Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking. Do not enable the production entry until the existing
> 60/300-image evidence gate passes.

**Goal:** Add a local “智能分题” flow to the capture inbox that analyzes only eligible
unassigned images, presents conservative question/answer and crop suggestions for fast
human review, and applies or reverts accepted suggestions without touching existing
manual organization.

**Architecture:** Recognition is a resumable, account/profile-scoped suggestion job,
not a mutation of the capture batch. A single bounded worker reads an encrypted asset
through a private temporary file, returns normalized crop/group proposals, and deletes
the temporary file; the Vue workbench reviews those proposals inline. Only
`capture_recognition_apply` changes capture items and drafts, in one transaction after
all derived blobs have been staged, and its operation can be reverted while the
generated entities have not been edited or committed.

**Tech Stack:** Vue 3, TypeScript, Vitest, Tauri 2, Rust, rusqlite/SQLCipher, existing
AES-GCM asset storage and crop recipes, existing optional PP-OCRv6 small component,
existing `capture_batch_changed` event pattern.

## Global Constraints

- This plan consumes the question-region engine and evidence gates defined in
  `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md`.
- Do not expose the production entry until question-start recall is at least `95%`,
  content-cut rate is below `0.5%`, false-split rate is below `3%`, and p95 warm
  one-image latency is below `2 s` on the 4-core/8 GB Windows reference machine.
- This slice performs question organization only. It does not persist recognized text,
  generate Markdown/LaTeX, erase handwriting, infer subjects, or change notes/tags.
  Content extraction remains in
  `docs/superpowers/plans/2026-07-24-verified-question-content-pipeline.md`.
- Recognition is local-only after the explicitly installed model is available. Starting
  or opening a capture batch must not download a model or contact a third party.
- Keep the encrypted color source canonical. Recognition may create ordinary crop
  derivations through the existing asset pipeline but must not replace or delete the
  source.
- Recognition never auto-applies. The user must explicitly accept suggestions and then
  choose `应用已接受建议`.
- Only unassigned, active capture items from an `organizing` batch are eligible.
  Existing drafts and manually assigned items are outside the job scope.
- New uploads and manual changes made after a job starts are never overwritten. The
  affected suggestion becomes `stale`; unrelated suggestions remain reviewable.
- Use confidence bands `high` for `>= 9000`, `review` for `6500..=8999`, and `low` for
  `< 6500` basis points. Bulk accept is available only for `high`; `low` stays skipped.
- Question/answer pairing is proposed only when both sides have a reliable matching
  anchor. The persisted group slot is an opaque integer scoped to the job; question
  numbers and OCR text are not stored.
- One inference worker processes one image at a time. Cancellation is checked between
  stages and images. The user can continue manual work; item snapshot validation handles
  races.
- A model/runtime failure cannot block templates, dragging, cropping, draft editing, or
  formal commit.
- Recognition jobs, suggestions, and unapplied review decisions remain local and do not
  enter the sync outbox. Backups include their database rows and referenced capture
  assets, but never model caches or decrypted temporary files.
- `capture_commit_ready` readiness remains exactly: question image, answer image, and
  batch or draft subject. Recognition confidence is not a readiness requirement.
- Vue submits only batch, job, suggestion, item, operation, and draft opaque IDs plus
  normalized crop recipes. Paths, asset keys, model paths, OCR tokens, and database
  handles never enter page state.
- Progress announcements are throttled to one accessible update per completed image.
  All actions have keyboard equivalents and respect reduced motion.
- No frontend main-thread recognition or full-resolution image decode. The existing
  320-pixel preview cache remains bounded to 40 images.

## Product Flow

1. After collection ends, the existing sequence templates remain visible. A new
   `智能分题（本机）` card appears above them.
2. If recognition is ready, the card says exactly how many unassigned images will be
   analyzed and opens a lightweight preflight sheet. If the model is missing, the card
   offers `去设置下载模型` and preserves a `继续用顺序模板` action. If the evidence gate
   is closed, it explains that the feature is still under verification and exposes no
   dead start button.
3. Starting a job immediately returns to the workbench. A compact progress strip shows
   `正在分析 12 / 50`, elapsed time, and `停止`; manual organization stays available.
4. Results open as an inline review queue, not a blocking modal. The summary separates
   `可快速确认`, `需要检查`, `未给出建议`, and `已过期`.
5. Each review row shows the source, proposed regions, proposed question/answer role,
   and a short reason such as `检测到清晰题号` or `题答编号匹配`. It never displays raw OCR
   text as trusted content.
6. The user can accept, edit regions in the existing crop editor, skip, or bulk-accept
   only high-confidence suggestions. `J/K` moves, `Enter` accepts, `E` edits, and `S`
   skips while focus remains inside the review surface.
7. `应用已接受建议` presents an exact impact summary, then atomically creates crop
   derivations and draft assignments. Unmatched answer regions return to the unassigned
   strip as `答案`.
8. A persistent success notice offers `撤销本次智能分题`. Undo restores the scoped sources
   and removes only still-unmodified entities created by that operation.

## File Map

- Create: `src-tauri/migrations/0014_capture_recognition_jobs.sql` — local job,
  suggestion, and reversible-operation ledger.
- Modify: `src-tauri/src/infrastructure/database.rs` — migrate v13 to v14.
- Create: `src-tauri/src/modules/capture_recognition.rs` — job state machine,
  snapshot validation, review decisions, atomic apply, and revert.
- Create: `src-tauri/src/infrastructure/capture_recognition_worker.rs` — bounded worker
  and replaceable evidence-gated engine contract.
- Create: `src-tauri/src/commands/capture_recognition.rs` — typed AppResult commands and
  stable user errors.
- Modify: `src-tauri/src/modules/capture_inbox.rs` — extract reusable staged crop
  preparation and transaction helpers without changing current crop behavior.
- Modify: `src-tauri/src/modules/mod.rs`,
  `src-tauri/src/infrastructure/mod.rs`, `src-tauri/src/commands/mod.rs`,
  `src-tauri/src/bindings.rs`, and `src-tauri/src/lib.rs` — register module, manager,
  commands, types, and events.
- Modify: `src-tauri/src/modules/ocr_capability.rs` — expose a typed recognition feature
  state without enabling it merely because a model exists.
- Test: `src-tauri/tests/capture_recognition.rs` — migration, ownership, job, apply,
  cancellation, restart, staleness, and revert coverage.
- Modify: `src-tauri/tests/database_schema.rs`,
  `src-tauri/src/modules/backup.rs`, and `src-tauri/tests/backup_store.rs` — v14 and
  backup validation.
- Create: `src/modules/ocr/domain/recognitionFlow.ts` — pure UI action/state selection.
- Create: `src/modules/ocr/domain/recognitionFlow.test.ts` — deterministic state tests.
- Create: `src/modules/ocr/components/CaptureRecognitionEntry.vue` — availability,
  preflight, and progress surface.
- Create: `src/modules/ocr/components/CaptureRecognitionEntry.test.ts` — entry and
  progress accessibility tests.
- Create: `src/modules/ocr/components/CaptureRecognitionReview.vue` — inline review
  queue and impact confirmation.
- Create: `src/modules/ocr/components/CaptureRecognitionReview.test.ts` — confidence,
  keyboard, stale, and bulk-action tests.
- Modify: `src/modules/capture/components/CaptureCropEditor.vue` and its test — proposal
  editing mode.
- Modify: `src/modules/capture/components/CaptureWorkspace.vue` and its test — place
  recognition above templates while preserving manual controls.
- Modify: `src/app/views/CaptureView.vue` and its test — command/event orchestration and
  route-preserved batch state.
- Modify: `src/app/views/SettingsView.vue`,
  `src/app/views/SettingsView.test.ts`, and `src/app/router.ts` — setup return path.
- Regenerate: `src/shared/api/bindings.ts` — typed public interface.
- Create: `docs/windows-smart-question-organizing-acceptance.md` — Windows, privacy,
  accessibility, and performance acceptance.
- Modify: `docs/architecture.md` and `docs/plans/library.md` — delivered boundary and
  remaining content-recognition roadmap.

---

### Task 1: Define the availability and UI state contract

**Files:**
- Modify: `src-tauri/src/modules/ocr_capability.rs`
- Test: `src-tauri/tests/ocr_capability_command.rs`
- Create: `src/modules/ocr/domain/recognitionFlow.ts`
- Create: `src/modules/ocr/domain/recognitionFlow.test.ts`

**Interfaces:**
- Consumes: existing `OcrCapabilityStatus`, batch state, unassigned count, and active
  recognition job state.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OcrRecognitionFeatureState {
    EvidenceGatePending,
    ModelMissing,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognitionFeatureStatus {
    pub state: OcrRecognitionFeatureState,
    pub required_component_id: OcrComponentId,
    pub detail: String,
}
```

```ts
export type RecognitionPrimaryAction =
  | 'hidden'
  | 'explain_gate'
  | 'open_setup'
  | 'start'
  | 'resume'

export function recognitionPrimaryAction(input: {
  batchState: CaptureBatchState
  unassignedCount: number
  featureState: OcrRecognitionFeatureState
  activeJobState?: CaptureRecognitionJobState
}): RecognitionPrimaryAction
```

- [ ] **Step 1: Write failing Rust feature-state tests**

Add tests proving:

```rust
assert_eq!(
    recognition_feature_status(true, OcrComponentState::Installed).state,
    OcrRecognitionFeatureState::Ready,
);
assert_eq!(
    recognition_feature_status(true, OcrComponentState::NotInstalled).state,
    OcrRecognitionFeatureState::ModelMissing,
);
assert_eq!(
    recognition_feature_status(false, OcrComponentState::Installed).state,
    OcrRecognitionFeatureState::EvidenceGatePending,
);
```

The evidence-gate boolean is a build-time product constant. Installing a model must not
change it.

- [ ] **Step 2: Run the focused Rust tests and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test ocr_capability_command
```

Expected: FAIL because `OcrRecognitionFeatureState` and
`recognition_feature_status` do not exist.

- [ ] **Step 3: Add the typed feature status**

Add `recognition_feature` to `OcrCapabilityStatus`. Use these exact user details:

```rust
const DETAIL_GATE: &str = "智能分题仍在真实题图验证中；顺序模板和手工整理可继续使用。";
const DETAIL_MODEL: &str = "智能分题需要已校验的 PP‑OCRv6 small 本地模型。";
const DETAIL_READY: &str = "智能分题可在本机运行；结果只会作为待确认建议。";
```

Keep `automatic_recognition_enabled` for binding compatibility and leave it `false`:
this flow is explicitly started and confirmed by the user, so it is assisted
recognition rather than automatic recognition. The new typed feature state controls the
entry.

- [ ] **Step 4: Write and implement the pure TypeScript action matrix**

Test this complete matrix:

```ts
expect(action({ batchState: 'collecting', unassignedCount: 4, featureState: 'ready' }))
  .toBe('hidden')
expect(action({ batchState: 'organizing', unassignedCount: 0, featureState: 'ready' }))
  .toBe('hidden')
expect(action({ batchState: 'organizing', unassignedCount: 4, featureState: 'evidence_gate_pending' }))
  .toBe('explain_gate')
expect(action({ batchState: 'organizing', unassignedCount: 4, featureState: 'model_missing' }))
  .toBe('open_setup')
expect(action({ batchState: 'organizing', unassignedCount: 4, featureState: 'ready' }))
  .toBe('start')
expect(action({
  batchState: 'organizing',
  unassignedCount: 4,
  featureState: 'ready',
  activeJobState: 'review',
})).toBe('resume')
expect(action({
  batchState: 'organizing',
  unassignedCount: 0,
  featureState: 'ready',
  activeJobState: 'review',
})).toBe('resume')
```

Resolve `queued`, `running`, and `review` to `resume` before checking the current
unassigned count. Resolve `applied`, `cancelled`, and `failed` as inactive terminal
states.

Run: `pnpm test -- src/modules/ocr/domain/recognitionFlow.test.ts`

Expected: PASS after the pure function is implemented without Vue or Tauri imports.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/modules/ocr_capability.rs `
  src-tauri/tests/ocr_capability_command.rs `
  src/modules/ocr/domain/recognitionFlow.ts `
  src/modules/ocr/domain/recognitionFlow.test.ts
git commit -m "feat: define smart organizing availability"
```

### Task 2: Persist resumable recognition jobs and item snapshots

**Files:**
- Create: `src-tauri/migrations/0014_capture_recognition_jobs.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Create: `src-tauri/src/modules/capture_recognition.rs`
- Test: `src-tauri/tests/capture_recognition.rs`

**Interfaces:**
- Consumes: account ID, profile ID, organizing batch ID, and ordered unassigned item IDs.
- Produces:

```rust
pub enum CaptureRecognitionJobState {
    Queued,
    Running,
    Review,
    Applied,
    Cancelled,
    Failed,
}

pub enum CaptureRecognitionReviewBand {
    High,
    Review,
    Low,
}

pub enum CaptureRecognitionSuggestionState {
    Proposed,
    Accepted,
    Rejected,
    Stale,
}

pub struct CaptureRecognitionRegionProposal {
    pub rect: NormalizedCropRect,
    pub role: CaptureRecognitionRole,
    pub group_slot: Option<u32>,
    pub confidence_basis_points: u16,
}

pub struct CaptureRecognitionSuggestion {
    pub id: String,
    pub item_id: String,
    pub regions: Vec<CaptureRecognitionRegionProposal>,
    pub confidence_basis_points: u16,
    pub review_band: CaptureRecognitionReviewBand,
    pub state: CaptureRecognitionSuggestionState,
    pub reason_codes: Vec<CaptureRecognitionReasonCode>,
}

pub struct CaptureRecognitionJob {
    pub id: String,
    pub batch_id: String,
    pub state: CaptureRecognitionJobState,
    pub total_items: u32,
    pub processed_items: u32,
    pub suggestions: Vec<CaptureRecognitionSuggestion>,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
}
```

- [ ] **Step 1: Write the v14 migration failure test**

Assert that migrating a v13 fixture creates all four tables and keeps existing problem,
asset, capture, crop-derivation, and sync-merge rows byte-for-byte unchanged:

```rust
for table in [
    "capture_recognition_jobs",
    "capture_recognition_job_items",
    "capture_recognition_suggestions",
    "capture_recognition_operations",
] {
    assert!(table_exists(&connection, table));
}
assert_eq!(schema_version(&connection), 14);
```

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test database_schema
```

Expected: FAIL because schema v14 is absent.

- [ ] **Step 2: Add the strict schema**

Use this shape:

```sql
CREATE TABLE capture_recognition_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN
      ('queued','running','review','applied','cancelled','failed')),
    engine TEXT NOT NULL CHECK(length(engine) BETWEEN 1 AND 60),
    engine_version TEXT NOT NULL CHECK(length(engine_version) BETWEEN 1 AND 60),
    model_component_id TEXT NOT NULL CHECK(model_component_id = 'ppocrv6_small'),
    total_items INTEGER NOT NULL CHECK(total_items BETWEEN 1 AND 150),
    processed_items INTEGER NOT NULL DEFAULT 0
      CHECK(processed_items BETWEEN 0 AND total_items),
    failure_code TEXT,
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE capture_recognition_job_items (
    job_id TEXT NOT NULL REFERENCES capture_recognition_jobs(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    source_snapshot_hash BLOB NOT NULL CHECK(length(source_snapshot_hash) = 32),
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 149),
    state TEXT NOT NULL CHECK(state IN
      ('pending','running','complete','no_suggestion','stale','failed')),
    PRIMARY KEY(job_id, item_id),
    UNIQUE(job_id, position)
) STRICT;

CREATE TABLE capture_recognition_suggestions (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES capture_recognition_jobs(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    regions_json TEXT NOT NULL CHECK(json_valid(regions_json)),
    confidence_basis_points INTEGER NOT NULL
      CHECK(confidence_basis_points BETWEEN 0 AND 10000),
    review_band TEXT NOT NULL CHECK(review_band IN ('high','review','low')),
    state TEXT NOT NULL CHECK(state IN ('proposed','accepted','rejected','stale')),
    reason_codes_json TEXT NOT NULL CHECK(json_valid(reason_codes_json)),
    reviewed_at_utc_ms INTEGER,
    UNIQUE(job_id, item_id)
) STRICT;

CREATE TABLE capture_recognition_operations (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL UNIQUE REFERENCES capture_recognition_jobs(id) ON DELETE RESTRICT,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    before_revision INTEGER NOT NULL,
    after_revision INTEGER NOT NULL,
    created_entity_ids_json TEXT NOT NULL CHECK(json_valid(created_entity_ids_json)),
    created_at_utc_ms INTEGER NOT NULL,
    reverted_at_utc_ms INTEGER
) STRICT;

CREATE INDEX capture_recognition_jobs_batch_idx
ON capture_recognition_jobs(account_id, profile_id, batch_id, updated_at_utc_ms DESC);
```

Update `migrate_database` to run v13 → v14 in one transaction and reject versions above
14.

- [ ] **Step 3: Write ownership, eligibility, and snapshot tests**

Cover:

- another account/profile receives the same not-found error;
- a collecting/completed batch is rejected;
- assigned, superseded, and foreign-batch items are rejected;
- duplicate IDs and more than 150 IDs are rejected;
- `source_snapshot_hash` changes when asset ID, staged role, crop derivation, assignment,
  or active/superseded state changes;
- a second active job for the same batch returns the existing job instead of duplicating
  work.

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test capture_recognition job_
```

Expected: PASS after implementing `create_or_resume_recognition_job` and
`get_recognition_job`.

- [ ] **Step 4: Add validated JSON serialization**

Reject more than 10 regions per item, a rectangle outside `[0,1]`, a rectangle smaller
than the existing crop minimum, confidence outside `0..=10000`, unknown reason codes,
and `group_slot > 149`. Compute `review_band` in Rust rather than accepting it from the
engine:

```rust
fn review_band(confidence: u16) -> CaptureRecognitionReviewBand {
    match confidence {
        9000..=10000 => CaptureRecognitionReviewBand::High,
        6500..=8999 => CaptureRecognitionReviewBand::Review,
        _ => CaptureRecognitionReviewBand::Low,
    }
}
```

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/migrations/0014_capture_recognition_jobs.sql `
  src-tauri/src/infrastructure/database.rs `
  src-tauri/src/modules/capture_recognition.rs `
  src-tauri/tests/database_schema.rs `
  src-tauri/tests/capture_recognition.rs
git commit -m "feat: persist smart organizing jobs"
```

### Task 3: Add the bounded worker and stable command boundary

**Files:**
- Create: `src-tauri/src/infrastructure/capture_recognition_worker.rs`
- Create: `src-tauri/src/commands/capture_recognition.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/capture_recognition.rs`

**Interfaces:**

```rust
pub trait CaptureRecognitionEngine: Send + Sync {
    fn analyze(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError>;
}

pub struct CaptureRecognitionStartInput {
    pub batch_id: String,
    pub item_ids: Vec<String>,
}

pub struct CaptureRecognitionReviewInput {
    pub job_id: String,
    pub suggestion_id: String,
    pub decision: CaptureRecognitionDecision, // accepted | rejected
    pub edited_regions: Option<Vec<CaptureRecognitionRegionProposal>>,
}

pub fn capture_recognition_start(
    input: CaptureRecognitionStartInput
) -> AppResult<CaptureRecognitionJob>;
pub fn capture_recognition_status(
    batch_id: String
) -> AppResult<Option<CaptureRecognitionJob>>;
pub fn capture_recognition_review(
    input: CaptureRecognitionReviewInput
) -> AppResult<CaptureRecognitionJob>;
pub fn capture_recognition_cancel(
    job_id: String
) -> AppResult<CaptureRecognitionJob>;
```

- [x] **Step 1: Write failing worker lifecycle tests**

Use a deterministic fake engine and assert:

```rust
assert_eq!(fake.max_parallel_calls(), 1);
assert_eq!(events.last().unwrap().processed_items, 3);
assert!(!private_temp_root.path().join("decrypted.png").exists());
assert_eq!(restarted_job.state, CaptureRecognitionJobState::Queued);
```

Also cover cancellation after preprocessing, corrupt engine geometry, one item failure
while later items continue, missing model, app shutdown, and a batch discarded during a
job.

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test capture_recognition worker_
```

Expected: FAIL because the worker and engine contract do not exist.

- [x] **Step 3: Implement one managed worker**

Add a Tauri-managed `CaptureRecognitionManager` containing:

```rust
pub struct CaptureRecognitionManager {
    mutation: tokio::sync::Mutex<()>,
    cancel: tokio::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,
    engine: Arc<dyn CaptureRecognitionEngine>,
}
```

For every item:

1. revalidate account/profile/batch ownership;
2. mark the job item `running`;
3. decrypt the asset into a newly created private job directory;
4. invoke the engine through `spawn_blocking`;
5. validate and persist the proposal;
6. securely drop buffers and remove the job directory;
7. update counts and emit `capture_recognition_changed`.

The cancellation flag uses `std::sync::atomic::AtomicBool`; no new runtime dependency is
required.

On startup, change abandoned `running` items back to `pending`. Never log source names,
paths, OCR rows, or recognized anchors.

- [x] **Step 4: Map stable AppResult errors**

Use these codes and user messages:

```rust
("capture_recognition_model_missing",
 "需要先下载并校验本地识别模型；当前草稿没有变化。", false)
("capture_recognition_gate_closed",
 "智能分题仍在真实题图验证中；请继续使用顺序模板或手工整理。", false)
("capture_recognition_stale",
 "部分图片已被移动或修改；这些建议已退出本次应用。", true)
("capture_recognition_busy",
 "这个批次已有识别任务，已为你恢复现有进度。", true)
("capture_recognition_failed",
 "本次识别没有完成；原图和手工分组保持不变。", true)
```

- [x] **Step 5: Register commands and regenerate bindings**

Register the four commands, manager, and shutdown cancellation. Emit event payloads with
only `jobId`, `batchId`, `state`, `processedItems`, and `totalItems`.

Run:

```powershell
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test capture_recognition
```

Expected: bindings have no drift and the focused Rust test passes.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/infrastructure/capture_recognition_worker.rs `
  src-tauri/src/commands/capture_recognition.rs `
  src-tauri/src/modules/capture_recognition.rs `
  src-tauri/src/modules/mod.rs src-tauri/src/infrastructure/mod.rs `
  src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs `
  src-tauri/tests/capture_recognition.rs src/shared/api/bindings.ts
git commit -m "feat: run bounded local smart organizing"
```

### Task 4: Build the setup, preflight, and progress experience

**Files:**
- Create: `src/modules/ocr/components/CaptureRecognitionEntry.vue`
- Create: `src/modules/ocr/components/CaptureRecognitionEntry.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: `OcrRecognitionFeatureStatus`, current job, unassigned count, and busy state.
- Emits: `start`, `cancel`, `resume`, and `openSetup`.

- [ ] **Step 1: Write failing component tests**

Test all visible states:

```ts
expect(screen.getByRole('button', { name: '分析 12 张未分配图片' })).toBeEnabled()
expect(screen.getByText('只在本机分析，原图和现有题卡不会改变')).toBeVisible()
expect(screen.getByRole('button', { name: '去设置下载模型' })).toBeVisible()
expect(screen.getByText('智能分题仍在真实题图验证中')).toBeVisible()
expect(screen.getByRole('status')).toHaveTextContent('正在分析 12 / 50')
expect(screen.getByRole('button', { name: '停止识别' })).toBeEnabled()
```

Assert that sequence templates and `应用模板` remain present in every recognition state.

- [ ] **Step 2: Run the component test and verify failure**

Run:

```powershell
pnpm test -- src/modules/ocr/components/CaptureRecognitionEntry.test.ts `
  src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: FAIL because the entry component and props do not exist.

- [ ] **Step 3: Implement the inline entry card**

Place it immediately before `.layout-bar`. Use the exact preflight facts:

- `将分析 N 张未分配图片`
- `只在这台电脑运行`
- `不会覆盖已整理题卡`
- `原图始终保留`
- `结果需要确认后才应用`

The preflight uses a non-blocking disclosure panel with `开始分析` and `暂不用`.
Opening it does not start the job.

- [ ] **Step 4: Implement progress without locking the workbench**

Render one compact strip with a native progress element:

```vue
<progress
  :value="job.processedItems"
  :max="job.totalItems"
  :aria-label="`智能分题进度 ${job.processedItems} / ${job.totalItems}`"
/>
```

Do not feed recognition into the existing global `busy` prop. Disable only duplicate
start and model removal; templates, drag, crop, notes, and commit remain usable.

- [ ] **Step 5: Commit**

```powershell
git add src/modules/ocr/components/CaptureRecognitionEntry.vue `
  src/modules/ocr/components/CaptureRecognitionEntry.test.ts `
  src/modules/capture/components/CaptureWorkspace.vue `
  src/modules/capture/components/CaptureWorkspace.test.ts
git commit -m "feat: add smart organizing entry and progress"
```

### Task 5: Build the fast review queue and proposal crop editing

**Files:**
- Create: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Create: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`

**Interfaces:**
- Consumes: ordered suggestions and existing preview URLs.
- Emits: `review`, `edit`, `applyAccepted`, and `close`.
- Extends `CaptureCropEditor` with:

```ts
type CaptureCropEditorMode = 'apply' | 'proposal'

const props = defineProps<{
  mode?: CaptureCropEditorMode
  initialRecipes?: CaptureCropRecipe[]
}>()

const emit = defineEmits<{
  save: [recipes: CaptureCropRecipe[]]
  saveProposal: [recipes: CaptureCropRecipe[]]
  close: []
}>()
```

- [ ] **Step 1: Write failing review tests**

Cover:

- summary counts for high/review/low/stale;
- default filter starts at `需要检查` when it is non-empty, otherwise `可快速确认`;
- bulk accept selects only `high`;
- low and stale suggestions cannot be accepted;
- `J/K`, `Enter`, `E`, and `S` work without stealing keys from inputs;
- screen-reader status announces one review decision;
- closing review returns focus to `查看识别建议`;
- no raw OCR token or filename appears in the explanation copy.

Use assertions:

```ts
expect(emitted.review).toContainEqual([{
  jobId: 'job-1',
  suggestionId: 'suggestion-high',
  decision: 'accepted',
  editedRegions: null,
}])
expect(emitted.review).not.toContainEqual([
  expect.objectContaining({ suggestionId: 'suggestion-low', decision: 'accepted' }),
])
```

- [ ] **Step 2: Run the tests and verify failure**

Run:

```powershell
pnpm test -- src/modules/ocr/components/CaptureRecognitionReview.test.ts `
  src/modules/capture/components/CaptureCropEditor.test.ts
```

Expected: FAIL because review and proposal mode do not exist.

- [ ] **Step 3: Implement the inline review surface**

Use four filters: `需要检查`, `可快速确认`, `未给出建议`, `已过期`. Display a 320-pixel
preview with normalized overlay rectangles and these reason-code mappings:

```ts
const reasonCopy = {
  clear_question_anchor: '检测到清晰题号',
  matched_question_answer_anchor: '题答编号匹配',
  consistent_reading_order: '版面顺序清晰',
  weak_anchor: '题号不够清晰',
  ambiguous_columns: '分栏顺序需要检查',
  possible_content_cut: '边界附近可能还有内容',
} as const
```

Confidence remains supporting information, not the button label. Primary labels are
`接受建议`, `调整边界`, and `跳过`.

- [ ] **Step 4: Add proposal mode to the crop editor**

When `mode === 'proposal'`, initialize regions from `initialRecipes` and emit
`saveProposal` without invoking `capture_crop_apply`. Keep zoom, pan, keyboard region
movement, validation, focus trap, and reduced-motion behavior identical to ordinary
crop mode.

- [ ] **Step 5: Add the impact confirmation**

Before `applyAccepted`, show:

- accepted source image count;
- number of resulting question regions;
- number of paired answers;
- unmatched answer count returning to the material strip;
- stale/skipped count remaining unchanged.

The primary button is `应用已接受建议`; there is no “trust all future results” option.

- [ ] **Step 6: Commit**

```powershell
git add src/modules/ocr/components/CaptureRecognitionReview.vue `
  src/modules/ocr/components/CaptureRecognitionReview.test.ts `
  src/modules/capture/components/CaptureCropEditor.vue `
  src/modules/capture/components/CaptureCropEditor.test.ts `
  src/modules/capture/components/CaptureWorkspace.vue
git commit -m "feat: review smart organizing suggestions"
```

### Task 6: Apply accepted suggestions atomically and support safe undo

**Files:**
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`
- Modify: `src-tauri/src/commands/capture_recognition.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/capture_recognition.rs`

**Interfaces:**

```rust
pub struct CaptureRecognitionApplyInput {
    pub batch_id: String,
    pub job_id: String,
    pub expected_revision: i64,
    pub accepted_suggestion_ids: Vec<String>,
}

pub struct CaptureRecognitionApplyReport {
    pub operation_id: String,
    pub applied_suggestion_count: u32,
    pub created_draft_count: u32,
    pub created_item_count: u32,
    pub unmatched_answer_count: u32,
    pub stale_suggestion_count: u32,
    pub detail: CaptureBatchDetail,
}

pub struct CaptureRecognitionRevertInput {
    pub batch_id: String,
    pub operation_id: String,
    pub expected_revision: i64,
}
```

- [ ] **Step 1: Write failing atomic-apply tests**

Cover:

1. accepted high/review suggestions create derivations and draft links;
2. rejected, low, unreviewed, and stale suggestions do nothing;
3. an opaque `group_slot` with a question creates one draft and matching answers join it;
4. answer-only slots produce unassigned answer items;
5. existing drafts and assigned items are unchanged;
6. an upload after job start remains unassigned and unsuperseded;
7. a staged blob write failure creates zero DB rows and removes all staged files;
8. a DB fault after staging creates zero applied entities and leaves source assets active;
9. repeating the same apply returns the existing operation report;
10. formal Problem/outbox rows remain absent until `capture_commit_ready`.

- [ ] **Step 2: Refactor crop preparation without changing current behavior**

Extract these crate-private helpers from `apply_capture_crop`:

```rust
pub(crate) fn prepare_capture_crops(
    source: &CaptureCropSource,
    recipes: &[CaptureCropRecipe],
    asset_key: &[u8; 32],
    blob_root: &Path,
) -> Result<Vec<PreparedCaptureCrop>, CaptureInboxError>;

pub(crate) fn insert_prepared_capture_crops(
    transaction: &Transaction<'_>,
    input: InsertPreparedCaptureCrops<'_>,
) -> Result<CaptureCropInsertReport, CaptureInboxError>;
```

`prepare_capture_crops` writes only `.staging` files. The caller promotes final blobs
inside the database transaction immediately before commit, matching the existing crop
pipeline; any rollback removes staging and every newly promoted blob. Existing
single-item `capture_crop_apply` tests must remain unchanged and pass.

- [ ] **Step 3: Implement snapshot-aware apply**

Before staging, recompute every selected item snapshot. Mark only mismatches `stale` and
continue with unrelated accepted suggestions. Stage every crop, then use one database
transaction to:

- insert/deduplicate assets and derived capture items;
- insert ordinary `asset_derivations(kind='crop')`;
- supersede accepted source items;
- create drafts for question-bearing group slots;
- link matched answers;
- leave unmatched answers unassigned with `staged_role='answer'`;
- insert one `capture_recognition_operations` row;
- mark the job `applied`;
- increment the batch revision once.

If every selected suggestion is stale, return
`capture_recognition_stale` without changing the batch.

- [ ] **Step 4: Write and implement safe revert**

Revert succeeds only when every created item/draft is still in the exact state produced
by the operation and none is referenced by a formal problem. On success:

- delete generated draft links and empty generated drafts;
- delete generated capture items and derivation rows;
- restore source items;
- clean only truly orphaned assets/blobs;
- mark `reverted_at_utc_ms`;
- increment the batch revision once.

If a generated item was moved, edited, cropped again, or committed, return
`capture_recognition_revert_conflict` and change nothing.

- [ ] **Step 5: Register apply/revert and run backend gates**

Run:

```powershell
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test capture_recognition
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test capture_inbox
```

Expected: all focused tests pass and existing crop behavior has no regression.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/modules/capture_inbox.rs `
  src-tauri/src/modules/capture_recognition.rs `
  src-tauri/src/commands/capture_recognition.rs `
  src-tauri/src/bindings.rs src-tauri/tests/capture_recognition.rs `
  src/shared/api/bindings.ts
git commit -m "feat: atomically apply smart organizing"
```

### Task 7: Orchestrate jobs, setup return, apply, and undo in Vue

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/router.ts`

**Interfaces:**
- Route to setup:
  `/settings?section=ocr&returnTo=inbox&batchId=<opaque-id>`.
- Return route:
  `/inbox?batchId=<opaque-id>&recognition=resume`.
- Event: `capture_recognition_changed`.

- [ ] **Step 1: Write failing orchestration tests**

Cover:

- opening an organizing batch loads capability and current job in parallel;
- start sends exactly the current unassigned item IDs;
- progress events for another account/profile/batch are ignored by opaque job/batch ID;
- a revision conflict reloads detail and job once;
- manual edits during a job do not stop progress;
- apply refreshes detail and shows `撤销本次智能分题`;
- revert restores the returned detail;
- leaving and reopening `/inbox?batchId=...` resumes review;
- setup navigation preserves the batch return target;
- model installation returns to the same batch but does not auto-start recognition.

Use:

```ts
expect(router.push).toHaveBeenCalledWith({
  name: 'settings',
  query: { section: 'ocr', returnTo: 'inbox', batchId: 'batch-1' },
})
expect(api.captureRecognitionStart).toHaveBeenCalledWith({
  batchId: 'batch-1',
  itemIds: ['loose-1', 'loose-2'],
})
```

- [ ] **Step 2: Implement batch-preserving routing**

When a batch opens, replace the query with its opaque ID. On mount, open that batch only
after it appears in `captureBatchList`; remove an invalid ID from the query and show the
existing not-found message.

In Settings, observe `route.query.section === 'ocr'`, scroll
`#settings-ocr` into view after mount, and show `返回采集整理` only when
`returnTo === 'inbox'`. Never put a model path or source path in the URL.

- [ ] **Step 3: Add a recognition-specific controller**

Keep recognition busy/error state separate from capture mutations:

```ts
const recognition = reactive({
  capability: undefined as OcrCapabilityStatus | undefined,
  job: undefined as CaptureRecognitionJob | undefined,
  loading: false,
  applying: false,
  message: '',
})
```

Serialize review decisions per job, but coalesce rapid decisions for different
suggestions. After a conflict, reload once and preserve unsent local choices in the
review component until the command succeeds.

- [ ] **Step 4: Wire apply and undo**

After apply, show a non-expiring workbench notice until the user dismisses it, leaves
the batch, performs another recognition operation, or chooses undo. Do not use a
five-second toast for the only recovery path.

- [ ] **Step 5: Run Vue tests**

Run:

```powershell
pnpm test -- src/app/views/CaptureView.test.ts `
  src/modules/capture/components/CaptureWorkspace.test.ts `
  src/app/views/SettingsView.test.ts `
  src/modules/ocr/components/CaptureRecognitionEntry.test.ts `
  src/modules/ocr/components/CaptureRecognitionReview.test.ts
```

Expected: all focused tests pass.

- [ ] **Step 6: Commit**

```powershell
git add src/app/views/CaptureView.vue src/app/views/CaptureView.test.ts `
  src/modules/capture/components/CaptureWorkspace.vue `
  src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts `
  src/app/router.ts
git commit -m "feat: complete smart organizing review flow"
```

### Task 8: Upgrade backup validation, documentation, and release acceptance

**Files:**
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/backup_store.rs`
- Create: `docs/windows-smart-question-organizing-acceptance.md`
- Modify: `docs/architecture.md`
- Modify: `docs/plans/library.md`

**Interfaces:**
- Consumes: schema v14 jobs, suggestions, operations, and ordinary asset derivations.
- Produces: validated backup/restore and a release evidence checklist.

- [ ] **Step 1: Write the failing v14 backup test**

Create a fixture containing a running job, reviewed suggestions, one applied operation,
and referenced source/derived assets. Assert:

- backup manifest reports schema 14;
- restore preserves job/suggestion/operation rows and encrypted assets;
- validation rejects a v14 database missing any recognition table;
- validation rejects foreign account/profile rows;
- the package contains no `optional-components`, `.staging`, decrypted image, OCR text,
  or absolute path.

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml `
  --test backup_store
```

Expected: FAIL until v14 validation is added.

- [ ] **Step 2: Update backup validation**

Require the four v14 tables only when `user_version >= 14`. Traverse job → item →
capture asset ownership and operation → job → batch ownership. Model cache remains
outside the backup root.

- [ ] **Step 3: Write the Windows acceptance matrix**

Include:

- model missing, corrupt, installed, gate closed, and unsupported hardware;
- 1, 50, and 150 unassigned images;
- start, cancel, restart recovery, app exit, and batch discard;
- single question, multiple questions per page, two columns, continuation page,
  question-only, answer-only, unequal question/answer counts, diagrams, formulas,
  handwriting, blur, perspective, shadow, and pen marks;
- manual drag/crop while the worker runs;
- new upload during a job;
- accept, edit, skip, high-confidence bulk accept, atomic apply, and safe undo;
- failure injection before staging, after staging, and inside the DB transaction;
- backup/restore with pending review;
- keyboard-only and screen-reader review;
- 150-preview scroll/drag responsiveness and main-thread tasks below `50 ms`;
- no network request during start, analysis, review, apply, or undo.

- [ ] **Step 4: Run all release gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

Expected: every command passes; the signed-app smoke test can organize manually with the
model missing and can complete the full suggestion/review/apply/undo loop with the
evidence-gated runtime installed.

- [ ] **Step 5: Update architecture and roadmap**

Document that smart organizing is a local, review-required capture derivative. Keep
verified text/LaTeX extraction as a separate future workflow that cannot block capture
commit.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/modules/backup.rs src-tauri/tests/backup_store.rs `
  docs/windows-smart-question-organizing-acceptance.md `
  docs/architecture.md docs/plans/library.md
git commit -m "docs: validate smart organizing release"
```

## Self-Review

- Spec coverage: availability, setup return, eligible scope, resumable jobs, local
  worker, progress, conservative confidence, inline review, proposal editing, atomic
  apply, stale protection, undo, backup, accessibility, privacy, and performance each
  map to a task.
- Scope boundary: this plan does not duplicate model benchmarking or verified content
  extraction. It ships one independently useful capture-organization slice.
- Placeholder scan: no unspecified implementation or error-handling step remains.
- Type consistency: job, suggestion, review, apply, and revert names match across Rust,
  bindings, Vue orchestration, and tests.
- Safety check: manual work remains canonical until explicit apply; sources remain
  encrypted and recoverable; no recognition result enters Problem/outbox before the
  existing commit command.
