# Export Selection Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing export snapshot generator into a transparent workflow where users choose due problems, the latest review batch, or all active problems, refine that selection, save an immutable snapshot, and regenerate the chosen layout.

**Architecture:** Rust exposes a profile-scoped, read-only export-candidate query with no review-session side effects. Vue keeps selection and presentation state, while snapshot validation and file generation remain authoritative Rust use cases. The report page delegates candidate picking and snapshot history rendering to focused components so that command orchestration stays readable.

**Tech Stack:** Rust, rusqlite/SQLCipher, serde, specta/tauri-specta, Vue 3, TypeScript strict, Vitest, Testing Library, Lucide, CSS transitions.

## Global Constraints

- Windows is the only v1 release platform.
- Vue never receives database handles, blob paths, arbitrary filesystem paths, or LAN tokens.
- A candidate query must not create, cancel, resume, or advance a review session.
- Export snapshots remain immutable, profile-scoped, ordered, and limited to 1 through 500 unique non-trashed problems.
- Generated files remain local and reproducible; only snapshot metadata enters the sync outbox.
- Motion uses transform and opacity, follows 120/180/240 ms tokens, and honors `prefers-reduced-motion`.
- No GitHub push is authorized by this plan.

---

### Task 1: Read-only export candidate contract

**Files:**
- Modify: `src-tauri/src/modules/exports.rs`
- Modify: `src-tauri/src/commands/exports.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/export_store.rs`
- Modify: `src/shared/api/bindings.ts` (generated)

**Interfaces:**
- Produces: `ExportCandidateSource::{Due, LatestReviewSession, AllActive}`.
- Produces: `ExportCandidate { id, subject, note, question_asset_count, answer_asset_count, due_at_utc_ms, review_count }`.
- Produces: `export_candidates(source) -> AppResult<Vec<ExportCandidate>>`.

- [ ] **Step 1: Write failing Rust tests for all candidate sources**

Create active problems with distinct schedules, insert an ordered completed review session, and assert:

```rust
let due = list_export_candidates(&connection, "account-1", &profile.id, ExportCandidateSource::Due, 100)?;
assert_eq!(due.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), vec![new_problem.id, due_problem.id]);

let recent = list_export_candidates(&connection, "account-1", &profile.id, ExportCandidateSource::LatestReviewSession, 100)?;
assert_eq!(recent.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), vec![second.id, first.id]);
```

Also assert trashed and foreign-profile problems are absent and that the query leaves `review_sessions` unchanged.

- [ ] **Step 2: Run the focused Rust test and observe the missing contract**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_store`

Expected: compilation fails because `ExportCandidateSource` and `list_export_candidates` do not exist.

- [ ] **Step 3: Implement deterministic read-only SQL queries**

Use one query for `Due`/`AllActive` and `json_each(problem_ids_json)` for the latest review session. Preserve source order, count assets by role, count review events, filter by account/profile, exclude trashed rows, and cap results at 500. `Due` must use the same rule as the review queue: no schedule state or `due_at_utc_ms <= now_utc_ms`.

- [ ] **Step 4: Add the typed Tauri command and generated binding**

Register:

```rust
#[tauri::command]
#[specta::specta]
pub fn export_candidates(
    state: State<'_, LibraryRuntime>,
    source: ExportCandidateSource,
) -> AppResult<Vec<ExportCandidate>>
```

Map candidate-read failures to `export_candidates_failed` with a retryable Chinese message. Run `pnpm bindings:generate` and verify the generated file is stable across a second generation.

- [ ] **Step 5: Run focused Rust and binding tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_store
pnpm vitest run src/shared/api/bindings.test.ts
```

Expected: all tests pass.

### Task 2: Candidate picker and explicit snapshot selection

**Files:**
- Create: `src/modules/export/components/ExportCandidatePicker.vue`
- Create: `src/modules/export/components/ExportCandidatePicker.test.ts`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: `ExportCandidate[]`, `ExportCandidateSource`, `selectedIds: string[]`, and `loading`.
- Produces events: `source(source)`, `toggle(problemId)`, `selectAll(problemIds)`, and `clear()`.

- [ ] **Step 1: Write picker tests before the component**

Verify source-tab selection, subject/note search, row checkbox toggling, select-visible, clear, ready/missing-answer badges, and empty states. Assert source selection is a normal single click and requires no double-click gesture.

- [ ] **Step 2: Implement the controlled picker**

Render three source cards:

```ts
const sourceOptions = [
  { value: 'due', label: '到期队列', hint: '新题和现在应该复习的题' },
  { value: 'latest_review_session', label: '最近训练批次', hint: '保留上一次训练的题目顺序' },
  { value: 'all_active', label: '全部活动题', hint: '从题库中手动筛选' },
] satisfies Array<{ value: ExportCandidateSource; label: string; hint: string }>
```

Filter locally over at most 500 candidates. Keep rows keyboard operable, animate only transform/opacity, and disable bulk actions while loading.

- [ ] **Step 3: Replace implicit all-active export in `ReportView`**

Load candidates for the default `due` source. On a source change, clear stale candidates, fetch the new source, then select every returned candidate in source order. Searching or toggling must not issue Rust commands. `createSnapshot()` must submit only `selectedIds` in candidate order and remain disabled when none are selected.

- [ ] **Step 4: Add report-view failure and recovery tests**

Cover candidate-load failure with an explicit retry button, source switching, deselecting one problem before save, and preserving the same selection after a failed `exportCreate` call.

- [ ] **Step 5: Run focused frontend tests**

Run:

```powershell
pnpm vitest run src/modules/export/components/ExportCandidatePicker.test.ts src/app/views/ReportView.test.ts
pnpm typecheck
```

Expected: all tests and strict type checking pass.

### Task 3: Snapshot history interaction and generation feedback

**Files:**
- Create: `src/modules/export/components/ExportSnapshotHistory.vue`
- Create: `src/modules/export/components/ExportSnapshotHistory.test.ts`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src-tauri/src/commands/exports.rs`

**Interfaces:**
- Consumes: active/deleted snapshot arrays plus `generatingId`, `deletingId`, and `restoringId`.
- Produces events: `generate(snapshot)`, `delete(snapshotId)`, and `restore(deletedSnapshot)`.

- [ ] **Step 1: Write history interaction tests**

Assert generated filenames are never rendered as paths, only the active row becomes busy, delete/restore buttons expose snapshot titles, and `TransitionGroup` insertion/removal keeps a stable list structure.

- [ ] **Step 2: Extract and polish snapshot history UI**

Use a compact paper-stack visual with layout labels, problem counts, creation/deletion dates, clear primary/secondary actions, 180 ms row motion, and reduced-motion overrides. Keep delete confirmation in the view orchestration layer.

- [ ] **Step 3: Validate a snapshot before opening the folder dialog**

Move `prepare_export(...)` before `pick_folder()` in `export_generate`. A deleted, foreign, or corrupt snapshot must fail without showing a folder picker. Continue releasing the SQLCipher lock before the native dialog and filesystem generation.

- [ ] **Step 4: Preserve cancellation semantics and improve messages**

Native dialog cancellation remains `AppResult::success(None)` and must not render success or error feedback. Map invalid snapshot/image/size errors to stable, non-leaking Chinese messages while retaining a diagnostic ID.

- [ ] **Step 5: Run command and component tests**

Run:

```powershell
pnpm vitest run src/modules/export/components/ExportSnapshotHistory.test.ts src/app/views/ReportView.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_generation --test export_store
```

Expected: all tests pass.

### Task 4: Documentation, visual QA, and local baseline

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/export.md`

**Interfaces:**
- Consumes all prior tasks.
- Produces a verified local Git commit; it does not push.

- [ ] **Step 1: Document the export read boundary**

State that candidate queries are side-effect-free, source ordering is deterministic, snapshot creation revalidates ownership/status transactionally, and output paths never cross into Vue state.

- [ ] **Step 2: Run all quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Regenerate bindings twice and compare SHA-256. Run `git diff --check`.

- [ ] **Step 3: Perform browser visual QA**

Use an explicit development-only report preview state, inspect 1280 px and narrow layouts, source switching, selection animation, empty/error states, snapshot generation feedback, focus order, and reduced-motion CSS. Remove any temporary fixture that would ship in production; an explicit `import.meta.env.DEV` gallery may remain.

- [ ] **Step 4: Review and commit the local baseline**

Inspect staged paths and the full diff. Commit only this plan's files with:

```powershell
git commit -m "feat: add explicit export selection workflow"
```

Report the exact local SHA. Do not push without a new exact-SHA authorization.
