# Learning Loop Upgrades Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing capture-to-review workflow into an actionable learning loop with reliable image intake, low-friction mistake classification, powerful library retrieval, focused short sessions, and evidence-based weakness reporting.

**Architecture:** Keep the encrypted local library authoritative and extend the existing Rust read/use-case boundaries rather than deriving learning policy in Vue. Deliver the work in independently testable vertical slices: canonical mistake-reason tags first, then query/bulk operations, quick sessions, report projections, and finally image-quality analysis plus reversible correction. Generated Tauri bindings remain the only Vue-to-Rust contract.

**Tech Stack:** Rust, rusqlite/SQLCipher, serde, Tauri/Specta bindings, Vue 3, TypeScript strict, Vitest, Testing Library, existing `image` crate.

## Global Constraints

- Windows-first and offline-first behavior must remain complete without a cloud backend.
- Every database read and mutation is scoped by both runtime `account_id` and active `profile_id`; Vue never supplies either identity.
- Existing user tags stay valid. Canonical mistake reasons are ordinary normalized tags with the reserved visible prefix `错因·`, so schema v17 backups and sync payloads remain compatible.
- Problem mutations and their sync outbox operations commit atomically; bulk mutation is all-or-nothing.
- Review events remain append-only and schedule state remains rebuildable from the complete event set.
- Quick sessions contain 1 through 100 unique active problems and never put problem IDs in routes, logs, or filenames.
- Image-quality analysis runs locally, never persists OCR text or plaintext images, and never creates or replaces a formal problem without explicit confirmation.
- Vue displays no invented statistics after a read failure. Empty, loading, and error states remain distinct.
- No new JavaScript runtime dependency is introduced. Any Rust image dependency addition requires a measured release-size justification.
- New interactive targets are at least 44 px, keyboard reachable, and motion is removed under `prefers-reduced-motion: reduce`.

---

### Task 1: Canonical mistake-reason catalog and quick selection

**Files:**
- Create: `src/modules/library/domain/mistakeReasons.ts`
- Create: `src/modules/library/domain/mistakeReasons.test.ts`
- Modify: `src/modules/library/components/ProblemTagEditor.vue`
- Modify: `src/modules/library/components/ProblemTagEditor.test.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Produces: `MISTAKE_REASON_TAGS: readonly MistakeReasonTag[]` where `MistakeReasonTag = { tag: string; label: string; description: string }`.
- Produces: `isMistakeReasonTag(tag: string): boolean` and `toggleMistakeReason(tags: string[], tag: string): string[]`.
- Extends: `ProblemTagEditor` with optional `suggestions?: readonly MistakeReasonTag[]` while preserving the existing `v-model` contract and 20-tag/30-character boundary.

- [x] **Step 1: Write failing domain tests**

  Add exact assertions for the canonical tags `错因·概念混淆`, `错因·审题遗漏`, `错因·计算失误`, `错因·方法不会`, `错因·步骤不完整`, and `错因·时间不足`. Assert `toggleMistakeReason` adds in catalog order, removes an active reason, preserves unrelated free tags, rejects unknown tags, and never mutates its input.

- [x] **Step 2: Run the domain test and verify failure**

  Run: `pnpm test -- src/modules/library/domain/mistakeReasons.test.ts`

  Expected: FAIL because `mistakeReasons.ts` does not exist.

- [x] **Step 3: Implement the pure catalog**

  Export the exact shape:

  ```ts
  export type MistakeReasonTag = {
    tag: `错因·${string}`
    label: string
    description: string
  }

  export const MISTAKE_REASON_TAGS = [
    { tag: '错因·概念混淆', label: '概念混淆', description: '定义、性质或适用条件没有分清' },
    { tag: '错因·审题遗漏', label: '审题遗漏', description: '漏看条件、单位、范围或题目要求' },
    { tag: '错因·计算失误', label: '计算失误', description: '方法正确，但运算、符号或抄写出错' },
    { tag: '错因·方法不会', label: '方法不会', description: '没有找到可执行的解题方法' },
    { tag: '错因·步骤不完整', label: '步骤不完整', description: '推理、证明或表达缺少关键步骤' },
    { tag: '错因·时间不足', label: '时间不足', description: '会做但没有在目标时间内完成' },
  ] as const satisfies readonly MistakeReasonTag[]
  ```

- [x] **Step 4: Write failing editor and capture tests**

  Assert suggestion chips expose pressed state, toggle one reason without clearing free tags, respect `disabled`, announce the reason description, and refuse a 21st tag. In `CaptureWorkspace`, select a draft and assert a quick-reason click flows through the existing queued draft update with normalized tags.

- [x] **Step 5: Implement accessible reason chips**

  Render the shared catalog in both the problem detail editor and selected capture draft editor. Use `aria-pressed`, keep the existing text tag editor available, and label the group `常见错因（可多选）`. Do not add a second save path; reuse existing dirty-state and draft-save queues.

- [x] **Step 6: Run focused tests**

  Run: `pnpm test -- src/modules/library/domain/mistakeReasons.test.ts src/modules/library/components/ProblemTagEditor.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

  Expected: PASS.

- [x] **Step 7: Commit the reason slice**

  ```powershell
  git add src/modules/library/domain src/modules/library/components/ProblemTagEditor.vue src/modules/library/components/ProblemTagEditor.test.ts src/modules/library/components/ProblemDetailDrawer.vue src/modules/library/components/ProblemDetailDrawer.test.ts src/modules/capture/components/CaptureWorkspace.vue src/modules/capture/components/CaptureWorkspace.test.ts
  git commit -m "feat: add quick mistake reason classification"
  ```

---

### Task 2: Advanced library query contract

**Files:**
- Modify: `src-tauri/src/modules/problems.rs`
- Modify: `src-tauri/src/modules/problem_query_repository.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/problem_query.rs`
- Modify: `src-tauri/tests/library_command.rs`
- Modify: `src/shared/api/bindings.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `ProblemListInput { status, search, subjects, tags, review_state, answer_state }`.
- Produces enums: `ProblemReviewState = Any | NeverReviewed | Due | RecentlyForgotten` and `ProblemAnswerState = Any | HasAnswer | MissingAnswer`.
- Replaces the page-facing call with `commands.problemList(input)` while keeping the Rust-owned runtime identity boundary.

- [x] **Step 1: Write failing repository tests**

  Create fixtures that cover two profiles, multiple subjects/tags, an unscheduled problem, a due schedule, a future schedule, a recent `again` event, and missing answer assets. Assert every single filter, AND composition across filter groups, OR behavior within selected subjects/tags, deterministic updated-time ordering, literal wildcard search, empty arrays as no-op, and a 100-character search limit.

- [x] **Step 2: Run focused Rust tests and verify failure**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test library_command`

  Expected: compilation fails because `ProblemListInput` and the filter enums do not exist.

- [x] **Step 3: Implement validated query input**

  Validate at most 20 subjects and 20 tags, each at most 30 Unicode characters. Build SQL with fixed predicates and bounded `json_each` checks; never concatenate user values into SQL. Define recently forgotten as the latest event for the problem having rating `again` within the previous 30 days.

- [x] **Step 4: Register typed command and regenerate bindings**

  Update the command to accept one typed input and call the repository with runtime account/profile and current UTC time. Run `pnpm bindings:generate`; assert generated unions and camelCase fields exactly match the Rust types.

- [x] **Step 5: Run query and binding tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test library_command` and `pnpm test -- src/shared/api/bindings.test.ts`.

  Expected: PASS.

---

### Task 3: Library filters and atomic bulk metadata editing

**Files:**
- Create: `src-tauri/src/modules/problem_bulk_metadata.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/bindings.rs`
- Create: `src-tauri/tests/problem_bulk_metadata.rs`
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`
- Create: `src/modules/library/components/LibraryFilterPanel.vue`
- Create: `src/modules/library/components/LibraryFilterPanel.test.ts`
- Create: `src/modules/library/components/LibraryBulkMetadataDialog.vue`
- Create: `src/modules/library/components/LibraryBulkMetadataDialog.test.ts`
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Consumes: `ProblemListInput` from Task 2.
- Produces: `ProblemBulkMetadataInput { problem_ids, subject, add_tags, remove_tags }` and `ProblemBulkMetadataReport { updated_count }`.
- Produces command: `problem_bulk_metadata(input) -> AppResult<ProblemBulkMetadataReport>`.

- [x] **Step 1: Write failing transaction tests**

  Assert 1–100 unique active problem IDs update atomically; optional subject replacement and tag add/remove preserve normalization; revisions increment exactly once; one canonical outbox operation is written per changed problem; foreign, archived, duplicate, stale, or invalid-tag input leaves every row and outbox count unchanged.

- [x] **Step 2: Implement the Rust transaction**

  Load and validate the entire selection before the first update. Apply subject and tag changes in caller order, skip byte-for-byte unchanged rows, and enqueue canonical upserts inside the same transaction. Return the number of rows actually changed.

- [x] **Step 3: Write failing filter and dialog tests**

  Assert filters are keyboard accessible, show removable active-filter chips, preserve state after a detail drawer closes, debounce text search only, and clear explicitly. Assert the bulk dialog previews the selected count, requires at least one change, and emits normalized metadata once.

- [x] **Step 4: Implement the Vue workflow**

  Keep filter state in `LibraryView`; refetch through the typed query after successful bulk edits. Extend the existing sticky selection bar with `批量修改` and do not disturb train/exam/archive/trash actions.

- [x] **Step 5: Run focused gates**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_bulk_metadata --test problem_query`, then `pnpm test -- src/modules/library/components/LibraryFilterPanel.test.ts src/modules/library/components/LibraryBulkMetadataDialog.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/app/views/LibraryView.test.ts`.

  Expected: PASS.

---

### Task 4: Focused five-minute and ten-problem sessions

**Files:**
- Modify: `src-tauri/src/modules/review.rs`
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/review_store.rs`
- Modify: `src-tauri/tests/review_command.rs`
- Modify: `src/modules/dashboard/components/TrainingDashboard.vue`
- Modify: `src/modules/dashboard/components/TrainingDashboard.test.ts`
- Create: `src/modules/review/components/QuickSessionDialog.vue`
- Create: `src/modules/review/components/QuickSessionDialog.test.ts`
- Modify: `src/app/views/DashboardView.vue`
- Modify: `src/app/views/DashboardView.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `QuickReviewPreset = FiveMinutes | TenProblems | RecentlyForgotten`.
- Produces: `ReviewQuickStartInput { preset, subject, tag }` and command `review_quick_start(input) -> AppResult<ReviewQueueOverview>`.
- Five-minute sessions select at most 8 cards; ten-problem sessions select at most 10; recently-forgotten sessions select at most 20. All select due cards before new cards and use deterministic `(due_at, updated_at, id)` ordering.

- [x] **Step 1: Write failing selection tests**

  Cover exact limits, due-before-new ordering, subject/tag filters, recent-`again` behavior, profile/status scoping, no candidates, and replacement of an existing inactive session without leaking IDs to the command response or route.

- [x] **Step 2: Implement Rust-owned quick selection**

  Translate the preset to a bounded query and reuse the existing validated manual-session transaction. Keep the experience `review`, source `manual`, and existing crash-safe progress behavior.

- [x] **Step 3: Write failing dashboard/dialog tests**

  Assert the dashboard exposes `快速训练`, presets explain their card limits, optional subject/tag filters are accessible, empty results keep the user on the dashboard with an actionable message, and success navigates only after persistence.

- [x] **Step 4: Implement the UI orchestration**

  Add one secondary CTA rather than three competing hero buttons. Disable duplicate starts, normalize command errors, and schedule mutation sync only after a session successfully persists.

- [x] **Step 5: Run focused tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store --test review_command` and `pnpm test -- src/modules/review/components/QuickSessionDialog.test.ts src/modules/dashboard/components/TrainingDashboard.test.ts src/app/views/DashboardView.test.ts`.

  Expected: PASS.

---

### Task 5: Weak-area analysis and seven-day workload forecast

**Files:**
- Modify: `src-tauri/src/modules/insights.rs`
- Modify: `src-tauri/src/modules/insights_read_repository.rs`
- Modify: `src-tauri/tests/insights_store.rs`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`
- Create: `src/modules/report/components/WeakAreaPanel.vue`
- Create: `src/modules/report/components/WeakAreaPanel.test.ts`
- Create: `src/modules/report/components/DueForecastPanel.vue`
- Create: `src/modules/report/components/DueForecastPanel.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Extends `ReportSummary` with `weakAreas: WeakAreaSummary[]` and `dueForecast: DueForecastDay[]`.
- Produces: `WeakAreaSummary { label, kind, reviewed_count, lapse_count, lapse_rate, average_duration_ms }`.
- Produces: `DueForecastDay { local_date, due_count, overdue_count }` for today plus the next six local calendar days.

- [x] **Step 1: Write failing insights tests**

  Build events across subjects and canonical reason tags. Assert a weakness needs at least two reviews, lapse means latest-30-day rating `again`, ordering is `(lapse_rate DESC, lapse_count DESC, reviewed_count DESC, label ASC)`, the list is capped at five, other profiles are excluded, and duration averages ignore invalid legacy nulls. Assert exact seven-day buckets across UTC offset boundaries and separate already-overdue count on today.

- [x] **Step 2: Implement the local read projection**

  Query canonical `错因·` tags through `json_each`, union them with subject aggregates, compute bounded numeric fields in Rust, and return empty arrays when no evidence exists. Do not convert sparse data into claims.

- [x] **Step 3: Write failing report presentation tests**

  Cover loading, error, sparse-data explanation, ordered weak-area rows, reason-vs-subject labels, lapse rate, average time, seven dates, overdue emphasis without anxiety-red copy, and responsive accessible text equivalents for charts.

- [x] **Step 4: Implement report panels**

  Place `本周最值得修正` before generic activity charts and `未来七天任务` beside the 14-day rhythm. Each weak row links to the library by setting a serializable filter descriptor through route query, never raw problem IDs.

- [x] **Step 5: Run focused tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store` and `pnpm test -- src/modules/report/components/WeakAreaPanel.test.ts src/modules/report/components/DueForecastPanel.test.ts src/app/views/ReportView.test.ts`.

  Expected: PASS.

---

### Task 6: Local capture quality analysis

**Files:**
- Create: `src-tauri/src/modules/capture_quality.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/commands/capture_inbox.rs`
- Modify: `src-tauri/src/bindings.rs`
- Create: `src-tauri/tests/capture_quality.rs`
- Modify: `src/modules/capture/components/CaptureThumbnail.vue`
- Modify: `src/modules/capture/components/CaptureThumbnail.test.ts`
- Create: `src/modules/capture/components/CaptureQualityPanel.vue`
- Create: `src/modules/capture/components/CaptureQualityPanel.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `CaptureQualityIssueCode = Blurry | TooDark | TooBright | LowContrast | PossibleEdgeCut | Skewed`.
- Produces: `CaptureQualityReport { item_id, issues, sharpness_score, dark_fraction, bright_fraction, contrast_score, suggested_rotation_degrees, suggested_crop }`.
- Produces command: `capture_quality_check(batch_id, item_id) -> AppResult<CaptureQualityReport>`; plaintext lifetime is bounded to the command and application-owned temp/decryption memory.

- [ ] **Step 1: Write deterministic image-analysis tests**

  Generate in-memory fixtures for sharp text-like edges, Gaussian-like blur, black/white clipping, low contrast, content touching each edge, and a rotated dominant baseline. Assert bounded scores and issue thresholds, plus ownership checks for foreign batch/item IDs and corrupt encrypted assets.

- [ ] **Step 2: Implement analysis without persistence**

  Downscale to at most 1,024 px, convert to luma, compute variance of a 3x3 Laplacian response, percentile contrast, clipped-pixel fractions, edge-band foreground density, and a conservative dominant-line rotation suggestion limited to `-8..=8` degrees. Delete or zero transient buffers as existing asset APIs permit.

- [ ] **Step 3: Write failing UI tests**

  Assert quality is checked lazily for the selected item, neutral results remain quiet, warnings use plain language, users can choose `继续使用`, `重新选择`, or `打开裁剪修正`, and no warning blocks capture commit without explicit product policy.

- [ ] **Step 4: Implement quality badges and correction handoff**

  Show at most one compact badge per thumbnail and full details in `CaptureQualityPanel`. Pass suggested rotation/crop into the existing crop editor as initial reversible recipes; saving continues through the current atomic crop operation.

- [ ] **Step 5: Run focused tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_quality` and `pnpm test -- src/modules/capture/components/CaptureQualityPanel.test.ts src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`.

  Expected: PASS.

---

### Task 7: Reversible perspective correction

**Files:**
- Modify: `src-tauri/src/modules/capture_crop.rs`
- Modify: `src-tauri/src/modules/capture_quality.rs`
- Modify: `src-tauri/tests/capture_quality.rs`
- Modify: `src-tauri/tests/capture_inbox_store.rs`
- Modify: `src/modules/capture/domain/cropGeometry.ts`
- Modify: `src/modules/capture/domain/cropGeometry.test.ts`
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.test.ts`

**Interfaces:**
- Adds: `PerspectiveQuad { top_left, top_right, bottom_right, bottom_left }` with normalized points in `[0,1]`.
- Extends: `CaptureCropRecipe` with `perspective_quad: PerspectiveQuad | null`.
- Existing derivation ledger remains the undo boundary; source assets remain immutable.

- [ ] **Step 1: Write failing geometry and Rust transform tests**

  Assert convex clockwise quads, minimum area, clamped keyboard movement, identity transform, known trapezoid rectification, output-size bounds, and rejection of crossed/tiny/out-of-range quads before staging any asset.

- [ ] **Step 2: Implement bilinear inverse mapping**

  Use the existing `image` buffers and bounded output dimensions. Sample with bilinear interpolation, preserve EXIF-free output behavior, and feed the transformed image into the existing crop/rotation/JPEG pipeline.

- [ ] **Step 3: Implement four-corner editor controls**

  Add an explicit `透视矫正` mode with four 44 px handles, keyboard arrow movement, reset, preview, and screen-reader coordinates. Never auto-apply the quality suggestion.

- [ ] **Step 4: Run focused tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_quality --test capture_inbox_store` and `pnpm test -- src/modules/capture/domain/cropGeometry.test.ts src/modules/capture/components/CaptureCropEditor.test.ts`.

  Expected: PASS.

---

### Task 8: Documentation, compatibility, and full release gates

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/library.md`
- Modify: `docs/plans/review.md`
- Modify: `docs/windows-capture-acceptance.md`
- Modify: `docs/windows-review-history-acceptance.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Documents the canonical reason-tag contract, query limits, quick-session ordering, sparse-data report policy, quality thresholds, and explicit confirmation boundary for correction.

- [ ] **Step 1: Run frontend quality gates**

  Run: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm bindings:check`, and `pnpm build`.

  Expected: every command exits zero and feature chunks remain within the documented gzip budgets.

- [ ] **Step 2: Run Rust and schema gates**

  Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`, `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings`, and `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`.

  Expected: every command exits zero; schema remains v17 unless a reviewed task explicitly requires a migration.

- [ ] **Step 3: Run installed-product acceptance**

  Execute the documented iPhone Safari and Android Chrome capture matrix, 150-image/1 GB recovery fixture, supported Word DOCX checks, 500-problem export, and two-device deterministic schedule scenario. Record exact device/build evidence in the acceptance documents.

- [ ] **Step 4: Review the complete diff**

  Verify every new command is registered in `src-tauri/src/bindings.rs`, every generated DTO is covered by the binding contract, no raw path/account/profile field crosses into Vue, and all new mutations notify the existing background-sync controller.

- [ ] **Step 5: Commit documentation and release evidence**

  ```powershell
  git add docs CHANGELOG.md
  git commit -m "docs: record learning loop upgrade acceptance"
  ```
