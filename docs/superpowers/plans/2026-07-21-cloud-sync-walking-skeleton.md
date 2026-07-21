# Cloud Account and Two-Device Sync Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the existing offline encrypted library to real Supabase email/password accounts and complete one truthful manual two-device sync loop for profiles, formal problems, immutable images, review events, exports, and tombstones.

**Architecture:** Rust owns Supabase Auth, credential storage, HTTP, canonical outbox projection, asset transfer, pull/apply, and conflict recording. The local UUID account remains the encrypted-library owner; a one-time binding maps it to exactly one Supabase `auth.users.id`, and wire DTOs translate only the account field so existing local data is never rewritten. Vue receives typed account/sync summaries and invokes manual actions; it never receives tokens, project keys, local account IDs, database handles, blob paths, or raw cloud payloads.

**Tech Stack:** Rust 1.97, `reqwest = 0.13.4` with rustls/JSON/stream, Windows Credential Manager through the existing `SecretStore`, rusqlite/SQLCipher, AES-GCM local blobs, Supabase Auth/PostgREST/Storage/RLS, Tauri 2.11 typed commands, Vue 3 strict TypeScript, Vitest/Testing Library, Supabase CLI `2.109.1`.

## Global Constraints

- Windows is the only v1 runtime target; the active review and capture flows remain fully offline.
- Supabase project URL and publishable key are build-time public configuration. A service-role or secret key must never enter the repository, desktop binary, Vue state, logs, or command DTOs.
- Only `https://<project-ref>.supabase.co` is accepted. The Rust client disables redirects, caps response bodies, sets explicit connect/request timeouts, and uses rustls.
- Email and password cross one typed command only for the immediate Auth request. Passwords are never persisted or logged. Refresh tokens live only in Windows Credential Manager; access tokens live only in Rust memory.
- A local encrypted library can bind to one Supabase user ID exactly once. A different user must be rejected without changing the library, outbox, credentials, or binding.
- Capture batches/drafts/items, generated DOCX/image folders, preview caches, local settings, and active review-session cursor state do not sync in this slice.
- Formal assets remain encrypted at rest locally. Rust decrypts only while streaming through HTTPS to the private Storage bucket; downloads are decoded/validated, hashed, and freshly AES-GCM-encrypted before local commit.
- Asset identity is account-wide by plaintext SHA-256. Remote assets must not be owned by a single learner profile.
- Outbox JSON is a wake-up record, not trusted wire state. Push reloads canonical rows and ordered `problem_assets` from SQLCipher by opaque entity ID.
- Push order is learner profiles, assets, problem aggregates, append-only review events, export snapshots, and tombstones. Pull is ordered by monotonic `change_seq` and advances its cursor in the same local transaction as applied rows.
- Review events merge by set union. After pull, affected schedules are deterministically rebuilt from the complete ordered event set; a remote schedule projection is never trusted as source data.
- Repeated push, refresh, upload, download, and pull must be idempotent. A failed batch remains retryable with bounded exponential backoff and never deletes an outbox row prematurely.
- No Realtime subscription, background tray service, automatic conflict resolution UI, class sharing, invitation, or multi-account switching is included. Those are separate hardening slices after the manual loop is proven.

---

### Task 1: Correct and version the Supabase wire contract

**Files:**
- Create: `supabase/migrations/202607210001_sync_contract_v2.sql`
- Create: `supabase/tests/0002_sync_contract.test.sql`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `docs/plans/sync.md`

**Interfaces:**
- Consumes: existing `public.app_change_seq`, owner RLS policies, private `mistake-assets` bucket, and `auth.uid()` tenancy.
- Produces: `public.push_sync_batch(p_operations jsonb)`, `public.pull_account_changes(p_after bigint, p_limit integer)`, account-wide `assets`, and pgtap contract tests.

- [ ] **Step 1: Write failing pgtap tests for the v2 contract**

Create two authenticated users and assert:

```sql
select is(
  (select count(*) from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'aaaaaaaa-0000-4000-8000-000000000001',
      'entityType', 'learner_profile',
      'operation', 'upsert',
      'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      'payload', jsonb_build_object('id', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'name', '主档案', 'revision', 1)
    )
  ))),
  1::bigint,
  'one owned profile operation is acknowledged'
);
```

Repeat the same operation ID and assert one row and the same revision. Attempt another account ID, profile ID, Storage prefix, oversized batch (`101`), unknown field/entity/operation, mutable review event, and a problem referencing a foreign asset; each must fail without partial rows. Assert a pull after `change_seq = 0` contains an ordered profile, asset, problem aggregate with ordered links, review event, export snapshot, and tombstone, but never another user's row.

- [ ] **Step 2: Pin and run the database test tool to verify failure**

Add:

```json
{
  "devDependencies": { "supabase": "2.109.1" },
  "scripts": { "supabase:test": "supabase test db" }
}
```

Run: `pnpm install --lockfile-only && pnpm supabase:test`

Expected: the new contract test fails because v2 functions and columns do not exist. Docker Desktop must be running; if unavailable, record the exact environmental gate and still validate SQL syntax in the hosted development project before calling this task complete.

- [ ] **Step 3: Migrate assets to account-wide ownership**

The migration must rebuild `public.assets` without `profile_id`, retain `unique(account_id, plaintext_sha256)`, keep `storage_object` under `<auth.uid()>/`, and update `problem_assets` to reference `(account_id, asset_id)`. The migration must be transactional and preserve any existing rows through `assets_v2`/`problem_assets_v2` copy-and-swap checks.

```sql
create table public.assets_v2 (
  id uuid primary key,
  account_id uuid not null references auth.users(id) on delete cascade,
  plaintext_sha256 text not null check (plaintext_sha256 ~ '^[0-9a-f]{64}$'),
  storage_object text not null,
  byte_length bigint not null check (byte_length > 0),
  media_type text not null check (media_type in ('image/jpeg','image/png','image/webp')),
  revision bigint not null check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, plaintext_sha256),
  unique (account_id, id),
  check (storage_object = account_id::text || '/' || plaintext_sha256)
);
```

- [ ] **Step 4: Implement bounded idempotent push and account pull RPCs**

`push_sync_batch` accepts 1–100 operations, derives `account_id` exclusively from `(select auth.uid())`, validates UUID/text/array sizes, and applies the whole batch in one PostgreSQL transaction. It records operation IDs in `public.applied_sync_operations(operation_id uuid, account_id uuid, applied_at timestamptz, primary key(account_id, operation_id))`; an existing ID returns its stored acknowledgement without reapplying.

`pull_account_changes` emits at most 500 rows ordered by `(change_seq, entity_type, entity_id)`. A `problem` payload includes `tags`, scalar fields, and a bounded `assets` array of `{ assetId, role, position }`. Asset payloads contain metadata and a private Storage object name, never a signed URL.

- [ ] **Step 5: Run RLS and contract tests**

Run: `pnpm supabase:test`

Expected: all `0001` and `0002` pgtap assertions pass, including cross-account denial and repeated operation replay.

- [ ] **Step 6: Commit the remote contract checkpoint**

```powershell
git add package.json pnpm-lock.yaml supabase docs/plans/sync.md
git commit -m "feat: define idempotent cloud sync contract"
```

### Task 2: Build a secret-safe Supabase Auth client and local binding

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `src-tauri/src/infrastructure/supabase.rs`
- Create: `src-tauri/src/modules/auth_sync.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Create: `src-tauri/tests/auth_sync.rs`
- Create: `src-tauri/tests/supabase_client.rs`

**Interfaces:**
- Consumes: build-time `MISTAKE_TRAINER_SUPABASE_URL` and `MISTAKE_TRAINER_SUPABASE_PUBLISHABLE_KEY`, existing `SecretStore`, and local account/device IDs.
- Produces: `SupabaseClient`, `AuthSessionStore`, `CloudBinding`, `AuthStatus`, `sign_up`, `sign_in`, `refresh`, and `disconnect` use cases.

- [ ] **Step 1: Write failing HTTP and credential tests**

Use an in-process Axum server and an in-memory `SecretStore`. Assert exact Auth paths/headers/bodies, generic invalid-credential messages, no password/token in `Debug` or public error strings, 2 MiB response cap, 10 s connect/30 s request timeout, no redirects, refresh rotation, email-verification/no-session signup, and network/429/5xx retry classification.

```rust
assert_eq!(request.uri(), "/auth/v1/token?grant_type=password");
assert_eq!(request.headers()["apikey"], PUBLISHABLE_KEY);
assert!(!format!("{client:?}").contains(PUBLISHABLE_KEY));
assert!(!format!("{session:?}").contains("refresh-secret"));
```

Binding tests must prove first login stores `remote-user-id`, later login by the same user succeeds, a different user returns `LibraryBoundToAnotherAccount`, and every failure leaves both keyring values and the encrypted database unchanged.

- [ ] **Step 2: Add the pinned HTTP dependency and verify failure**

```toml
reqwest = { version = "=0.13.4", default-features = false, features = ["json", "rustls", "stream", "system-proxy"] }
```

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test supabase_client --test auth_sync`

Expected: fails because the new modules/types do not exist.

- [ ] **Step 3: Implement hardened public configuration and HTTP transport**

```rust
pub struct SupabaseConfig {
    base_url: reqwest::Url,
    publishable_key: SecretString,
}

pub trait SupabaseTransport: Send + Sync {
    async fn sign_up(&self, email: &str, password: &str) -> Result<AuthReply, CloudError>;
    async fn sign_in(&self, email: &str, password: &str) -> Result<AuthReply, CloudError>;
    async fn refresh(&self, refresh_token: &str) -> Result<AuthReply, CloudError>;
    async fn revoke(&self, access_token: &str) -> Result<(), CloudError>;
}
```

Validate the URL once at startup: HTTPS, no username/password/query/fragment, path `/`, and hostname suffix `.supabase.co`. Derive the direct Storage host only by inserting `.storage` before that validated suffix; never accept a second user-controlled URL. Construct one reqwest client with redirect policy `none`, user agent, bounded timeouts, and rustls. Read response bytes through a capped stream before deserializing.

- [ ] **Step 4: Implement session storage and permanent library binding**

Use keyring names `cloud-refresh-token` and `cloud-user-id`. The local `account-id` remains unchanged. On first successful session, write the remote ID only after the refresh token write succeeds; compensate the refresh token if binding persistence fails. On startup, refresh into an in-memory access token. No token enters SQLite.

```rust
pub struct CloudBinding {
    pub local_account_id: String,
    remote_user_id: String,
}

pub enum AuthStatusKind { Unconfigured, SignedOut, VerificationRequired, Connected, Offline }
```

- [ ] **Step 5: Run focused auth tests and inspect dependency features**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test supabase_client --test auth_sync`

Expected: all tests pass and `cargo tree -e features -i reqwest` contains rustls but no native-tls.

- [ ] **Step 6: Commit the Auth core checkpoint**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/infrastructure src-tauri/src/modules src-tauri/tests/auth_sync.rs src-tauri/tests/supabase_client.rs
git commit -m "feat: connect secure Supabase sessions"
```

### Task 3: Add local sync state and canonical wire projection

**Files:**
- Create: `src-tauri/migrations/0011_cloud_sync_state.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Create: `src-tauri/src/modules/sync_store.rs`
- Create: `src-tauri/tests/sync_store.rs`
- Modify: `src-tauri/tests/database_migrations.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Consumes: local v10 schema and canonical formal entities.
- Produces: v11 `cloud_sync_state`, leased outbox access, `WireOperation`, `PendingAssetTransfer`, cursor management, and version-aware backup validation.

- [ ] **Step 1: Write failing v10→v11 and projection tests**

Assert migration preserves every row/blob hash and adds exactly one account cursor. Insert stale/misleading outbox JSON, then assert `lease_push_batch` reloads the canonical profile/problem/assets/ordered links/review/export/tombstone from normalized tables. Assert 100-operation cap, dependency order, expired-lease recovery, retry backoff, asset dedupe, cross-account/profile rejection, and no capture table rows in any wire DTO.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_migrations --test sync_store --test backup_store`

Expected: fails before v11 and `sync_store` exist.

- [ ] **Step 3: Add the v11 state migration**

```sql
CREATE TABLE cloud_sync_state (
  account_id TEXT PRIMARY KEY NOT NULL,
  pull_cursor INTEGER NOT NULL DEFAULT 0 CHECK(pull_cursor >= 0),
  last_attempt_at_utc_ms INTEGER,
  last_success_at_utc_ms INTEGER,
  last_error_code TEXT,
  remote_user_fingerprint TEXT
) STRICT;

CREATE TABLE cloud_asset_transfers (
  asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  upload_url TEXT NOT NULL,
  confirmed_offset INTEGER NOT NULL DEFAULT 0 CHECK(confirmed_offset >= 0),
  expires_at_utc_ms INTEGER NOT NULL,
  updated_at_utc_ms INTEGER NOT NULL
) STRICT;

ALTER TABLE sync_operations ADD COLUMN lease_id TEXT;
ALTER TABLE sync_operations ADD COLUMN lease_expires_at_utc_ms INTEGER;
ALTER TABLE sync_operations ADD COLUMN last_error_code TEXT;
CREATE INDEX sync_operations_lease_idx ON sync_operations(status, lease_expires_at_utc_ms);
```

`remote_user_fingerprint` is SHA-256 of the remote UUID plus an application domain separator; it detects a mismatched binding without storing/exposing the raw remote ID in SQLite.

- [ ] **Step 4: Implement canonical projection and leasing**

```rust
pub enum WireEntity {
    LearnerProfile(WireProfile),
    Asset(WireAsset),
    Problem(WireProblemAggregate),
    ReviewEvent(WireReviewEvent),
    ExportSnapshot(WireExportSnapshot),
    Tombstone(WireTombstone),
}

pub struct LeasedPushBatch {
    pub lease_id: String,
    pub operations: Vec<WireOperation>,
    pub assets: Vec<PendingAssetTransfer>,
}
```

The transaction first resets expired leases, selects due rows, loads canonical state, and marks selected operations `processing` with a 5-minute lease. Acknowledgement deletes only matching `(operation_id, lease_id)` rows. Failure restores them to `pending`, increments attempts, records a stable code, and schedules `min(2^attempt * 5 s, 30 min)` plus deterministic per-operation jitter.

- [ ] **Step 5: Raise backup validation to v11 and run tests**

Backups at schema v11 must contain `cloud_sync_state`, `cloud_asset_transfers`, and new outbox columns, but refresh/access tokens remain absent. Older v1–v10 backups retain their existing validation rules. Transfer rows may be restored, but an expired URL is discarded before any network request.

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_migrations --test sync_store --test backup_store`

Expected: all focused tests pass.

- [ ] **Step 6: Commit the local sync-state checkpoint**

```powershell
git add src-tauri/migrations src-tauri/src/infrastructure/database.rs src-tauri/src/modules/backup.rs src-tauri/src/modules/sync_store.rs src-tauri/tests
git commit -m "feat: project canonical sync batches"
```

### Task 4: Push metadata and private immutable assets idempotently

**Files:**
- Create: `src-tauri/src/modules/sync_push.rs`
- Create: `src-tauri/tests/sync_push.rs`
- Modify: `src-tauri/src/infrastructure/supabase.rs`
- Modify: `src-tauri/src/modules/mod.rs`

**Interfaces:**
- Consumes: refreshed in-memory access token, `LeasedPushBatch`, encrypted local assets, v2 `push_sync_batch`, and private Storage.
- Produces: `push_once(runtime, session, now) -> PushReport` with acknowledged operation IDs and retry-safe asset uploads.

- [ ] **Step 1: Write failing push integration tests**

With an Axum fake Supabase, assert refresh-before-expiry, account translation to remote user, dependency order, canonical problem links, successful replay after a dropped acknowledgement, 401 refresh-and-retry once, 429/5xx backoff, nonretryable 4xx failure, missing/corrupt local blob failure, TUS resume from a server-reported offset, an expired upload URL restarting safely, and no outbox deletion until both required asset upload and RPC acknowledgement succeed.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_push --test sync_store --test encrypted_asset`

Expected: fails because `sync_push` does not exist.

- [ ] **Step 3: Implement private asset upload**

For each unsatisfied account-wide hash, decrypt and authenticate the local blob, verify plaintext length/hash/media type, and upload to `mistake-assets/<remote-user-id>/<sha256>`. Files up to 6 MiB use the standard endpoint; larger files use Supabase TUS at `https://<project-ref>.storage.supabase.co/storage/v1/upload/resumable` with exactly 6 MiB chunks. Persist only the opaque TUS upload URL and confirmed offset in encrypted `cloud_asset_transfers`; URLs expire after 24 hours and are discarded on 404/410. Use `x-upsert: false`; treat an existing object as success only after remote metadata matches hash/length/media type. Keep plaintext in one bounded buffer, overwrite it after completion/failure, and never log the upload URL or response body.

- [ ] **Step 4: Implement transactional metadata push**

Replace every local `accountId` in the canonical wire operation with the bound remote user ID inside Rust. POST at most 100 operations to `/rest/v1/rpc/push_sync_batch` with `apikey` and `Authorization: Bearer <access-token>`. Validate exact acknowledgement IDs; unknown, missing, or duplicate acknowledgements fail the lease without deleting it.

- [ ] **Step 5: Run push tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_push --test sync_store --test encrypted_asset`

Expected: all focused tests pass.

- [ ] **Step 6: Commit the push checkpoint**

```powershell
git add src-tauri/src/infrastructure/supabase.rs src-tauri/src/modules/sync_push.rs src-tauri/src/modules/mod.rs src-tauri/tests/sync_push.rs
git commit -m "feat: push encrypted library changes"
```

### Task 5: Pull, validate, apply, and rebuild schedules

**Files:**
- Create: `src-tauri/src/modules/sync_pull.rs`
- Create: `src-tauri/tests/sync_pull.rs`
- Modify: `src-tauri/src/infrastructure/supabase.rs`
- Modify: `src-tauri/src/modules/sync_store.rs`
- Modify: `src-tauri/src/modules/review.rs`

**Interfaces:**
- Consumes: v2 `pull_account_changes`, private Storage, local account translation, AES-GCM blob store, and FSRS schedule rebuild.
- Produces: `pull_until_current(runtime, session, now) -> PullReport`, atomic cursor advancement, unioned review history, conflict rows, and freshly encrypted local assets.

- [ ] **Step 1: Write failing pull and two-device tests**

Seed remote changes from device A, pull into an empty device B fixture, and assert identical profile/problem/link/image/review/export/tombstone semantics and identical due dates. Cover duplicate pages, repeated cursor, out-of-order/duplicate sequence, malformed JSON, wrong account/profile/entity IDs, invalid Storage object prefix, hash mismatch, corrupt/oversized image, partial download, tombstone-before-upsert, a local pending edit, and an interrupted apply before/after blob rename.

- [ ] **Step 2: Run focused tests and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull --test review_store`

Expected: fails because pull/apply does not exist.

- [ ] **Step 3: Implement bounded remote decoding and account translation**

Accept 1–500 strictly increasing changes and known fields only. Require every remote `account_id` to equal the bound user, then replace it with the local account ID before persistence. Reject UUID/profile relationships that do not close within local/remote owned data. Never deserialize directly into SQL write structs without validation.

- [ ] **Step 4: Stage downloads and apply one page atomically**

For each new asset, download through the private Storage API, cap at 25 MiB/12,000 px/80 MP, decode, verify SHA-256 and media type, encrypt into `.sync-pull/<page-id>`, and record planned final paths. Begin one SQLite transaction, apply profiles/assets/problems/links/review events/exports/tombstones, create true same-field conflicts when a local pending operation has a divergent base revision, rebuild affected schedules, move staged blobs, set `pull_cursor = max(change_seq)`, then commit. On any failure rollback SQL and remove only this page's staged/final new blobs.

- [ ] **Step 5: Run pull/two-device tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull --test sync_push --test review_store`

Expected: all focused tests pass and device A/device B schedules are equal from the same event set.

- [ ] **Step 6: Commit the pull checkpoint**

```powershell
git add src-tauri/src/infrastructure/supabase.rs src-tauri/src/modules/sync_pull.rs src-tauri/src/modules/sync_store.rs src-tauri/src/modules/review.rs src-tauri/tests/sync_pull.rs
git commit -m "feat: pull and merge cloud changes"
```

### Task 6: Expose typed Auth/sync commands and a truthful account workspace

**Files:**
- Create: `src-tauri/src/commands/auth.rs`
- Create: `src-tauri/src/commands/sync.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/shared/api/bindings.ts`
- Create: `src-tauri/tests/auth_sync_command.rs`
- Create: `src/modules/auth-sync/components/CloudAccountPanel.vue`
- Create: `src/modules/auth-sync/components/SyncStatusPanel.vue`
- Create: `src/modules/auth-sync/components/CloudAccountPanel.test.ts`
- Create: `src/modules/auth-sync/components/SyncStatusPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src-tauri/src/modules/insights.rs`

**Interfaces:**
- Produces typed commands `auth_status`, `auth_sign_up`, `auth_sign_in`, `auth_disconnect`, `sync_status`, and `sync_now` using `AppResult<T>`.
- Produces a settings workspace for unconfigured, signed-out, verification-required, connected, offline, syncing, failed, and current states.

- [ ] **Step 1: Write failing command/security and UI state tests**

Assert commands accept only email/password or no input; no URL/key/token/account/device/path is accepted or returned. Assert concurrent `sync_now` calls coalesce, profile transitions/LAN session transitions cannot race sync apply, progress events ignore stale run IDs, and all public errors are stable/path-free/token-free.

UI tests cover password visibility, disabled double-submit, generic invalid credentials, verification instructions, offline use, pending/failed/conflict counts, real phase progress, retry, last successful time, keyboard focus, 44 px targets, and reduced motion. No panel may claim “已同步” while pending operations, failed operations, unresolved conflicts, or an incomplete pull remain.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test auth_sync_command --test command_contract --test bindings_contract
pnpm exec vitest run src/modules/auth-sync src/app/views/SettingsView.test.ts
```

Expected: fails before commands/components exist.

- [ ] **Step 3: Implement commands and one serialized sync coordinator**

```rust
pub struct SyncStatus {
    pub state: SyncState,
    pub pending_count: i32,
    pub failed_count: i32,
    pub unresolved_conflict_count: i32,
    pub last_success_at_utc_ms: Option<f64>,
}

pub struct SyncReport {
    pub pushed_count: i32,
    pub pulled_count: i32,
    pub downloaded_asset_count: i32,
    pub conflict_count: i32,
}
```

Hold one `SyncCoordinator` mutex per app. Acquire the existing profile-transition lock before the database lock. Refresh session, push until no due operations, pull until the final short page, then return a report. Emit `sync_progress` with an opaque run ID and bounded phase/counts only.

- [ ] **Step 4: Implement the account and sync panels**

Use a compact settings card, not a full-screen mandatory login. Local work remains available when unconfigured, signed out, or offline. The connected state shows redacted email, last successful sync, queued work, conflicts, and one primary `立即同步` action. Use 120 ms feedback, 180 ms status transitions, and a 240 ms account sheet; animate only opacity/transform/progress scale and disable transitions under reduced motion.

- [ ] **Step 5: Regenerate bindings and run focused tests**

Run:

```powershell
pnpm bindings:generate
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test auth_sync_command --test command_contract --test bindings_contract
pnpm exec vitest run src/modules/auth-sync src/app/views/SettingsView.test.ts
```

Expected: all focused command, binding, and UI tests pass.

- [ ] **Step 6: Commit the public workflow checkpoint**

```powershell
git add src-tauri/src/commands src-tauri/src/bindings.rs src-tauri/src/lib.rs src-tauri/src/modules/insights.rs src/shared/api/bindings.ts src/modules/auth-sync src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: guide cloud account sync"
```

### Task 7: Document, audit, and verify the walking skeleton

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/sync.md`
- Create: `docs/windows-cloud-sync-acceptance.md`
- Modify: `.env.example`
- Modify: `docs/superpowers/plans/2026-07-21-cloud-sync-walking-skeleton.md`

**Interfaces:**
- Produces: reproducible configuration, automated evidence, two-device Windows acceptance, and a final local Git checkpoint.

- [ ] **Step 1: Document the delivered security and data boundaries**

Document public build config, token placement, permanent library binding, local/remote account translation, canonical outbox projection, asset plaintext-in-transit boundary, retry/lease semantics, pull validation, conflicts, source-of-truth rules, and the deliberate exclusions. `.env.example` contains variable names and fake values only.

- [ ] **Step 2: Run all automated quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
pnpm supabase:test
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

Expected: all pass; initial JS remains below 300 KB gzip and each lazy account/settings feature block remains below 120 KB gzip.

- [ ] **Step 3: Perform real hosted and two-device Windows acceptance**

On a dedicated Supabase development project, register/verify/login, sync a profile with a multi-image problem and one review from device A, disconnect its network, train again, sync after reconnect, then install on device B and pull. Verify the same image roles/order, review union, due date, exports, deletes/restores, RLS denial, retry after interrupted upload, restart session refresh, and truthful UI counts. Record that capture drafts and generated exports did not upload.

- [ ] **Step 4: Audit the complete range and fix Critical/Important findings**

Review exact commits for secret leakage, SSRF/redirects, response bounds, binding mismatch, account translation, RLS, canonical projection, lease loss, partial acknowledgements, Storage ownership, plaintext lifetime, path containment, transaction/blob compensation, cursor atomicity, delete resurrection, event immutability, conflict safety, stale UI events, keyboard behavior, and reduced motion. Rerun every affected focused suite.

- [ ] **Step 5: Create the final local checkpoint**

```powershell
git add .env.example docs
git commit -m "docs: verify cloud sync walking skeleton"
git status --short
```

Expected: clean worktree. Do not push without explicit authorization for the resulting SHA.

## Deliberate Follow-up Slices

The walking skeleton proves real ownership and data movement but does not finish all release hardening. Separate plans must add explicit sign-out database closure/offline unlock, trusted-device management, automatic background scheduling, conflict-resolution UI, 30-day remote purge jobs, signed updater/installer, and the authenticated server-side capture Agent broker. These slices must build on the wire contract above rather than widening this plan mid-implementation.
