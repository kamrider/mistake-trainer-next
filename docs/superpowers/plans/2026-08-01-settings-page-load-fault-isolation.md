# Settings Page Load Fault Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep every independent settings section usable when one status or preference read fails, while centralizing refresh admission, loading state, stale-preference protection, and truthful partial-failure copy.

**Architecture:** Add a domain-focused Vue composable that runs backend status plus independent desktop settings reads through isolated tasks. Overview, subject preferences, and review preferences retain typed `AppResult` handling; supplementary probes keep their panel-local errors. The controller aggregates only failed section names, applies preferences only when their revision still matches the load snapshot, and never lets one rejected task cancel sibling sections.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, existing typed Tauri bindings.

## Global Constraints

- Preserve the existing unsaved-preference refresh guard and its exact copy.
- Preserve subject and review revision checks so an in-flight refresh cannot overwrite edits made after refresh began.
- A failed overview, subject, review, account restore, or supplementary task must not prevent any sibling task from running or applying successful data.
- Report partial failures as partial failures; never claim the whole settings page is unavailable when unaffected sections loaded.
- Browser preview must load backend status, skip every desktop command, and retain the existing storage-preview guidance.
- Do not change synchronization semantics, backend selection, authentication mutations, storage/device migration, updater failure recovery, account deletion, Rust, bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize independent load outcomes

**Files:**

- Create: `src/app/composables/useSettingsPageLoad.test.ts`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**

- Consumes typed `AppResult<SettingsOverview>`, `AppResult<SubjectPreferences>`, and `AppResult<ReviewPreferences>` operations plus named supplementary tasks.
- Produces executable requirements for `useSettingsPageLoad(options)`.

- [x] **Step 1: Add a controller harness with deferred and rejecting tasks**

Create complete settings/preference fixtures, mutable subject/review revisions, apply spies, browser detector, backend/session spies, and named supplementary task spies.

- [x] **Step 2: Prove sibling fault isolation and truthful copy**

Make overview throw while subject/review and all supplementary reads succeed. Assert successful preferences apply, every sibling runs once, loading clears, and the message names only `资料库概览` plus the fact that other settings remain usable.

- [x] **Step 3: Prove typed failures, stale guards, duplicate admission, and browser mode**

Assert one typed failure retains its exact `userMessage`; multiple failures use an aggregated section list; revision changes suppress only the stale preference apply; concurrent `load()` calls are rejected; browser mode skips all desktop tasks.

- [x] **Step 4: Add a page-level partial-load regression**

Make `settingsOverview` reject during mount and assert the subject panel, review panel, OCR panel, and storage panel still render or receive their successful data while the global alert names only the unavailable overview.

- [x] **Step 5: Run the new suites red**

Run `npm test -- --run src/app/composables/useSettingsPageLoad.test.ts src/app/views/SettingsView.test.ts`. Expect controller import resolution to fail and the page regression to show later commands were skipped.

### Task 2: Implement the page-load controller

**Files:**

- Create: `src/app/composables/useSettingsPageLoad.ts`
- Test: `src/app/composables/useSettingsPageLoad.test.ts`

**Interfaces:**

- Consumes `errorMessage: Ref<string>`, refresh guard callback, runtime detector, browser callback, revision snapshot/current callbacks, backend/session operations, three typed primary reads, three apply callbacks, and named supplementary tasks.
- Produces readonly `loading` and `load(): Promise<boolean>`.

- [x] **Step 1: Implement refresh admission and lifecycle**

Initialize loading for first paint, reject duplicate or guarded refreshes, clear prior page errors on accepted load, and restore loading in `finally`.

- [x] **Step 2: Implement isolated task execution**

Run all desktop tasks concurrently behind independent `try/catch` boundaries. Convert failed `AppResult` reads into named failures without preventing sibling execution.

- [x] **Step 3: Implement revision-aware application and error aggregation**

Apply overview immediately on success; apply subject/review data only if the corresponding current revision equals the snapshot. Preserve the sole typed error copy when exactly one typed task fails; otherwise name unique failed sections and state that other settings remain usable.

- [x] **Step 4: Implement browser boundary**

Load backend state, invoke browser-preview guidance, skip desktop tasks, and complete successfully without a desktop failure banner.

- [x] **Step 5: Run controller tests green and typecheck**

Run the controller suite and `npm run typecheck`.

### Task 3: Integrate SettingsView

**Files:**

- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**

- Consumes the Task 2 controller and existing API/panel adapters.
- Produces the existing `loading`, `errorMessage`, and `load` template contract with fault-isolated behavior.

- [x] **Step 1: Replace the inline load function**

Wire backend/session operations, typed overview/preference reads, revision callbacks, browser guidance, and existing supplementary loaders into `useSettingsPageLoad`; remove the sequential page-local load body.

- [x] **Step 2: Preserve exact page interactions**

Keep the refresh button binding, on-mount load, dirty-preference guard, and all panel-local status/error states unchanged.

- [x] **Step 3: Run focused and adjacent tests**

Run page loader, Settings view, queued preference save, backend selection, cloud session, and OCR management tests.

### Task 4: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review fault boundaries and ownership**

Confirm `SettingsView` no longer owns sequential load orchestration, every independent read has an isolated outcome, exact single errors are preserved, multiple failures remain truthful, and stale drafts cannot be overwritten.

- [x] **Step 3: Check workspace hygiene and record evidence**

Run target whitespace checks, confirm the index is empty, confirm build output remains ignored, and verify the pre-existing recognition modification was untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Baseline: Settings view, backend selection, and cloud session passed, 3 files / 66 tests.
- Red: the controller suite failed at import resolution; the Settings regression received the old whole-page failure copy after `settingsOverview` rejected, and later section commands were skipped.
- Controller and page green: page-load controller plus Settings view passed, 2 files / 61 tests.
- Focused integration: page load, Settings view, preference save queue, backend selection, cloud session, and OCR management passed, 6 files / 92 tests; TypeScript passed.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed both directly and through the production build; mobile vendor verification and Vite production build passed; serial full Vitest passed, 131 files / 729 tests.
- Review: the 134-line controller owns refresh admission and loading lifecycle, executes backend/account/overview/preferences/supplementary tasks through isolated settlement boundaries, applies successful siblings, preserves the sole typed error copy, aggregates multiple or thrown failures by unique section name, and always clears in-flight state. Subject and review applies compare live revisions with the load snapshot independently.
- Integration: `SettingsView.vue` is 984 lines, no longer contains the sequential page-load `try/catch`, and exposes only typed command adapters, state application callbacks, browser guidance, and named supplementary tasks. The existing refresh button, first-mount load, dirty guard, backend selection ordering, and panel-local errors remain intact.
- Hygiene: target whitespace checks passed; nothing was staged or committed; production output remained ignored; `recognition_visual_split.rs` remains an untouched pre-existing modification; excluded launch-gate work was not changed.
