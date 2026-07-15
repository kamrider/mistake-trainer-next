# Capture Agent Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a learner scan, shoot a batch, and stop; the desktop then proposes subjects, question/answer roles, and safe pairings, while requiring human confirmation before anything enters the formal library.

**Architecture:** Build a bounded, persisted analysis pipeline rather than a free-running chat agent. Rust decrypts selected capture assets, creates temporary analysis derivatives, calls a provider through a port, validates strict structured output, and writes suggestions only; a separate transaction applies accepted suggestions to capture drafts without deleting images or committing formal `Problem` records. The experiment calls OpenAI directly from Rust with a user-owned key stored in Windows Credential Manager; the future platform adapter replaces that port with an authenticated Supabase broker after `auth-sync` exists.

**Tech Stack:** Tauri 2.11, Rust 2024, rusqlite/SQLCipher, reqwest with rustls, Vue 3, TypeScript strict, Vitest, Tauri Specta, OpenAI Responses API image input and strict JSON Schema output.

## Global Constraints

- Manual templates and drag/drop remain fully usable offline when no Agent credential or network is available.
- The experimental API key is stored only in Windows Credential Manager; it never enters Vue state, logs, SQLite, diagnostics, or generated bindings.
- The Agent may create suggestions and capture drafts only. It may not delete assets, move manually assigned items, commit `Problem` rows, or write sync outbox operations.
- Only an explicit per-batch consent action may send analysis derivatives to a remote model.
- Analysis derivatives are JPEG, longest edge 2048 px, quality 0.88, no EXIF, generated in memory, and never added to the asset store.
- A classification request contains at most 8 images; at most 2 requests run concurrently; each request has a 10-second connect timeout and 90-second total timeout.
- Confidence is stored as integer basis points. At least 9,000 is high confidence, 6,500–8,999 is review-required, and below 6,500 remains unassigned.
- The initial provider is `openai`; its configurable default model is `gpt-5.6-luna`, image detail is `high`, `store` is `false`, and prompt version is `capture-v1`.
- Every provider result is validated against the requested opaque item IDs; unknown, duplicate, or missing IDs fail the chunk without changing capture assignments.
- A job is stale when its recorded item-set hash differs from the current batch. Applying a stale job skips changed or manually assigned items and reports every skip.
- The public platform must not ship a shared OpenAI key in the desktop binary. Moving the provider call behind Supabase is a separate release task gated by real Supabase auth and RLS.

---

## File map

- `src-tauri/migrations/0004_capture_agent.sql`: persisted local jobs and suggestions.
- `src-tauri/src/modules/capture_agent.rs`: domain types, repository, deterministic reconciliation, and safe application transaction.
- `src-tauri/src/modules/capture_agent_runner.rs`: bounded background runner, retry, progress, and cancellation.
- `src-tauri/src/infrastructure/capture_agent_provider.rs`: credential store, derivative creation, provider port, and OpenAI adapter.
- `src-tauri/src/commands/capture_agent.rs`: typed commands, public errors, and Tauri events.
- `src-tauri/tests/capture_agent_*.rs`: store, provider, runner, and command contracts.
- `src/modules/capture/components/CaptureAgentPanel.vue`: consent, progress, suggestion review, apply, retry, and cancel UI.
- `src/app/views/CaptureView.vue`: command orchestration and event refresh.
- `src/app/views/SettingsView.vue`: BYOK credential status, set, and clear controls.
- `docs/capture-agent-evaluation.md`: privacy disclosure, pilot procedure, metrics, and promotion gate.

### Task 1: Persist jobs and immutable suggestions

**Files:**
- Create: `src-tauri/migrations/0004_capture_agent.sql`
- Create: `src-tauri/src/modules/capture_agent.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Test: `src-tauri/tests/capture_agent_store.rs`

**Interfaces:**
- Consumes: `capture_batches`, `capture_items`, `capture_drafts`, and `capture_draft_items`.
- Produces: `CaptureAgentJobSummary`, `CaptureAgentSuggestion`, `CaptureAgentApplyReport`, `create_agent_job`, `replace_agent_suggestions`, `get_agent_job`, `cancel_agent_job`, and `apply_agent_suggestions`.

- [ ] **Step 1: Write the failing store test**

```rust
#[test]
fn applying_agent_suggestions_never_commits_problems_or_outbox() {
    let mut fixture = CaptureFixture::new();
    let batch = fixture.batch_with_unassigned_images(4);
    let before_outbox = fixture.outbox_count();
    let job = create_agent_job(&mut fixture.connection, &batch.id, "a".repeat(64), 4, 1).unwrap();
    replace_agent_suggestions(&mut fixture.connection, &job.id, suggestions_for_four_items()).unwrap();
    let report = apply_agent_suggestions(
        &mut fixture.connection,
        &fixture.account_id,
        &fixture.profile_id,
        ApplyAgentSuggestions { job_id: job.id, selected_group_keys: vec!["g1".into()], apply_batch_subject: true },
        2,
    ).unwrap();
    assert_eq!(report.created_draft_count, 1);
    assert_eq!(fixture.problem_count(), 0);
    assert_eq!(fixture.outbox_count(), before_outbox);
    assert_eq!(fixture.asset_count(), 4);
}
```

- [ ] **Step 2: Verify the test fails before implementation**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_store`

Expected: compilation fails because `capture_agent` and migration 0004 do not exist.

- [ ] **Step 3: Add the strict schema**

```sql
CREATE TABLE capture_agent_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('queued','running','ready','applied','failed','cancelled')),
    provider TEXT NOT NULL CHECK(length(provider) BETWEEN 1 AND 40),
    model TEXT NOT NULL CHECK(length(model) BETWEEN 1 AND 80),
    prompt_version TEXT NOT NULL CHECK(length(prompt_version) BETWEEN 1 AND 40),
    item_set_hash TEXT NOT NULL CHECK(length(item_set_hash) = 64),
    processed_item_count INTEGER NOT NULL DEFAULT 0 CHECK(processed_item_count >= 0),
    total_item_count INTEGER NOT NULL CHECK(total_item_count BETWEEN 1 AND 150),
    suggested_subject TEXT CHECK(suggested_subject IS NULL OR length(suggested_subject) <= 40),
    subject_confidence_bps INTEGER CHECK(subject_confidence_bps IS NULL OR subject_confidence_bps BETWEEN 0 AND 10000),
    error_code TEXT CHECK(error_code IS NULL OR length(error_code) <= 80),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    UNIQUE(batch_id, item_set_hash, prompt_version)
) STRICT;

CREATE TABLE capture_agent_suggestions (
    job_id TEXT NOT NULL REFERENCES capture_agent_jobs(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    subject TEXT NOT NULL DEFAULT '' CHECK(length(subject) <= 40),
    role TEXT NOT NULL CHECK(role IN ('question','answer','unknown')),
    group_key TEXT CHECK(group_key IS NULL OR length(group_key) BETWEEN 1 AND 80),
    confidence_bps INTEGER NOT NULL CHECK(confidence_bps BETWEEN 0 AND 10000),
    reason_code TEXT NOT NULL CHECK(reason_code IN ('number_match','visual_match','sequence_match','model_only','ambiguous')),
    anchor_text TEXT NOT NULL DEFAULT '' CHECK(length(anchor_text) <= 160),
    PRIMARY KEY(job_id, item_id)
) STRICT;
```

- [ ] **Step 4: Implement public domain types and the high-confidence guard**

```rust
#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAgentSuggestion {
    pub item_id: String,
    pub subject: String,
    pub role: CaptureAgentRole,
    pub group_key: Option<String>,
    pub confidence_bps: u16,
    pub reason_code: CaptureAgentReason,
    pub anchor_text: String,
}

pub fn is_auto_applicable(value: &CaptureAgentSuggestion) -> bool {
    value.confidence_bps >= 9_000
        && value.group_key.is_some()
        && value.role != CaptureAgentRole::Unknown
}
```

`apply_agent_suggestions` opens one transaction, re-checks account/profile ownership, selects only unassigned items, creates one draft per selected group, orders roles by source sequence, fills an empty batch subject only at 9,000+ confidence, and commits only after validating every ID.

- [ ] **Step 5: Run migrations and capture regressions**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_store --test capture_inbox_store --test database_schema`

Expected: all pass and v3 data upgrades without changing problem or asset hashes.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/migrations/0004_capture_agent.sql src-tauri/src/modules/capture_agent.rs src-tauri/src/modules/mod.rs src-tauri/src/infrastructure/database.rs src-tauri/tests/capture_agent_store.rs
git commit -m "feat: persist capture agent proposals"
```

### Task 2: Add secure credentials and an OpenAI provider adapter

**Files:**
- Create: `src-tauri/src/infrastructure/capture_agent_provider.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/tests/capture_agent_provider.rs`

**Interfaces:**
- Consumes: encrypted asset bytes, `SecretStore`, and opaque capture item IDs.
- Produces: `CaptureAgentCredentialStore`, `CaptureAnalysisProvider`, `OpenAiCaptureAnalysisProvider`, `CaptureAnalysisChunk`, and `CaptureAnalysisChunkResult`.

- [ ] **Step 1: Write provider tests against a local Axum server**

```rust
#[tokio::test]
async fn provider_sends_derivatives_with_strict_output_and_no_paths() {
    let server = FakeResponsesServer::start(valid_response_for(&["item-a", "item-b"])).await;
    let provider = OpenAiCaptureAnalysisProvider::new(server.url(), "test-key", "gpt-5.6-luna").unwrap();
    let result = provider.analyze(chunk(&["item-a", "item-b"])).await.unwrap();
    assert_eq!(result.items.len(), 2);
    let request = server.single_request();
    assert_eq!(request["store"], false);
    assert_eq!(request["text"]["format"]["strict"], true);
    assert!(!request.to_string().contains("C:\\"));
    assert!(!request.to_string().contains("asset-key"));
}
```

Add 401, 429/`Retry-After`, timeout, refusal, malformed JSON, duplicate ID, unknown ID, and missing requested ID cases.

- [ ] **Step 2: Verify the provider test fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_provider`

Expected: compilation fails on the missing provider module.

- [ ] **Step 3: Add HTTP and credential boundaries**

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
zeroize = "1"
```

```rust
pub const CAPTURE_AGENT_SERVICE: &str = "com.mistaketrainer.next.capture-agent";
pub const CAPTURE_AGENT_KEY: &str = "openai-api-key";

pub trait CaptureAgentCredentialStore: Send + Sync {
    fn has_key(&self) -> Result<bool, CredentialError>;
    fn set_key(&self, value: &str) -> Result<(), CredentialError>;
    fn clear_key(&self) -> Result<(), CredentialError>;
    fn load_key(&self) -> Result<Zeroizing<String>, CredentialError>;
}
```

Reject empty keys and values over 512 bytes, redact `Debug`, and expose only `has_key` to commands.

- [ ] **Step 4: Generate in-memory derivatives and the strict request**

```rust
let request = ResponsesRequest {
    model: self.model.clone(),
    store: false,
    input: vec![capture_prompt_with_images(chunk, "high")],
    text: strict_capture_json_schema("capture_chunk_v1"),
};
```

The JSON Schema requires one object per item with `itemId`, `subject`, `role`, `anchorText`, `confidenceBps`, and `reasonCode`, with `additionalProperties: false` at every object level. Decode in Rust, resize to 2048 px, flatten alpha on warm white, encode JPEG 0.88, and create `data:image/jpeg;base64,...` without plaintext files.

- [ ] **Step 5: Verify provider and secret redaction**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_provider --test runtime_state`

Expected: all pass; output contains neither the key nor a local path.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/infrastructure/capture_agent_provider.rs src-tauri/src/infrastructure/mod.rs src-tauri/src/infrastructure/runtime.rs src-tauri/tests/capture_agent_provider.rs
git commit -m "feat: add secure capture analysis provider"
```

### Task 3: Build the bounded two-pass runner

**Files:**
- Modify: `src-tauri/src/modules/capture_agent.rs`
- Create: `src-tauri/src/modules/capture_agent_runner.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/capture_agent_runner.rs`

**Interfaces:**
- Consumes: `CaptureAnalysisProvider`, source order, persisted jobs, and a cancellation watch channel.
- Produces: `CaptureAgentManager::start`, `status`, `retry`, `cancel`, and `capture_agent_changed` notifications.

- [ ] **Step 1: Write runner tests with a deterministic fake provider**

```rust
#[tokio::test]
async fn runner_reconciles_pairs_and_preserves_manual_work() {
    let fixture = AgentFixture::new(10);
    fixture.manually_assign("item-3");
    let job = fixture.manager.start(fixture.context(), Arc::new(FakeProvider::mixed())).await.unwrap();
    fixture.manager.wait_until_terminal(&job.id).await;
    let report = fixture.apply(job.id, vec!["pair-1", "pair-2"]);
    assert!(report.skipped_item_ids.contains(&"item-3".to_owned()));
    assert_eq!(fixture.problem_count(), 0);
}
```

Add cancellation, 429 retry, interrupted app restart, partial chunk failure, and new-upload staleness tests.

- [ ] **Step 2: Verify the runner test fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_runner`

Expected: compilation fails on `CaptureAgentManager`.

- [ ] **Step 3: Implement the bounded state machine**

```rust
pub enum CaptureAgentJobState { Queued, Running, Ready, Applied, Failed, Cancelled }

pub struct CaptureAgentManager {
    active: Mutex<HashMap<String, watch::Sender<bool>>>,
}

const CHUNK_SIZE: usize = 8;
const MAX_CONCURRENT_CHUNKS: usize = 2;
const PROMPT_VERSION: &str = "capture-v1";
```

Pass one classifies each image and extracts a short anchor. Rust groups exact normalized problem numbers first, then question-before-answer adjacency within the same subject, and leaves collisions ambiguous. Only ambiguous candidate sets trigger pass two, still capped at 8 images. Persist progress after each accepted chunk.

- [ ] **Step 4: Implement bounded retry and cancellation**

```rust
fn retry_delay(attempt: u8, retry_after: Option<Duration>) -> Option<Duration> {
    if attempt >= 3 { return None; }
    Some(retry_after.unwrap_or_else(|| Duration::from_millis(800 * 2_u64.pow(attempt.into()))))
}
```

Retry transport errors, 408, 429, and 5xx only. Check cancellation before derivative creation, each request, and each write. Mark jobs that were `running` at startup as retryable `failed` with `agent_interrupted`.

- [ ] **Step 5: Run runner and capture regressions**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_runner --test capture_inbox_store --test encrypted_asset`

Expected: all pass; cancellation never changes assets, assignments, problems, or outbox.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/modules/capture_agent.rs src-tauri/src/modules/capture_agent_runner.rs src-tauri/src/modules/mod.rs src-tauri/src/lib.rs src-tauri/tests/capture_agent_runner.rs
git commit -m "feat: orchestrate capture agent analysis"
```

### Task 4: Expose typed commands without leaking credentials

**Files:**
- Create: `src-tauri/src/commands/capture_agent.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/capture_agent_command.rs`
- Generated: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `capture_agent_key_status/set/clear`, `capture_agent_start/status/retry/cancel/apply`.

- [ ] **Step 1: Write the command boundary test**

```rust
#[test]
fn commands_use_runtime_identity_and_never_return_the_key() {
    let fixture = CommandFixture::new();
    capture_agent_key_set_for(&fixture.credentials, CaptureAgentKeyInput { key: "sk-test-secret".into() });
    let serialized = serde_json::to_string(&capture_agent_key_status_for(&fixture.credentials)).unwrap();
    assert!(!serialized.contains("sk-test-secret"));
    assert!(!serialized.contains("accountId"));
}
```

- [ ] **Step 2: Implement DTOs and async commands**

```rust
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAgentApplyInput {
    pub job_id: String,
    pub selected_group_keys: Vec<String>,
    pub apply_batch_subject: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn capture_agent_start(
    app: AppHandle,
    runtime: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureAgentManager>,
    input: CaptureAgentStartInput,
) -> AppResult<CaptureAgentJobSummary> {
    start_for(&app, &runtime, &manager, input).await
}
```

Start input contains only `batch_id` and `consent_remote_analysis: true`. Map invalid key, rate limit, offline, stale apply, and cancellation to stable public codes; internal messages stay behind diagnostic IDs.

- [ ] **Step 3: Generate bindings and run contracts**

Run: `corepack pnpm bindings:generate`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_agent_command --test command_contract`

Expected: both pass and output types have no credential field.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/commands/capture_agent.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/tests/capture_agent_command.rs src/shared/api/bindings.ts
git commit -m "feat: expose capture agent commands"
```

### Task 5: Add the one-button review experience

**Files:**
- Create: `src/modules/capture/components/CaptureAgentPanel.vue`
- Create: `src/modules/capture/components/CaptureAgentPanel.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: generated Agent commands and `capture_agent_changed`.
- Emits: `start-agent`, `retry-agent`, `cancel-agent`, and `apply-agent`.

- [ ] **Step 1: Write UI tests**

```ts
it('requires consent and distinguishes confidence bands', async () => {
  const user = userEvent.setup()
  render(CaptureAgentPanel, { props: { job: readyJob, hasCredential: true } })
  await user.click(screen.getByRole('button', { name: 'Agent 自动整理' }))
  expect(screen.getByText('会把本批次的分析副本发送到云端模型')).toBeVisible()
  expect(screen.getByText('高置信度 12 组')).toBeVisible()
  expect(screen.getByText('需要确认 3 组')).toBeVisible()
  expect(screen.getByText('未分配 2 张')).toBeVisible()
})
```

Add reduced-motion, keyboard focus, cancel, offline fallback, missing-key routing, stale result, and apply-report cases.

- [ ] **Step 2: Implement the shortest product path**

```text
扫码 → 连续拍/相册多选 → 结束拍摄 → Agent 自动整理 → 确认建议 → 保存全部就绪题
```

Show `正在识别 12 / 50`. Preselect only 9,000+ groups, mark 6,500–8,999 for review, and leave lower confidence unassigned. Never hide manual templates or drag controls.

- [ ] **Step 3: Implement credential UX**

```vue
<input v-model="draftKey" type="password" autocomplete="off"
  aria-label="OpenAI API 密钥" placeholder="只保存到 Windows 凭据管理器" />
```

Clear `draftKey` immediately after save and show only configured/unconfigured. Clear requires confirmation. Do not add reveal, persistence, telemetry, or diagnostics containing the key.

- [ ] **Step 4: Run UI and frontend gates**

Run: `corepack pnpm test -- src/modules/capture/components/CaptureAgentPanel.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/SettingsView.test.ts`

Run: `corepack pnpm lint && corepack pnpm typecheck && corepack pnpm build`

Expected: all pass, keyboard-only flow works, and initial desktop JS grows by at most 20 KB gzip.

- [ ] **Step 5: Commit**

```powershell
git add src/modules/capture/components/CaptureAgentPanel.vue src/modules/capture/components/CaptureAgentPanel.test.ts src/modules/capture/components/CaptureWorkspace.vue src/app/views/CaptureView.vue src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: add capture agent review flow"
```

### Task 6: Evaluate before enabling by default

**Files:**
- Create: `docs/capture-agent-evaluation.md`
- Modify: `docs/architecture.md`
- Modify: `docs/windows-capture-acceptance.md`
- Modify: `README.md`

**Interfaces:**
- Produces: a repeatable opt-in pilot gate and a platform migration decision.

- [ ] **Step 1: Document the pilot gate**

```markdown
| Metric | Promotion gate |
| --- | ---: |
| Batch subject accuracy | >= 95% |
| Question/answer role macro F1 | >= 90% |
| Correct question-answer grouping | >= 85% |
| Groups requiring correction | <= 15% |
| 50-image p95 completion time | <= 120 seconds |
| Silent image loss or formal auto-commit | 0 |
```

Use at least 20 consented batches spanning Chinese, English, mathematics, physics, chemistry, screenshots, camera photos, multi-image questions, answer sheets, questions-only, and shuffled order. Record cost and latency per batch; do not set a platform price before measuring the pilot distribution.

- [ ] **Step 2: Add failure acceptance cases**

Cover valid key, invalid key, offline, 429, cancellation, restart, new uploads during analysis, manual moves during analysis, malformed provider output, 150 images, mixed subjects, missing answers, one answer matching multiple questions, and rejecting all suggestions. Every failure must preserve the manual workspace and encrypted assets.

- [ ] **Step 3: Run all gates**

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Run: `corepack pnpm test`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Run: `corepack pnpm bindings:check`

Run: `corepack pnpm tauri build`

Expected: all exit 0; no CI test requires a live paid key.

- [ ] **Step 4: Commit**

```powershell
git add docs/capture-agent-evaluation.md docs/architecture.md docs/windows-capture-acceptance.md README.md
git commit -m "docs: add capture agent promotion gate"
```

## Self-review result

- Coverage: one-action capture, subject inference, role classification, pairing, confirmation, recovery, privacy, provider abstraction, and platform migration boundary are all assigned to tasks.
- Deliberate split: the public Supabase broker is not mixed into the experiment because the app does not yet have real Supabase authentication. BYOK proves value without embedding a shared key; broker work starts only after `auth-sync` supplies access tokens and RLS identity.
- Type consistency: `CaptureAgentJobSummary`, `CaptureAgentSuggestion`, `CaptureAgentApplyInput`, and `CaptureAgentApplyReport` keep the same names across Rust, bindings, Vue, and tests.
- Safety: no task grants the model or Vue direct database/file access, and no Agent path creates a formal `Problem` without the existing `capture_commit_ready` action.
- Placeholder scan: every implementation, validation, error, and test step is concrete.

## Official API references

- Image inputs and detail levels: <https://developers.openai.com/api/docs/guides/images-vision>
- Strict structured outputs: <https://developers.openai.com/api/docs/guides/structured-outputs>
- Background mode for the later broker: <https://developers.openai.com/api/docs/guides/background>
- Signed completion webhooks for the later broker: <https://developers.openai.com/api/docs/guides/webhooks>
