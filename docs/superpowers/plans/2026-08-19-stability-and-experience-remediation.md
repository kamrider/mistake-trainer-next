# Stability and Experience Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the current release blocker, make report calendar metrics consistently local, bound large-library resource use, and turn Windows reliability evidence into repeatable gates.

**Architecture:** Each remediation remains independently releasable. Dependency resolution stays pinned in the workspace manifest; report bucketing stays in the Rust read repository; library pagination is cursor-owned by Rust and consumed incrementally by Vue; Windows warning handling and acceptance evidence remain explicit rather than hiding production failures.

**Tech Stack:** pnpm 11, Vue 3, TypeScript, Tauri 2, Rust 1.97, rusqlite/SQLCipher, Vitest, Rust integration tests, GitHub Actions.

## Global Constraints

- Preserve all existing uncommitted smart-question-organizing changes without staging, rewriting, or reverting them.
- Keep the encrypted local library authoritative and do not change its schema for pagination.
- Keep problem search limits, filter semantics, account/profile scoping, and the 100-item bulk-action ceiling unchanged.
- Calendar-day metrics use the caller-supplied Windows UTC offset in the inclusive range `-840..=840`.
- Do not disable `cipher_memory_security` in production to silence a development-machine warning.
- Do not create commits while unrelated user changes are present unless the user explicitly requests commits.

---

### Task 1: Remove the production dependency audit blocker

**Files:**
- Modify: `pnpm-workspace.yaml`
- Modify: `pnpm-lock.yaml`
- Test: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: pnpm workspace resolution and the existing `pnpm security:audit` CI command.
- Produces: one lockfile resolution of `nanoid` at `3.3.18` or newer, with no high-severity production audit finding.

- [ ] **Step 1: Add an explicit safe transitive override**

Update the existing workspace override:

```yaml
overrides:
  nanoid: 3.3.18
  postcss: 8.5.23
```

- [ ] **Step 2: Regenerate only dependency metadata**

Run: `corepack pnpm install --lockfile-only`

Expected: `pnpm-lock.yaml` records `nanoid: 3.3.18` in `overrides`, package snapshots, and dependents; application source files do not change.

- [ ] **Step 3: Verify the release gate**

Run: `corepack pnpm security:audit`

Expected: exit code `0` and no high-severity production vulnerability.

- [ ] **Step 4: Verify dependency changes did not alter the application**

Run: `corepack pnpm test && corepack pnpm build`

Expected: all Vitest tests pass and the production build completes.

### Task 2: Make report activity and streaks use local calendar days

**Files:**
- Modify: `src-tauri/tests/insights_store.rs`
- Modify: `src-tauri/src/modules/insights_read_repository.rs`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: `report_summary(connection, account_id, profile_id, now_utc_ms, utc_offset_minutes)`.
- Produces: `DailyActivity.day_start_utc_ms` values representing each local-day boundary in UTC, plus `current_streak_days` derived from those same buckets.

- [ ] **Step 1: Add a failing non-zero-offset Rust integration test**

Insert review events on adjacent local dates around a UTC boundary, call `report_summary(..., -480)`, and assert that the newest two `daily_activity` buckets each contain one event and `current_streak_days == 2`.

```rust
let offset_minutes = -480;
let report = report_summary(
    &connection,
    "account-1",
    &profile.id,
    now_utc_ms,
    offset_minutes,
)
.unwrap();
assert_eq!(report.current_streak_days, 2);
assert_eq!(
    report.daily_activity.iter().rev().take(2)
        .map(|day| day.review_count).collect::<Vec<_>>(),
    vec![1, 1],
);
```

- [ ] **Step 2: Run the focused test and confirm the UTC implementation fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store report_uses_local_calendar_days`

Expected: FAIL because the existing SQL groups directly by `occurred_at_utc_ms / DAY_MS`.

- [ ] **Step 3: Apply the offset symmetrically when creating and querying buckets**

Use:

```rust
let offset_ms = i64::from(utc_offset_minutes) * 60_000;
let today_bucket = (now_utc_ms + offset_ms).div_euclid(DAY_MS);
let start_bucket = today_bucket - (REPORT_DAYS - 1);

// Returned timestamps remain UTC instants for the local-day boundaries.
day_start_utc_ms: ((start_bucket + offset) * DAY_MS - offset_ms) as f64,
```

Group SQL by `(occurred_at_utc_ms + ?1) / ?2`, query by the exact UTC interval, and map returned bucket numbers relative to `start_bucket`. Derive the streak with `current_streak_from_buckets` so dashboard and report share identical semantics.

- [ ] **Step 4: Correct the user-facing report copy**

Replace `按 UTC 训练日计算` with `按本地训练日计算`. Keep `dayLabel` formatting aligned with the returned local-day boundary so negative and positive offsets display the intended local date.

- [ ] **Step 5: Run focused and regression tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store`

Run: `corepack pnpm test -- src/app/views/ReportView.test.ts`

Expected: both commands pass, including non-zero positive and negative offset coverage.

### Task 3: Bound problem-list queries and load thumbnails incrementally

**Files:**
- Modify: `src-tauri/src/modules/problems.rs`
- Modify: `src-tauri/src/modules/problem_query_repository.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts` through the existing binding generator
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: corresponding Rust and Vitest problem-list tests

**Interfaces:**
- Consumes: the existing validated filters and ordering `(updated_at_utc_ms DESC, id DESC)`.
- Produces: `ProblemListPage { items, next_cursor }`, where the opaque cursor encodes the final item ordering tuple and each page contains at most 40 summaries.

- [ ] **Step 1: Add repository tests for first page, next page, stable ordering, and filter isolation**

Create more than 40 problems with duplicate timestamps. Assert no duplicate or missing IDs across pages, account/profile filters remain enforced, and malformed cursors return `InvalidQuery` without reading assets.

- [ ] **Step 2: Define the typed page contract**

```rust
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemListPage {
    pub items: Vec<ProblemSummary>,
    pub next_cursor: Option<String>,
}
```

Extend `ProblemListInput` with `cursor: Option<String>` and validate an opaque bounded cursor. Use a fixed `PAGE_SIZE: usize = 40` and query `PAGE_SIZE + 1` rows to determine continuation.

- [ ] **Step 3: Add keyset predicates and a SQL limit**

Append a predicate equivalent to:

```sql
AND (?11 IS NULL OR p.updated_at_utc_ms < ?11
  OR (p.updated_at_utc_ms = ?11 AND p.id < ?12))
ORDER BY p.updated_at_utc_ms DESC, p.id DESC
LIMIT ?13
```

Decrypt and resize previews only for the returned page items.

- [ ] **Step 4: Regenerate and verify Tauri bindings**

Run: `corepack pnpm bindings:check`

Expected: generated TypeScript exposes `ProblemListPage` and the cursor field with no remaining diff after generation is accepted.

- [ ] **Step 5: Make Vue replace on filter changes and append on continuation**

`LibraryView.vue` keeps `nextCursor`, sends `cursor: null` on refresh, and exposes `loadMore()` with its own single-flight state. Appends only when the filter request epoch still matches. `LibraryWorkspace.vue` renders a retryable `加载更多` action and never discards the already visible page after an append failure.

- [ ] **Step 6: Add large-list interaction tests**

Assert initial mount contains only the first 40 cards, the continuation action appends in server order, concurrent clicks make one command call, filter changes reject stale append responses, and selection is retained only for still-visible items.

- [ ] **Step 7: Run problem-list, library-view, binding, and build gates**

Run the focused Rust repository test, `corepack pnpm test -- src/app/views/LibraryView.test.ts src/modules/library/components/LibraryWorkspace.test.ts`, `corepack pnpm bindings:check`, and `corepack pnpm build`.

Expected: all pass and the generated command payload remains fully typed.

### Task 4: Make Windows reliability evidence bounded and repeatable

**Files:**
- Modify: `src-tauri/src/infrastructure/database.rs` only if a test-only connection policy is required
- Modify: `.github/workflows/ci.yml`
- Create: `scripts/windows-rust-test.ps1`
- Create: `docs/releases/0.1.3.md`
- Update: applicable `docs/windows-*-acceptance.md` files only with actually observed evidence

**Interfaces:**
- Consumes: `cargo test --all-targets`, SQLCipher production memory security, Windows CI logs, installer smoke results.
- Produces: bounded Rust test logs, explicit timeout/failure summaries, and immutable acceptance evidence for version 0.1.3.

- [ ] **Step 1: Reproduce and classify warning sources separately**

Run focused SQLCipher contract tests, one encrypted database integration test, and the all-target suite. Record whether `VirtualLock` 1453 occurs during connection setup, query execution, teardown, or only under aggregate parallel pressure.

- [ ] **Step 2: Add a bounded PowerShell test runner**

The runner streams normal output, counts identical `VirtualLock` and OpenSSL PDB warnings, prints the first occurrence plus a final count, preserves the exact cargo exit code, and terminates owned test processes after a configured timeout. It must never turn a failing test into success.

- [ ] **Step 3: Keep production memory protection enabled**

If aggregate tests require a test-only connection option, expose it only under `cfg(test)` or an explicit test binary input. Production `open_encrypted_database` and backup validation paths continue executing `PRAGMA cipher_memory_security = ON`.

- [ ] **Step 4: Wire CI to the bounded runner and test the runner contract**

Add script tests proving exit-code preservation, warning deduplication, timeout ownership, and absence of broad process termination. Replace only the Rust-test workflow command after those tests pass.

- [ ] **Step 5: Create current release evidence**

Record exact commit, CI run, installer hashes, supported Windows/DPI results, 150-image/1-GB resource measurements, low-disk behavior, and unresolved rows in `docs/releases/0.1.3.md`. Never convert an unchecked acceptance row to checked without captured evidence.

- [ ] **Step 6: Run the complete release audit**

Run lint, typecheck, coverage, build, architecture contracts, binding checks, Rust formatting/lint/tests, x64 installer smoke, and the applicable manual matrix.

Expected: every automated gate exits `0`; unresolved manual rows remain visibly blocking rather than being inferred from unit tests.
