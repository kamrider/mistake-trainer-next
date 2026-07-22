# Safe Learner Profile Deletion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow a learner to permanently delete any non-last private learning profile without leaving the desktop runtime, encrypted blobs, or cloud replicas in an inconsistent state.

**Architecture:** Deletion is a single local SQLite transaction that selects a deterministic replacement profile, removes the target profile through existing foreign-key cascades, records an account-scoped learner-profile tombstone, and returns orphan encrypted-blob paths for post-commit cleanup. The Tauri command serializes profile transitions with LAN capture, activates the replacement in memory, and the Vue shell exposes a typed-name confirmation step. Supabase stores profile tombstones without a profile foreign key, cascades the remote profile, and pull applies the same deletion while preserving at least one profile.

**Tech Stack:** Rust stable, rusqlite/SQLCipher, Tauri 2 + tauri-specta, Vue 3 + TypeScript strict, Vitest/Testing Library, Supabase Postgres/pgtap.

## Global Constraints

- A local account must always retain at least one learner profile.
- A forged account/profile ID must not delete, rename, switch, or reveal another account's data.
- Deletion is irreversible in v1 and requires typing the exact profile name; the UI must say that questions, history, drafts, and export snapshots are removed.
- Stop the active LAN capture session before deleting or switching the active profile.
- Database mutation and outbox/tombstone creation are atomic; filesystem cleanup happens only after commit and is idempotent.
- Never remove an asset row or encrypted blob while any `problem_assets` or `capture_items` row still references it.
- Profile deletion outbox operations precede orphan-asset deletion operations.
- Ordinary motion uses only opacity and transform, honors `prefers-reduced-motion`, and keeps all controls keyboard reachable.

---

### Task 1: Atomic local profile deletion and orphan discovery

**Files:**
- Modify: `src-tauri/src/modules/profiles.rs`
- Test: `src-tauri/tests/profile_store.rs`

**Interfaces:**
- Consumes: `learner_profiles`, `account_preferences`, cascaded profile-owned tables, account-wide `assets`.
- Produces: `DeleteProfile { account_id, profile_id, now_utc_ms }`, `DeleteProfileReceipt { deleted_profile_id, replacement_profile, orphan_assets }`, and `delete_profile(&mut Connection, DeleteProfile)`.

- [x] **Step 1: Write failing store tests**

  Cover deletion of an inactive profile, deletion of the active profile with deterministic oldest replacement, refusal to delete the last profile, forged-account refusal, cascading problems/reviews/drafts/exports, and preservation of a deduplicated asset still referenced by another profile. Assert that a `learner_profile/delete` outbox row and account-scoped tombstone are committed together.

- [x] **Step 2: Run the focused Rust test**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test profile_store`
  Expected: FAIL because `DeleteProfile`, `DeleteProfileReceipt`, and `delete_profile` do not exist.

- [x] **Step 3: Implement the transaction**

  Add `ProfileUseCaseError::LastProfile`, `DeleteProfile`, `OrphanAsset`, and `DeleteProfileReceipt`. In `delete_profile`, load the owned target and all owned profiles, reject a missing/last target, select the oldest other profile, update `account_preferences`, collect candidate asset IDs/paths, delete stale target-profile sync operations/conflicts, delete the target profile, insert a tombstone with `profile_id = NULL`, insert the learner-profile delete outbox row, then delete only candidate asset rows for which neither link table contains a reference. Return deleted orphan paths only after `transaction.commit()`.

- [x] **Step 4: Run store tests and format**

  Run the focused test above, then `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check`.
  Expected: PASS.

- [x] **Step 5: Commit the store slice**

  Commit message: `feat: delete learner profiles atomically`.

### Task 2: Runtime-safe typed command

**Files:**
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/commands/profiles.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/profile_command.rs`
- Test: `src-tauri/tests/command_contract.rs`

**Interfaces:**
- Produces: `ProfileDeleteInput { profile_id: String, confirmation_name: String }` and `profile_delete(input) -> AppResult<ProfileOverview>`.

- [x] **Step 1: Write failing command tests**

  Assert exact-name confirmation, last-profile rejection, LAN stop before mutation, active replacement in both database and `LibraryRuntime`, encrypted orphan file cleanup after commit, and stable errors `profile_delete_confirmation_mismatch`, `profile_last_cannot_delete`, and `profile_not_found`.

- [x] **Step 2: Run focused command/contract tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test profile_command --test command_contract`.
  Expected: FAIL because `profile_delete` is not registered.

- [x] **Step 3: Implement orchestration**

  Under `runtime.lock_profile_transition()`, validate the exact current name, stop `CaptureLanManager`, call `delete_profile`, remove returned blob paths only when `safe_asset_path` keeps them below `blob_root`, then replace the in-memory active profile with the receipt's replacement. File removal failures are logged with a diagnostic ID but do not roll back the already-safe database transaction.

- [x] **Step 4: Register and generate bindings**

  Add `profile_delete` to Tauri invoke registration and specta export, then run `pnpm bindings:generate`.

- [x] **Step 5: Run focused tests**

  Expected: all profile and command contract tests PASS.

- [x] **Step 6: Commit the command slice**

  Commit message: `feat: expose safe profile deletion command`.

### Task 3: Cloud profile tombstone contract

**Files:**
- Create: `supabase/migrations/202607220001_profile_deletion.sql`
- Modify: `src-tauri/src/modules/sync_pull.rs`
- Modify: `src-tauri/src/commands/sync.rs`
- Test: `supabase/tests/sync_contract.sql`
- Test: `src-tauri/tests/sync_pull.rs`

**Interfaces:**
- Consumes: `WireTombstone { profile_id: Option<String>, entity_type: "learner_profile" }`.
- Produces: account-scoped profile tombstones that survive profile-row cascade and pull-side profile deletion with deterministic replacement.

- [x] **Step 1: Add failing pgtap and Rust pull tests**

  Prove that deleting one of two profiles cascades its cloud children, retains a tombstone, refuses deletion of the last cloud profile, is idempotent, rejects cross-account IDs, and makes a second local device replace its active profile without deleting shared assets.

- [ ] **Step 2: Run Supabase and focused Rust tests** *(Rust passes; Supabase is blocked locally because Docker Engine is unavailable)*

  Run: `pnpm supabase:test` and `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test sync_pull`.
  Expected: FAIL on learner-profile tombstone handling.

- [x] **Step 3: Add the database migration**

  Drop the tombstone profile foreign key, allow `profile_id NULL` for learner-profile tombstones, enforce a check that only `learner_profile` tombstones may have a null profile, and replace `push_sync_batch` so learner-profile delete inserts the account-scoped tombstone before deleting the owned profile. Lock the account's profile rows and reject deletion when only one remains. Repeated operation IDs return their prior acknowledgement.

- [x] **Step 4: Apply pull deletion locally**

  Extend `apply_tombstone` to update `account_preferences`, cascade-delete the target profile only when another owned profile exists, and report the replacement ID. After pull commit, refresh `LibraryRuntime` from the persisted preference so Vue never observes a deleted active ID.

- [ ] **Step 5: Run sync tests** *(Rust sync tests pass; pgtap remains an external environment gate)*

  Expected: Supabase contract, sync store, sync push, and sync pull tests PASS.

- [x] **Step 6: Commit the sync slice**

  Commit message: `feat: synchronize learner profile deletion`.

### Task 4: Confirmation UX, animation, and accessibility

**Files:**
- Modify: `src/modules/profiles/components/ProfileSwitcher.vue`
- Modify: `src/modules/profiles/components/ProfileSwitcher.test.ts`
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: generated `commands.profileDelete({ profileId, confirmationName })`.
- Produces: `delete(profileId, confirmationName)` component event and a closed, refreshed shell after success.

- [x] **Step 1: Write failing Vue tests**

  Assert that each non-last profile exposes a labelled delete action, the last profile does not, the confirmation view lists removed data, Save/Delete stays disabled until the exact name is typed, Escape cancels and restores focus, busy state prevents duplicates, and success returns to the dashboard with the replacement profile.

- [x] **Step 2: Run focused Vue tests**

  Run: `pnpm test -- src/modules/profiles/components/ProfileSwitcher.test.ts src/app/App.profile.test.ts`.
  Expected: FAIL because no delete event or command exists.

- [x] **Step 3: Implement the confirmation flow**

  Add `Trash2` beside rename, a `delete` mode with the profile name rendered in a copyable chip, an exact-name input, irreversible warning, Cancel and “永久删除” actions, and `aria-describedby` wiring. Keep the popover open on command failure and close it only after a successful overview is applied.

- [x] **Step 4: Add restrained motion**

  Transition list-to-confirmation with 180 ms opacity plus horizontal transform; animate row removal with 180 ms opacity/scale; use cinnabar only for the destructive action. Under reduced motion, remove transform and set transitions to 1 ms.

- [x] **Step 5: Wire shell orchestration**

  Propagate the delete event through `AppShell`; in `App.vue`, call `mutateProfile(() => commands.profileDelete(...), true)` and return to the dashboard after success.

- [x] **Step 6: Run focused Vue tests**

  Expected: PASS.

- [x] **Step 7: Commit the UI slice**

  Commit message: `feat: add guided profile deletion`.

### Task 5: Documentation and release gates

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/foundation.md`
- Create: `docs/windows-profile-deletion-acceptance.md`

- [x] **Step 1: Document invariants and manual matrix**

  Include inactive deletion, active deletion, drafts, shared image dedupe, LAN session, offline outbox, two-device pull, last-profile refusal, incorrect confirmation, restart, and backup/restore cases.

- [ ] **Step 2: Run full gates**

  Run: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm bindings:check`, `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`, `pnpm supabase:test`, and `pnpm tauri build`.
  Expected: all PASS; non-fatal SQLCipher `VirtualLock` and OpenSSL PDB warnings may remain documented.

- [ ] **Step 3: Perform Windows visual review**

  Verify the profile popover at desktop width, keyboard-only focus order, high contrast, reduced motion, long Chinese names, and deletion of active/inactive profiles in the packaged Tauri application.

- [ ] **Step 4: Commit the verified feature**

  Commit message: `docs: verify learner profile deletion`.
