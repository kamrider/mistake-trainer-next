# Review History and Algorithm Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a profile-scoped, paginated review-history workspace where learners can filter real ReviewEvents, inspect one event in detail, and audit the algorithm and parameter versions that produced it.

**Architecture:** A dedicated Rust `review_history` use case owns filtering, cursor validation, profile isolation, and detail projection. Tauri commands inject runtime identity and expose typed DTOs; Vue adds a lazy `/report/history` page with a master-detail timeline, while the existing report page remains the entry point. The list cursor is opaque, event IDs never enter the route, and raw device IDs never cross the command boundary.

**Tech Stack:** SQLite/SQLCipher migration, Rust + rusqlite + base64 + serde + Specta, Tauri 2 typed commands, Vue 3 + Vue Router, Vitest/Testing Library, Lucide, existing paper-and-ink motion tokens.

## Global Constraints

- All queries are restricted by runtime `account_id` and active `profile_id`; callers cannot submit either identity.
- Supported rolling ranges are exactly `all | 7_days | 30_days` and exact rating filters are `again | hard | good | easy | null`.
- Search is trimmed, at most 80 Unicode scalar values, and treats `%`, `_`, and `\` as literal characters.
- Page size is 20 in the UI and must be accepted only in `1..=50`; cursors are URL-safe base64, at most 512 bytes, and fail closed when malformed.
- Ordering is deterministic: `occurred_at_utc_ms DESC, id DESC`; cursor pagination cannot duplicate or skip equal-time rows.
- List DTOs do not contain `problem_id`, `device_id`, file paths, database handles, or decrypted images.
- Detail accepts only an opaque event ID and returns `isCurrentDevice`; it never returns the raw device ID.
- Event algorithm and parameter versions are immutable audit facts. Current schedule fields are clearly labelled as the current projection, not historical snapshots.
- Event IDs never enter routes, browser history, filenames, logs, or arbitrary global Vue state.
- The page uses explicit “load more”, not an infinite-scroll observer. Failed page/detail reads preserve already visible data and remain retryable.
- Motion is limited to transform and opacity, respects `prefers-reduced-motion`, preserves 44 px targets and keyboard focus, and never delays persistence or data visibility.
- No new JavaScript dependency. Keep the lazy history feature chunk below 120 KB gzip and initial JavaScript below 300 KB gzip.
- Do not push a commit without explicit authorization for its exact SHA.

---

### Task 1: Add the v9 history query index

**Files:**
- Create: `src-tauri/migrations/0009_review_history_index.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produces `review_events_profile_time_idx(account_id, profile_id, occurred_at_utc_ms DESC, id DESC)`.
- Raises the encrypted-library and backup schema version from 8 to 9 without changing any row or column.

- [x] **Step 1: Write the failing v8-to-v9 migration test**

Create a schema-v8 library containing a review event, schedule state, and active focus session. After migration, compare all existing row values and assert:

```rust
assert_eq!(user_version, 9);
assert_eq!(review_event_before, review_event_after);
assert_eq!(index_columns, vec!["account_id", "profile_id", "occurred_at_utc_ms", "id"]);
```

- [x] **Step 2: Run the schema test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema`

Expected: FAIL because schema version 9 and `review_events_profile_time_idx` do not exist.

- [x] **Step 3: Add the additive migration and wire every starting version**

Use:

```sql
CREATE INDEX review_events_profile_time_idx
ON review_events(account_id, profile_id, occurred_at_utc_ms DESC, id DESC);
```

Every schema path 0 through 8 must apply `0009_review_history_index.sql` last and set `user_version=9`. Change backup `CURRENT_SCHEMA_VERSION` to 9 and require the index for a version-9 package; version 1 through 8 packages remain valid and migrate after restore.

- [x] **Step 4: Run migration and backup tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: both PASS with review-event rows unchanged.

- [x] **Step 5: Create a local checkpoint**

```powershell
git add src-tauri/migrations/0009_review_history_index.sql src-tauri/src/infrastructure/database.rs src-tauri/src/modules/backup.rs src-tauri/tests/database_schema.rs src-tauri/tests/backup_store.rs
git commit -m "perf: index profile review history"
```

### Task 2: Implement the Rust history query and detail projection

**Files:**
- Create: `src-tauri/src/modules/review_history.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/modules/review.rs`
- Create: `src-tauri/tests/review_history_store.rs`

**Interfaces:**
- Consumes `review_events`, `problems`, and the current `schedule_states` projection.
- Produces:

```rust
enum ReviewHistoryRange { All, SevenDays, ThirtyDays }

struct ReviewHistoryQuery {
    account_id: String,
    profile_id: String,
    range: ReviewHistoryRange,
    rating: Option<FsrsRating>,
    subject: Option<String>,
    search: String,
    cursor: Option<String>,
    limit: u32,
    now_utc_ms: i64,
}

struct ReviewHistoryPage {
    items: Vec<ReviewHistoryItem>,
    next_cursor: Option<String>,
    total_count: i32,
    available_subjects: Vec<String>,
}

struct ReviewHistoryDetailQuery {
    account_id: String,
    profile_id: String,
    event_id: String,
    current_device_id: String,
}
```

- [x] **Step 1: Write failing store tests**

Cover deterministic equal-time ordering, two-page traversal without duplicates, each range/rating/subject/search filter, literal wildcard search, archived-problem visibility, cross-account/profile isolation, invalid limit/cursor/search, and available-subject ordering. Detail tests must cover full note, `reviewOrdinal`, `problemReviewCount`, current schedule fields, legacy/current version flags, current/other device booleans, and foreign/missing event rejection.

- [x] **Step 2: Run the focused test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_history_store`

Expected: FAIL because the module and types do not exist.

- [x] **Step 3: Implement bounded filters and opaque cursor helpers**

Encode only `{ occurred_at_utc_ms, event_id }` using `base64::engine::general_purpose::URL_SAFE_NO_PAD`. Decode only after checking the 512-byte bound. Validate event ID length `1..=128`, page size `1..=50`, subject `1..=40` after trim, and search at most 80 characters. Escape LIKE input using `ESCAPE '\'`.

- [x] **Step 4: Implement one deterministic list query**

Fetch `limit + 1` rows, then truncate and derive `next_cursor` from the final returned item. Apply cursor ordering as:

```sql
AND (?cursor_time IS NULL
  OR e.occurred_at_utc_ms < ?cursor_time
  OR (e.occurred_at_utc_ms = ?cursor_time AND e.id < ?cursor_id))
ORDER BY e.occurred_at_utc_ms DESC, e.id DESC
LIMIT ?limit_plus_one
```

The count and available-subject queries must use the same account/profile and filter boundaries. Map times and durations to Specta-safe `f64` values.

- [x] **Step 5: Implement the scoped detail query**

Join the selected event to its problem and left-join current schedule state. Compute ordinal and total using the event ordering `(occurred_at_utc_ms, id)`. Compare stored versions with public crate constants from `review.rs`, but label schedule values as current. Return `NotFound` for every foreign/missing event without revealing which boundary failed.

- [x] **Step 6: Run the focused store tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_history_store`

Expected: all pagination, isolation, audit, and validation tests PASS.

- [x] **Step 7: Create a local checkpoint**

```powershell
git add src-tauri/src/modules/review_history.rs src-tauri/src/modules/mod.rs src-tauri/src/modules/review.rs src-tauri/tests/review_history_store.rs
git commit -m "feat: query auditable review history"
```

### Task 3: Add typed commands and binding contracts

**Files:**
- Create: `src-tauri/src/commands/review_history.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/command_contract.rs`
- Modify: `src/shared/api/bindings.ts` (generated)

**Interfaces:**
- Produces `review_history_list(input: ReviewHistoryInput)`.
- Produces `review_history_detail(eventId: string)`.
- Both return the shared `AppResult<T>` and derive identity from `LibraryRuntime`.

- [x] **Step 1: Write failing command contract tests**

Serialize the exact range union, rating nullable field, page/detail DTO camelCase names, and stable invalid-input/not-found errors. Assert serialized public errors contain no SQL, raw device ID, local path, or internal error text.

- [x] **Step 2: Run command tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract`

Expected: FAIL because history commands are absent.

- [x] **Step 3: Implement runtime-identity command adapters**

Map invalid query/cursor to non-retryable `review_history_query_invalid`, missing/foreign detail to non-retryable `review_history_event_missing`, and database/serialization failures to retryable `review_history_read_failed`. User messages must remain Chinese and path-free.

- [x] **Step 4: Register commands and regenerate bindings**

Register both commands in `bindings.rs`, export TypeScript, and confirm inputs contain only filters/cursor or event ID. No account/profile/device ID may appear in the generated inputs.

- [x] **Step 5: Run contracts and binding generation**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
pnpm bindings:generate
pnpm typecheck
```

Expected: PASS and generated bindings contain both commands and exact unions.

- [x] **Step 6: Create a local checkpoint**

```powershell
git add src-tauri/src/commands/review_history.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/tests/command_contract.rs src/shared/api/bindings.ts
git commit -m "feat: expose typed review history commands"
```

### Task 4: Build the history master-detail workspace

**Files:**
- Create: `src/modules/review-history/components/ReviewHistoryFilters.vue`
- Create: `src/modules/review-history/components/ReviewHistoryFilters.test.ts`
- Create: `src/modules/review-history/components/ReviewHistoryTimeline.vue`
- Create: `src/modules/review-history/components/ReviewHistoryTimeline.test.ts`
- Create: `src/modules/review-history/components/ReviewHistoryDetail.vue`
- Create: `src/modules/review-history/components/ReviewHistoryDetail.test.ts`
- Create: `src/app/views/ReviewHistoryView.vue`
- Create: `src/app/views/ReviewHistoryView.test.ts`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`
- Modify: `src/app/router.ts`
- Modify: `src/app/router.test.ts`
- Modify: `src/app/App.vue`

**Interfaces:**
- Consumes generated `reviewHistoryList` and `reviewHistoryDetail` commands.
- Produces lazy route `/report/history` named `review-history` with `meta.shellPage='report'`.

- [x] **Step 1: Write failing component and view tests**

Cover initial loading, genuine empty state, filter submission/reset, cursor append, append failure preserving prior rows, retry, deterministic date grouping, rating copy, row keyboard activation, detail request race cancellation, drawer close/focus restoration, current versus legacy version badges, no raw device/event ID in visible copy, and reduced-motion-independent state changes.

- [x] **Step 2: Write failing route and report-entry tests**

Assert the history route is lazy, carries `shellPage='report'`, and report contains a discoverable “查看完整复习历史” link. Update `App.vue` so the report navigation remains active on the child page.

- [x] **Step 3: Implement explicit filters**

Render rolling range, exact rating, subject, and search controls in a single labelled form. Submitting or clearing resets cursor and selected detail, starts request epoch protection, and replaces the list only after a successful response. A failed replacement keeps the prior list visibly marked as stale and retryable.

- [x] **Step 4: Implement the date-grouped timeline**

Use button rows grouped by the Windows local date. Each row shows time, subject, note preview, rating seal, and duration. “加载更多” appends only on success and is removed when `nextCursor` is null. Card entry uses a 120 ms transform/opacity transition; no layout animation runs for bulk page append.

- [x] **Step 5: Implement the audit detail surface**

Desktop uses a sticky right panel; at 760 px and below it becomes an overlay sheet with focusable close control and background scrim. Show full note, rating/time/duration, review ordinal, problem status, current due/stability/difficulty, immutable event versions, current/legacy badges, and “本机设备/其他设备” only. Never render raw event or device IDs.

- [x] **Step 6: Add development preview and restrained motion**

Support `/#/report/history?preview=history` with static non-private sample data. Use only transform/opacity transitions, remove them under `prefers-reduced-motion`, keep every interactive target at least 44 px, and preserve visible focus/high contrast.

- [x] **Step 7: Run focused UI tests, lint, typecheck, and build**

Run:

```powershell
pnpm vitest run src/modules/review-history/components/ReviewHistoryFilters.test.ts src/modules/review-history/components/ReviewHistoryTimeline.test.ts src/modules/review-history/components/ReviewHistoryDetail.test.ts src/app/views/ReviewHistoryView.test.ts src/app/views/ReportView.test.ts src/app/router.test.ts
pnpm lint
pnpm typecheck
pnpm build
```

Expected: all PASS; initial and lazy chunks remain within budget.

- [x] **Step 8: Create a local checkpoint**

```powershell
git add src/modules/review-history src/app/views/ReviewHistoryView.vue src/app/views/ReviewHistoryView.test.ts src/app/views/ReportView.vue src/app/views/ReportView.test.ts src/app/router.ts src/app/router.test.ts src/app/App.vue
git commit -m "feat: add review history workspace"
```

### Task 5: Document, review, and verify the vertical slice

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/review.md`
- Create: `docs/windows-review-history-acceptance.md`
- Modify: `docs/superpowers/plans/2026-07-20-review-history-audit.md`

**Interfaces:**
- Documents cursor isolation, immutable event audit facts, current schedule disclaimer, and Windows manual acceptance.

- [x] **Step 1: Document architecture and Windows acceptance**

Acceptance covers empty/large histories, all filters, literal wildcard search, equal-time pagination, archived problems, current/legacy versions, current/other device labels, detail race handling, offline restart, 1280×900, 760×900, 390×844, keyboard operation, high contrast, and reduced motion.

- [x] **Step 2: Run all quality gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

After committing generated bindings, run `pnpm bindings:check` and require a clean worktree.

- [x] **Step 3: Perform browser and real-Tauri QA**

Inspect overview, filters, list append, empty/error state, detail open/close, legacy badge, and focus restoration. At 390×844 assert `scrollWidth === clientWidth`, no clipped rating seal or sheet action, and no console error. Reset viewport overrides afterward.

- [x] **Step 4: Request independent code review and fix findings**

Review the complete range for cursor correctness, account/profile isolation, LIKE escaping, device-ID leakage, time conversion, stale request races, focus management, motion constraints, binding drift, and unrelated files. Fix every Critical/Important issue and rerun affected focused plus full gates.

- [x] **Step 5: Mark the plan and create the final local checkpoint**

Keep all commits local. Report every new SHA and explicitly state that none was pushed.
