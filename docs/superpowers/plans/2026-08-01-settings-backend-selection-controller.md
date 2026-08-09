# Settings Backend Selection Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent stale sync-backend status reads or concurrent refreshes from visually undoing a backend selection that the user has already completed.

**Architecture:** Add a focused Vue composable that exclusively owns backend status, selection admission, user copy, and request revisions. A selection invalidates every earlier status read; status refresh is refused while a selection is pending. `SettingsView` supplies the existing API adapters and presents the controller state through the existing backend panel.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue.

## Global Constraints

- Preserve local-first behavior and the generated `CloudBackendKind` / `CloudBackendStatus` contracts.
- Never select the unavailable Tencent adapter, never issue duplicate selection commands, and retain command-provided user messages.
- A successful selection is authoritative over status reads that began earlier.
- Disable top-level settings refresh while backend selection is pending.
- Do not change synchronization semantics, authentication, storage/device migration, updater recovery, account deletion, Rust, bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize status and selection ordering

**Files:**

- Create: `src/app/composables/useSettingsBackendSelection.test.ts`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: Add controller tests for stale-read rejection**

Start a deferred status read, complete a Supabase selection, then resolve the old read as local-only. Assert Supabase remains selected.

- [x] **Step 2: Cover admission and failure copy**

Assert duplicate selections, the current backend, and Tencent are inert; command failures retain their user message and thrown commands use the local-data-safe fallback.

- [x] **Step 3: Add pending-selection page regression**

Hold backend selection pending and assert both backend choices and top-level refresh are disabled until completion.

- [x] **Step 4: Run the new tests red**

Expect the controller import to fail and the page-level refresh assertion to fail before implementation.

### Task 2: Implement the backend selection controller

**Files:**

- Create: `src/app/composables/useSettingsBackendSelection.ts`
- Test: `src/app/composables/useSettingsBackendSelection.test.ts`

- [x] **Step 1: Centralize state ownership**

Expose readonly `status`, `busy`, and `message`, plus `loadStatus()` and `choose(kind)` actions.

- [x] **Step 2: Enforce ordering**

Use load and selection revisions so only the newest valid read may apply, and a selection invalidates all reads that began before it.

- [x] **Step 3: Preserve interaction truthfulness**

Keep the previous status on selection failure, announce exact success/failure copy, reject unsupported or duplicate intentions, and always clear busy in `finally`.

- [x] **Step 4: Run controller tests green**

Run the Task 1 controller suite and TypeScript.

### Task 3: Integrate SettingsView

**Files:**

- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Verify: `src/app/components/SettingsSyncBackendPanel.vue`
- Verify: `src/app/components/SettingsSyncBackendPanel.test.ts`

- [x] **Step 1: Replace direct shared-state writes**

Adapt `loadSyncBackendStatus`, `setSyncBackend`, and `backendKindLabel` into the controller; remove the inline backend refs and selection function.

- [x] **Step 2: Serialize page refresh with selection**

Use the controller load action in page load and include `backendBusy` in the top-level refresh disabled state.

- [x] **Step 3: Run focused and adjacent tests**

Run controller, Settings view, and backend panel tests.

### Task 4: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review ownership and race behavior**

Confirm no direct backend-state writer remains in `SettingsView`, stale reads cannot overwrite selection, and UI admission mirrors controller admission.

- [x] **Step 3: Check workspace hygiene and record evidence**

Confirm target whitespace, empty index, ignored build output, and untouched pre-existing recognition changes. Record baseline, red, green, final gates, and review below.

## Verification record

- Baseline: backend panel and Settings view passed, 2 files / 56 tests.
- Red: the controller suite failed at import resolution; the Settings interaction test showed top-level refresh remained enabled while backend selection was pending.
- Controller and integration green: controller, Settings view, and backend panel passed, 3 files / 63 tests.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed; mobile vendor verification and production Vite build passed; serial full Vitest passed, 130 files / 722 tests.
- Review: the 83-line controller is now the only writer for backend status, busy state, and user copy. Each selection advances both selection and load revisions, so an earlier read cannot apply after a selection starts; only the newest concurrent read can apply; reads and duplicate selections are refused during a pending selection. Success copy uses the backend actually returned by the command, while failures retain prior status and exact command guidance.
- Integration: `SettingsView.vue` is 1,005 lines and only provides API adapters plus panel bindings for this workflow. Page load calls the controller action, and top-level refresh now shares the same pending-selection admission boundary as backend choices.
- Hygiene: target whitespace checks passed; nothing was staged or committed; production output remained ignored; `recognition_visual_split.rs` remains an untouched pre-existing modification; excluded launch-gate work was not changed.
