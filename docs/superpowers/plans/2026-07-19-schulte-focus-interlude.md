# Crash-safe Schulte Focus Interlude Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional, restart-safe 5×5 Schulte-table interlude before ordinary review sessions or after every ten completed cards, with an accessible paper-and-ink interaction and a profile-scoped setting.

**Architecture:** Store the learner preference in `profile_preferences`, snapshot it into each new ordinary `review_session`, and persist the active shuffled board, next expected number, and elapsed time in that session. Rust owns board creation and every accepted transition; Vue renders the typed state, gives immediate local feedback for a wrong tile, and never inserts the interlude into simulated exams.

**Tech Stack:** SQLite/SQLCipher migration, Rust + rusqlite + serde + Specta, Tauri 2 typed commands, Vue 3 + TypeScript, Vitest/Testing Library, existing paper-and-ink CSS motion tokens.

## Global Constraints

- The preference is profile scoped and defaults to `off`; supported values are exactly `off | session_start | every_10`.
- Existing active sessions migrate with `focus_policy='off'` and must resume unchanged.
- A setting change applies only to a newly created ordinary session; it never rewrites an active session.
- Simulated exams always use `focus_policy='off'`; no focus board interrupts either exam phase.
- A focus board contains every integer from 1 through 25 exactly once, and Rust validates each accepted number against persisted state.
- Selecting a wrong number is immediate, non-destructive UI feedback; selecting the expected number is persisted before the tile disappears.
- “暂时跳过” is always available so an attention exercise cannot trap a learner or block the review queue.
- Restart preserves the same board, the next expected number, and the last persisted elapsed time.
- The question-detail command rejects access while a focus board is active.
- Motion uses only `transform` and `opacity`, respects `prefers-reduced-motion`, and preserves 44 px targets, keyboard operation, visible focus, and screen-reader status.
- Do not add a dependency, route parameter, database handle, filesystem path, or problem ID to the focus UI.
- Do not push any commit without explicit authorization for its exact SHA.

---

### Task 1: Persist focus preferences and active-board state

**Files:**
- Create: `src-tauri/migrations/0008_review_focus.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produces `profile_preferences.review_focus_policy` with `off | session_start | every_10`.
- Produces `review_sessions.focus_policy`, `focus_round`, `focus_order_json`, `focus_next_number`, and `focus_elapsed_ms`.
- Raises the encrypted-library schema and backup validator to version 8.

- [x] **Step 1: Write the v7-to-v8 migration test**

Create a v7 database with one stored preference and one active exam/review session. After migration, assert all original rows are byte-for-byte equivalent in their existing columns and the new columns are:

```rust
assert_eq!(preference_policy, "off");
assert_eq!(session_focus, ("off", 0_i64, None::<String>, 0_i64, 0_i64));
assert!(connection.execute(
    "UPDATE profile_preferences SET review_focus_policy='sometimes'",
    [],
).is_err());
assert!(connection.execute(
    "UPDATE review_sessions SET focus_order_json='[1,2]', focus_next_number=0",
    [],
).is_err());
```

- [x] **Step 2: Run the focused schema test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema`

Expected: FAIL because schema version 8 and the focus columns do not exist.

- [x] **Step 3: Add the additive migration**

Use these exact state constraints:

```sql
ALTER TABLE profile_preferences
ADD COLUMN review_focus_policy TEXT NOT NULL DEFAULT 'off'
CHECK(review_focus_policy IN ('off', 'session_start', 'every_10'));

ALTER TABLE review_sessions
ADD COLUMN focus_policy TEXT NOT NULL DEFAULT 'off'
CHECK(focus_policy IN ('off', 'session_start', 'every_10'));
ALTER TABLE review_sessions
ADD COLUMN focus_round INTEGER NOT NULL DEFAULT 0 CHECK(focus_round >= 0);
ALTER TABLE review_sessions
ADD COLUMN focus_order_json TEXT
CHECK(focus_order_json IS NULL OR json_valid(focus_order_json));
ALTER TABLE review_sessions
ADD COLUMN focus_next_number INTEGER NOT NULL DEFAULT 0
CHECK(focus_next_number BETWEEN 0 AND 25);
ALTER TABLE review_sessions
ADD COLUMN focus_elapsed_ms INTEGER NOT NULL DEFAULT 0
CHECK(focus_elapsed_ms BETWEEN 0 AND 3600000);
```

Add both guards because SQLite cannot add a cross-column table constraint after creation:

```sql
CREATE TRIGGER review_sessions_focus_state_insert_guard
BEFORE INSERT ON review_sessions
WHEN (NEW.focus_order_json IS NULL AND NEW.focus_next_number != 0)
  OR (NEW.focus_order_json IS NOT NULL AND NEW.focus_next_number NOT BETWEEN 1 AND 25)
BEGIN SELECT RAISE(ABORT, 'invalid review focus state'); END;

CREATE TRIGGER review_sessions_focus_state_update_guard
BEFORE UPDATE OF focus_order_json, focus_next_number ON review_sessions
WHEN (NEW.focus_order_json IS NULL AND NEW.focus_next_number != 0)
  OR (NEW.focus_order_json IS NOT NULL AND NEW.focus_next_number NOT BETWEEN 1 AND 25)
BEGIN SELECT RAISE(ABORT, 'invalid review focus state'); END;
```

- [x] **Step 4: Wire every migration path and backup validation**

Apply `0008_review_focus.sql` from schema versions 0 through 7, set `user_version=8`, change `CURRENT_SCHEMA_VERSION` to 8, and extend backup schema tests so a version-8 package requires both focus columns while a version-7 fixture still migrates safely after restore.

- [x] **Step 5: Run schema and backup tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test backup_store
```

Expected: both suites PASS.

- [x] **Step 6: Create a local checkpoint**

```powershell
git add src-tauri/migrations/0008_review_focus.sql src-tauri/src/infrastructure/database.rs src-tauri/src/modules/backup.rs src-tauri/tests/database_schema.rs src-tauri/tests/backup_store.rs
git commit -m "feat: persist review focus state"
```

### Task 2: Add profile-scoped review preferences

**Files:**
- Modify: `src-tauri/src/modules/preferences.rs`
- Modify: `src-tauri/src/commands/preferences.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/preferences_store.rs`
- Modify: `src-tauri/tests/command_contract.rs`
- Modify: `src/shared/api/bindings.ts` (generated)

**Interfaces:**
- Produces `ReviewFocusPolicy = off | session_start | every_10`.
- Produces `ReviewPreferences { focusPolicy: ReviewFocusPolicy }`.
- Produces typed commands `review_preferences_get()` and `review_preferences_save({ focusPolicy })`.

- [x] **Step 1: Write failing preference-store tests**

Cover the default, each valid round trip, profile/account isolation, preservation of existing subject preferences, and rejection of a missing profile. Use a raw SQL insertion of an invalid value to prove the schema guard, not an untyped public command.

- [x] **Step 2: Run the focused preference tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test preferences_store`

Expected: FAIL because review preference types/functions are absent.

- [x] **Step 3: Implement review preference types and persistence**

Add:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFocusPolicy {
    Off,
    SessionStart,
    #[serde(rename = "every_10")]
    EveryTen,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPreferences { pub focus_policy: ReviewFocusPolicy }
```

`load_review_preferences` must return `Off` when no row exists. `save_review_preferences` must upsert only `review_focus_policy` on conflict and must use the same default subject JSON as `default_subject_preferences()` when it creates the row, so saving one preference never resets another.

- [x] **Step 4: Add typed command adapters and stable errors**

Register `review_preferences_get` and `review_preferences_save`. Map a changed/missing profile to a non-retryable error and storage failure to a retryable error; never return SQL details in `userMessage`.

- [x] **Step 5: Regenerate bindings and run contract checks**

Run:

```powershell
pnpm bindings:generate
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test preferences_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
pnpm bindings:check
```

Expected: all commands PASS and generated TypeScript contains the exact three-value union.

- [x] **Step 6: Create a local checkpoint**

```powershell
git add src-tauri/src/modules/preferences.rs src-tauri/src/commands/preferences.rs src-tauri/src/bindings.rs src-tauri/tests/preferences_store.rs src-tauri/tests/command_contract.rs src/shared/api/bindings.ts
git commit -m "feat: configure review focus timing"
```

### Task 3: Implement the Rust focus state machine

**Files:**
- Create: `src-tauri/src/modules/review_focus.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/modules/review.rs`
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/review_store.rs`
- Modify: `src-tauri/tests/command_contract.rs`
- Modify: `src/shared/api/bindings.ts` (generated)

**Interfaces:**
- Produces `ReviewFocusState { kind, roundIndex, numbers, nextNumber, elapsedMs }`.
- Extends `ReviewQueueOverview` and `ReviewSubmission` with `focus: ReviewFocusState | null`.
- Produces `review_focus_select({ number, elapsedMs })` and `review_focus_skip()`.

- [x] **Step 1: Write failing state-machine tests**

Cover all of these independent invariants:

```text
off: no board at session start or after card ten
session_start: one persisted board before the first problem
every_10: no initial board; one board after cards 10, 20, ... while the session remains active
exam: no board regardless of preference
board: exactly 1..25 with no duplicate and stable across reopen
wrong/stale number: transaction unchanged
correct number: next number and elapsed time persist
number 25: board clears and focus_round increments
skip: board clears and focus_round increments
active board: review submission and current-problem access fail closed
completed session: no trailing board after its final card
```

- [x] **Step 2: Run the focused review tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store`

Expected: FAIL because the focus state machine is absent.

- [x] **Step 3: Implement deterministic board creation**

In `review_focus.rs`, create the vector `1..=25` and sort it by SHA-256 of `session_id || ':' || round_index || ':' || number`; this produces a stable, shuffled order without a new random-number dependency. Validate deserialized boards by length, range, and uniqueness before returning them.

- [x] **Step 4: Snapshot preference when ordinary sessions start**

Due and manual review sessions copy the current profile policy. `session_start` creates round zero immediately. `every_10` starts a new board transactionally after successful review numbers 10, 20, and so on, but only when the session remains `active`. Exam insertions explicitly store `off`.

- [x] **Step 5: Enforce fail-closed transitions**

`select_focus_number` accepts only the persisted `focus_next_number`, clamps elapsed time to `0..=3_600_000`, and updates `next` or completes the round in one transaction. `skip_focus_round` completes the active round without a review event. Add `focus_order_json IS NULL` to both question-detail selection and rating advancement so stale Vue state cannot bypass the interlude.

- [x] **Step 6: Expose typed state and commands**

Return the active focus state in queue and submission DTOs. A wrong/stale selection returns `review_focus_state_changed`; a storage failure returns a retryable internal error. The page sends only `{ number, elapsedMs }` or no input for skip.

- [x] **Step 7: Run focused Rust and binding tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test bindings_contract
pnpm bindings:check
```

Expected: all suites PASS.

- [x] **Step 8: Create a local checkpoint**

```powershell
git add src-tauri/src/modules/review_focus.rs src-tauri/src/modules/mod.rs src-tauri/src/modules/review.rs src-tauri/src/commands/review.rs src-tauri/src/bindings.rs src-tauri/tests/review_store.rs src-tauri/tests/command_contract.rs src/shared/api/bindings.ts
git commit -m "feat: add crash-safe focus rounds"
```

### Task 4: Add the training-rhythm setting

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes `commands.reviewPreferencesGet()` and `commands.reviewPreferencesSave({ focusPolicy })`.
- Produces a “训练节奏” panel with three explicit radio-card choices.

- [x] **Step 1: Write failing settings tests**

Assert the page loads the stored policy, explains that changes apply to the next ordinary session, saves `every_10`, preserves the rendered selection when save fails, and never claims simulated exams are affected.

- [x] **Step 2: Run the focused settings test and verify it fails**

Run: `pnpm vitest run src/app/views/SettingsView.test.ts`

Expected: FAIL because the training-rhythm controls are absent.

- [x] **Step 3: Implement the accessible radio-card panel**

Use labels “关闭专注环节”, “每轮开始前”, and “每完成 10 题”. Put the recommended explanation on “每轮开始前”, but preserve the stored `off` default. Use a separate save button/message so a failed focus save cannot falsely report that subject configuration saved.

- [x] **Step 4: Add restrained selection motion**

Selected cards move by at most `translateY(-2px)` and reveal a small ink check with opacity. Use the existing 120/180 ms tokens and remove transitions under `prefers-reduced-motion`.

- [x] **Step 5: Run settings tests, typecheck, and lint**

Run:

```powershell
pnpm vitest run src/app/views/SettingsView.test.ts
pnpm typecheck
pnpm lint
```

Expected: all commands PASS.

- [x] **Step 6: Create a local checkpoint**

```powershell
git add src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: add focus rhythm settings"
```

### Task 5: Build the Schulte-table interaction and review orchestration

**Files:**
- Create: `src/modules/review/components/SchulteFocus.vue`
- Create: `src/modules/review/components/SchulteFocus.test.ts`
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReviewView.test.ts`

**Interfaces:**
- `SchulteFocus` consumes `ReviewFocusState`, `busy`, `completed`, and `resumed`.
- `SchulteFocus` emits `select(number, elapsedMs)`, `skip`, and `exit`.
- `ReviewView` persists correct selections before visual removal and loads the next problem only after the focus round completes or is skipped.

- [x] **Step 1: Write failing component tests**

Assert a 5×5 grid renders all numbers once, the persisted next number is announced, wrong selection shakes locally without emitting, correct selection emits elapsed time, arrow keys use roving focus, Enter selects, skip/exit emit, completed state shows the seal, and reduced-motion mode has no delayed dependency.

- [x] **Step 2: Write failing view integration tests**

Cover initial focus without calling `reviewCurrentProblem`, resume at a persisted next number, correct selection persistence, stale-selection error recovery without losing the board, skip, focus returned after the tenth rating, and transition to the next problem only after Rust returns `focus: null`.

- [x] **Step 3: Implement `SchulteFocus.vue`**

Render a square responsive 5-column board with `role="grid"`, one roving `tabindex=0`, arrow/Home/End navigation, status text “下一位 N”, a visible “暂时跳过” action, and an exit action. Correct tiles fade/scale toward the centre only after `busy` resolves; wrong tiles use a one-shot cinnabar shake. The completion seal lasts 420 ms, or zero under reduced motion.

- [x] **Step 4: Integrate focus state into `ReviewView`**

Give focus state precedence over loading current problem. Freeze/reset the review clock while the board is active. On a correct selection call `reviewFocusSelect`; on null focus show completion, then load the current Rust-selected problem. After a successful rating, consume `ReviewSubmission.focus`; if present, show the board instead of incrementing into/decrypting the next card. Skip follows the same persisted transition.

- [x] **Step 5: Run focused UI tests**

Run:

```powershell
pnpm vitest run src/modules/review/components/SchulteFocus.test.ts src/app/views/ReviewView.test.ts
pnpm typecheck
pnpm lint
```

Expected: all commands PASS.

- [x] **Step 6: Create a local checkpoint**

```powershell
git add src/modules/review/components/SchulteFocus.vue src/modules/review/components/SchulteFocus.test.ts src/app/views/ReviewView.vue src/app/views/ReviewView.test.ts
git commit -m "feat: add Schulte focus interludes"
```

### Task 6: Document, review, and verify the vertical slice

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/review.md`
- Create: `docs/windows-focus-acceptance.md`
- Modify: `docs/superpowers/plans/2026-07-19-schulte-focus-interlude.md`

**Interfaces:**
- Documents focus-policy snapshotting, fail-closed board transitions, exam exclusion, and manual Windows acceptance.

- [x] **Step 1: Document architecture and Windows acceptance**

Acceptance must cover each policy, due/manual sessions, exam exclusion, restart midway through a board, wrong-number feedback, correct-number persistence, skip, card-ten insertion, keyboard arrows/Enter, 760 px layout, 1280 px layout, and reduced motion.

- [x] **Step 2: Run complete quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
pnpm tauri build
```

Expected: every command PASS. Record the existing OpenSSL missing-PDB linker warning and SQLCipher `VirtualLock` warning separately if they recur; neither may hide a failing exit code.

- [x] **Step 3: Perform visual and interaction QA**

Use the development preview plus a real Tauri session to inspect settings, a new focus board, a resumed board, wrong feedback, completion transition, and the following review card at 1280×900 and 760×900. Assert `scrollWidth === clientWidth`, no clipped number, readable next-number hierarchy, stable focus, and no motion when reduced motion is enabled.

- [x] **Step 4: Self-review the complete diff**

Inspect every SQL transition and command boundary for cross-account/profile access, stale-state mutation, rating during focus, accidental exam insertion, answer leakage, generated-binding drift, and unstaged unrelated files. Fix every confirmed issue and rerun the affected focused plus full gate.

- [x] **Step 5: Mark plan checkboxes and create the final local checkpoint**

## Verification record

- `pnpm lint`, `pnpm typecheck`, `pnpm test` (32 files / 127 tests), and
  `pnpm build` passed on 2026-07-20.
- `cargo test --all-targets` passed, including 14 review-store tests and the v8
  migration/backup suites.
- `pnpm tauri build` produced the release executable successfully. The existing
  OpenSSL missing-PDB linker warning and SQLCipher `VirtualLock` warning remained
  non-fatal and did not hide a failing exit code.
- Browser QA passed at 390×844 with 25 readable tiles, 44 px actions, no horizontal
  overflow, no target-location highlight, correct local wrong-number feedback, and
  no console errors. The viewport override was reset afterward.

If task commits were used, keep them local and do not squash without user direction. Commit the documentation/checklist with:

```powershell
git add docs/architecture.md docs/plans/review.md docs/windows-focus-acceptance.md docs/superpowers/plans/2026-07-19-schulte-focus-interlude.md
git commit -m "docs: verify Schulte focus workflow"
```

Report every new local SHA and state explicitly that none was pushed.
