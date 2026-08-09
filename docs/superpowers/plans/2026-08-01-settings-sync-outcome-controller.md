# Settings Sync Outcome Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent a successful manual synchronization from being reported as failed when only the supplementary settings overview or conflict-list refresh fails.

**Architecture:** Add a focused Vue composable that owns manual-sync admission, primary result copy, and best-effort post-sync view refreshes. The primary `AppResult<SyncNowReport>` is authoritative; overview and conflict refreshes may append precise “暂时没有刷新” guidance but can never revoke a successful sync. `SettingsView` supplies the existing global/native sync adapter, overview application callback, and DOM-aware conflict refresh callback.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, existing global sync controller.

## Global Constraints

- Preserve the exact sync command selection: use the injected global controller when available and never issue a duplicate native `syncNow` call.
- Preserve exact success counts and primary command error copy.
- Admit only one page-level manual sync at a time.
- Treat settings overview and conflict-list refreshes as supplementary; failures must name the stale view without claiming synchronization failed.
- Disable top-level settings refresh while manual sync is active.
- Do not change global synchronization semantics, conflict resolution, backend selection, authentication, storage/device migration, updater recovery, account deletion, Rust, bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize primary and supplementary outcomes

**Files:**

- Create: `src/app/composables/useSettingsSyncOperations.test.ts`

**Interfaces:**

- Consumes: internal `AppResult<T>`, generated `SyncNowReport`, and `SettingsOverview`.
- Produces: executable requirements for `useSettingsSyncOperations(options)`.

- [x] **Step 1: Add a typed harness**

Supply mocked `sync`, `refreshOverview`, `applyOverview`, and `refreshConflicts` callbacks with complete report/overview fixtures and a deferred primary result helper.

- [x] **Step 2: Cover authoritative success**

Assert successful sync renders `同步完成：上传 1 项，拉取 2 项。`, applies refreshed overview, and refreshes conflicts once.

- [x] **Step 3: Cover supplementary failures independently**

When overview throws or returns failure, retain success counts and append `顶部资料库统计暂时没有刷新`; when conflict refresh returns false or throws, append `同步冲突列表暂时没有刷新`; when both fail, name both stale surfaces without the phrase `同步请求没有完成`.

- [x] **Step 4: Cover primary failure and single admission**

Assert command errors use their user message, thrown primary calls use the existing retry copy, supplementary callbacks are skipped on primary failure, and a second call is rejected while the first is pending.

- [x] **Step 5: Run the new suite red**

Run:

```powershell
npm test -- --run src/app/composables/useSettingsSyncOperations.test.ts
```

Expected: import-resolution failure because `useSettingsSyncOperations.ts` does not exist.

### Task 2: Implement the outcome controller

**Files:**

- Create: `src/app/composables/useSettingsSyncOperations.ts`
- Test: `src/app/composables/useSettingsSyncOperations.test.ts`

**Interfaces:**

- Consumes:

```ts
sync: () => Promise<AppResult<SyncNowReport>>
refreshOverview: () => Promise<AppResult<SettingsOverview>>
applyOverview: (overview: SettingsOverview) => void
refreshConflicts: () => Promise<boolean>
```

- Produces readonly `busy`, readonly `message`, and `syncNow(): Promise<boolean>`.

- [x] **Step 1: Implement authoritative primary handling**

Set busy before invoking `sync`, reject duplicates, use the exact command error or fallback copy on failure, and return `false` without invoking supplementary callbacks.

- [x] **Step 2: Implement isolated supplementary refreshes**

After primary success, call overview and conflicts in separate `try/catch` blocks. Apply only successful overview data. Collect failed surface labels and append one precise warning sentence to the successful counts.

- [x] **Step 3: Run controller tests green**

Run the Task 1 command and expect every case to pass.

### Task 3: Integrate SettingsView and prove the user path

**Files:**

- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Verify: `src/app/components/SettingsCloudAuthPanel.vue`
- Verify: `src/modules/sync/components/SyncConflictCenter.vue`

**Interfaces:**

- Consumes: the Task 2 controller and existing panel props/emits.
- Produces: unchanged `syncBusy`, `syncMessage`, and `sync` event presentation contract.

- [x] **Step 1: Add successful-sync/supplementary-failure regression**

Provide a successful injected global sync result, make the post-sync `settingsOverview` call throw, click `立即同步`, and assert the UI reports uploaded/pulled counts plus stale overview guidance and never shows `同步请求没有完成`.

- [x] **Step 2: Add pending-sync refresh regression**

Hold the global sync result pending and assert top-level `刷新` is disabled. Resolve success and assert it re-enables.

- [x] **Step 3: Replace inline sync orchestration**

Adapt global/native sync into `AppResult<SyncNowReport>`, normalize overview refresh, wait one Vue tick before calling the exposed conflict reload, and replace the view-local `syncNow` body with the controller action.

- [x] **Step 4: Run focused and adjacent tests**

Run:

```powershell
npm test -- --run src/app/composables/useSettingsSyncOperations.test.ts src/app/views/SettingsView.test.ts src/modules/sync/components/SyncConflictCenter.test.ts src/modules/sync/composables/useSyncConflictOperations.test.ts src/app/components/SettingsCloudAuthPanel.test.ts
```

Expected: all existing and new synchronization interactions pass.

### Task 4: Commercial-quality gates and local review

**Files:**

- Verify: all files in Tasks 1–3
- Modify: `docs/superpowers/plans/2026-08-01-settings-sync-outcome-controller.md`

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`. Expect zero warnings/errors, a successful production build, and the complete suite to pass.

- [x] **Step 2: Review outcome truthfulness**

Confirm primary success cannot be revoked by either supplementary refresh, failed surfaces are named precisely, duplicates are blocked, global/native command selection is unchanged, and page refresh is disabled only while necessary.

- [x] **Step 3: Check workspace hygiene**

Run target/global whitespace checks, confirm the index is empty, confirm build output remains ignored, and confirm `recognition_visual_split.rs` remains an untouched pre-existing modification.

- [x] **Step 4: Record evidence below**

Record baseline, red, green, focused integration, final gates, review, and hygiene results.

## Verification record

- Baseline: Settings view, conflict center, and conflict operations passed, 3 files / 65 tests.
- Controller red: the new suite failed at import resolution because `useSettingsSyncOperations.ts` did not exist.
- Interaction red: a successful global sync followed by a thrown overview refresh never produced truthful success guidance, and `刷新` remained enabled while manual sync was pending.
- Controller green: 1 file / 6 tests passed; TypeScript passed before page integration.
- Focused integration: controller, Settings view, cloud auth panel, conflict center, and conflict operations passed, 5 files / 77 tests; ESLint passed.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed; mobile vendor verification and the production Vite build passed; serial full Vitest passed, 129 files / 715 tests.
- Review: the 72-line controller has one authoritative primary-sync branch and two isolated supplementary refresh branches. Command failure skips both refreshes; success always retains uploaded/pulled counts; stale overview/conflict surfaces are named individually or together. `SettingsView.vue` is 1,021 lines and contains only the global/native adapter, overview callback, and next-tick conflict callback for this workflow.
- Preserved behavior: injected global sync remains preferred, native `syncNow` remains the sole fallback, no duplicate native call occurs, exact primary success counts and command error messages remain, conflict resolution and backend selection are unchanged, and the refresh button is disabled only for existing page operations plus active manual sync.
- Hygiene: target whitespace checks passed; nothing was staged or committed; production output remained ignored; no generated source artifact entered status; `recognition_visual_split.rs` remains an untouched pre-existing modification; excluded launch-gate work was not changed.
