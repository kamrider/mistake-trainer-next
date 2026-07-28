# Persistent Multi-Profile Switcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inert, hard-coded learner badge with a persistent multi-profile workflow that can list, create, rename, and switch private learner profiles without leaking data or active phone-capture sessions across profiles.

**Architecture:** Schema v6 stores the device-local active profile for the current account. `LibraryRuntime` owns a lock-protected active-profile snapshot; every command copies that snapshot before touching the database, and profile switching updates persistence and memory as one runtime operation. A dedicated Vue `ProfileSwitcher` owns the popover interaction while `App.vue` owns typed commands, navigation, and route remounting.

**Tech Stack:** Rust 1.97, rusqlite/SQLCipher, Tauri 2.11, tauri-specta, Vue 3 strict TypeScript, Testing Library, Vitest.

## Global Constraints

- Windows v1 remains offline-first and profile switching must not require Supabase.
- Every profile mutation and read is scoped by `account_id`; arbitrary profile IDs from Vue are never trusted.
- An active LAN capture session must stop before the active profile changes.
- Switching profile remounts the current feature route so no Vue state from the previous profile survives.
- Profile create and rename use the existing `ProfileName` validation and return public `AppResult<T>` errors without internal SQL or paths.
- Profile deletion is intentionally excluded from this slice because it needs a separate asset/blob garbage-collection and recovery contract; no destructive placeholder control is shown.
- Motion uses only opacity and transform, 180–240 ms, and is disabled under `prefers-reduced-motion: reduce`.

---

### Task 1: Schema v6 active-profile persistence

**Files:**
- Create: `src-tauri/migrations/0006_account_preferences.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produces table `account_preferences(account_id, active_profile_id, updated_at_utc_ms)`.
- `active_profile_id` references `learner_profiles(id)` with `ON DELETE RESTRICT`.
- Produces schema version `6`; versions `0..=5` upgrade transactionally.

- [ ] **Step 1: Write failing migration and backup tests**

Assert a fresh database contains `account_preferences` at `user_version = 6`; a v5 database upgrades without altering profiles/problems/preferences; backup validation rejects a schema-v6 package missing `account_preferences` and rejects a foreign `account_id` row.

- [ ] **Step 2: Run the focused tests and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test database_schema --test backup_store`

Expected: failures mention version `5` or missing `account_preferences` behavior.

- [ ] **Step 3: Implement the strict migration and validators**

```sql
CREATE TABLE account_preferences (
    account_id TEXT PRIMARY KEY NOT NULL,
    active_profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE RESTRICT,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;
```

Set `CURRENT_SCHEMA_VERSION` to `6`. In `ensure_single_account`, require the table exactly for schema `>= 6`, reject foreign account rows, and verify the referenced profile belongs to the same account.

- [ ] **Step 4: Run focused tests and verify pass**

Run the command from Step 2. Expected: all schema and backup tests pass.

---

### Task 2: Profile list, rename, and active selection use cases

**Files:**
- Modify: `src-tauri/src/modules/profiles.rs`
- Modify: `src-tauri/tests/profile_store.rs`

**Interfaces:**
- Produces `list_profiles(connection, account_id) -> Result<Vec<LearnerProfile>, ProfileUseCaseError>`.
- Produces `rename_profile(connection, RenameProfile) -> Result<LearnerProfile, ProfileUseCaseError>`.
- Produces `persist_active_profile(connection, account_id, profile_id, now_utc_ms) -> Result<LearnerProfile, ProfileUseCaseError>`.
- Adds `ProfileUseCaseError::NotFound` and `ProfileUseCaseError::DuplicateName` with stable matching before public error mapping.

- [ ] **Step 1: Write failing profile-store tests**

Cover deterministic list order, cross-account isolation, trimmed rename with revision/outbox atomicity, duplicate-name rejection with no partial update, active selection persistence, and forged/cross-account profile IDs.

- [ ] **Step 2: Run the focused test and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test profile_store`

Expected: compile failure for the new functions and types.

- [ ] **Step 3: Implement scoped profile operations**

Use `ORDER BY created_at_utc_ms, id`; update rename with `revision = revision + 1`, serialize the returned row, and write its `learner_profile/upsert` outbox row in the same transaction. Persist selection with an ownership-checked `INSERT ... SELECT ... ON CONFLICT(account_id) DO UPDATE` and return `NotFound` when zero rows are affected.

- [ ] **Step 4: Run the focused test and verify pass**

Run the command from Step 2. Expected: all profile-store tests pass.

---

### Task 3: Mutable runtime active-profile boundary

**Files:**
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: all files under `src-tauri/src/commands/` that call `profile_id()` or `profile_name()`
- Modify: `src-tauri/tests/runtime_state.rs`
- Modify: affected command tests under `src-tauri/tests/`

**Interfaces:**
- Produces public `ActiveProfile { id: String, name: String }`.
- Produces `LibraryRuntime::active_profile() -> ActiveProfile` returning a clone without holding a lock across database work.
- Produces `LibraryRuntime::activate_profile(profile_id, now_utc_ms) -> Result<ActiveProfile, ProfileUseCaseError>` that locks selection, persists it, then changes memory.
- Initialization restores `account_preferences.active_profile_id`; invalid/missing preference falls back deterministically and writes the fallback.

- [ ] **Step 1: Write failing runtime tests**

Create two profiles, activate the second, drop/reopen runtime, and assert the second profile remains active. Assert a forged ID leaves both memory and persistence unchanged. Assert debug output redacts every active profile field.

- [ ] **Step 2: Run the runtime test and verify failure**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test runtime_state`

- [ ] **Step 3: Implement the runtime snapshot and refactor commands**

Replace immutable `profile_id/profile_name` strings with `RwLock<ActiveProfile>`. Each command starts with `let profile = runtime.active_profile();` and passes `profile.id.as_str()` or a clone to its use case; it never calls the accessor twice inside one command.

- [ ] **Step 4: Run all Rust command and runtime tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --all-targets`

Expected: all tests pass with no old-profile accessor remaining under `src-tauri/src/commands`.

---

### Task 4: Typed profile commands and LAN-session safety

**Files:**
- Create: `src-tauri/src/commands/profiles.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Create: `src-tauri/tests/profile_command.rs`
- Modify: `src/shared/api/bindings.ts`
- Modify: `src/shared/api/bindings.test.ts`

**Interfaces:**

```rust
pub struct ProfileOverview {
    pub active_profile_id: String,
    pub profiles: Vec<LearnerProfile>,
}

profile_list() -> AppResult<ProfileOverview>
profile_create(name: String) -> AppResult<ProfileOverview>
profile_rename(profile_id: String, name: String) -> AppResult<ProfileOverview>
profile_select(profile_id: String) -> AppResult<ProfileOverview>
```

- [ ] **Step 1: Write failing command-contract tests**

Assert runtime identity is always used; create selects the new profile; rename does not switch profiles; select stops an active `CaptureLanManager` session before changing runtime state; failed stop or invalid ID leaves selection unchanged; public errors distinguish invalid, duplicate, missing, and internal failures.

- [ ] **Step 2: Implement commands and register bindings**

`profile_select` and auto-selecting `profile_create` receive both `State<LibraryRuntime>` and `State<CaptureLanManager>`. Call `manager.stop()` before activation. Return the fresh overview after every mutation.

- [ ] **Step 3: Generate and verify bindings**

Run: `pnpm bindings:generate`

Expected: `commands.profileList/profileCreate/profileRename/profileSelect` and `ProfileOverview` exist; no account ID parameter is generated.

- [ ] **Step 4: Run command and binding tests**

Run: `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --test profile_command && pnpm test`

Expected: both suites pass.

---

### Task 5: Accessible animated profile switcher

**Files:**
- Create: `src/modules/profiles/components/ProfileSwitcher.vue`
- Create: `src/modules/profiles/components/ProfileSwitcher.test.ts`
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/AppShell.test.ts`

**Interfaces:**
- Consumes `profiles: LearnerProfile[]`, `activeProfileId`, `busy`, `errorMessage`.
- Emits `select(profileId)`, `create(name)`, and `rename({ profileId, name })`.
- AppShell forwards `select-profile`, `create-profile`, and `rename-profile`.

- [ ] **Step 1: Write failing interaction tests**

Cover open/close, selected profile announcement, single-click switch, inline create, explicit rename button (no double click), Enter submit, Escape cancel, busy disabling, error live region, and keyboard focus return.

- [ ] **Step 2: Implement the switcher**

Render a popover above the rail footer with one button per profile, selected checkmark, a small explicit rename control, and a “新建学习档案” form. Use `aria-expanded`, `aria-controls`, `role=status`, and labelled inputs. Do not show deletion.

- [ ] **Step 3: Add restrained motion**

Animate popover opacity/translate/scale for 180 ms, selected indicator for 120 ms, and profile-row hover for 120 ms. Add a complete reduced-motion override.

- [ ] **Step 4: Run component tests**

Run: `pnpm test`

Expected: all switcher and existing shell tests pass.

---

### Task 6: App orchestration and profile-safe route remount

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.test.ts`
- Modify: `src/app/views/DashboardView.test.ts`
- Modify: `docs/architecture.md`

**Interfaces:**
- `App.vue` calls `profileList()` after system status load.
- On successful create/select, it navigates to `dashboard`, increments `profileEpoch`, and keys `.route-page` with `${route.fullPath}:${profileEpoch}`.
- On rename, it updates the shell immediately without remounting unrelated page state unless the active profile changed.

- [ ] **Step 1: Write failing App integration tests**

Mock typed commands and assert the real learner name replaces “小树”; selecting another profile calls `profileSelect`, returns to dashboard, remounts the page, and updates the shell; failed switch keeps the previous active profile and exposes the server message; create and rename update the menu.

- [ ] **Step 2: Implement orchestration with stale-request protection**

Use a monotonically increasing request sequence for overview refresh. While a mutation is active, disable profile actions. Normalize every generated `AppResult`; on exceptions show a profile-specific reconnect message and keep the last confirmed active profile.

- [ ] **Step 3: Document the active-profile invariant**

Record persistence, command snapshotting, route remount, LAN-session stop, and the deliberate absence of destructive deletion until asset cleanup is transactional.

- [ ] **Step 4: Run full gates and visual acceptance**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, and `./scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --all-targets`.

Then inspect desktop and narrow layouts in the local browser. Expected: no overflow, no console errors, clear active state, and reduced-motion rules present.

- [ ] **Step 5: Review and commit**

Review the diff against this plan, commit as `feat: add persistent learner profile switching`, and keep the commit local until the user explicitly approves its GitHub push.
