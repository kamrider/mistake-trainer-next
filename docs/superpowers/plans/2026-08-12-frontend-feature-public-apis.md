# Frontend Feature Public APIs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Give each consumed frontend feature one explicit public entrypoint and prevent the app layer from reaching into feature internals.

**Architecture:** Each directory under `src/modules` exposes only the components, composables, domain functions, and types used outside that feature through `index.ts`. `src/app` imports `@/modules/<feature>` only; internal feature files retain relative imports and remain free to change without widening the public API.

**Tech Stack:** Vue 3.5, TypeScript 5.9, Vitest 4, ESLint 9.

## Global Constraints

- Preserve the dependency rules enforced by `tests/architecture-dependency-direction.test.ts`.
- Do not change component behavior, props, events, user-facing copy, or generated bindings.
- Do not export internal components or composables that have no external consumer.
- Empty `src/modules/licensing` remains untouched until it owns production behavior.
- Existing tests may import feature internals when they are white-box tests; production files outside a feature may not.

---

### Task 1: App-to-feature public-entry contract

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`

**Interfaces:**
- Consumes: production imports from `src/app/**/*.ts` and `src/app/**/*.vue`.
- Produces: a failing assertion unless every module import has exactly the form `@/modules/<feature>`.

- [x] **Step 1: Add the failing import-shape test**

Add a scanner that reports every production app import which resolves inside `src/modules` and is not a root alias:

```ts
function appFeatureInternalImports() {
  return sourceFiles('src/app', new Set(['.ts', '.vue']))
    .filter(file => !/\.(?:test|spec)\.ts$/.test(file))
    .flatMap((file) => {
      const source = readFileSync(file, 'utf8')
      return importSpecifiers(source)
        .filter(({ value }) => resolvesIntoRoot(file, value, 'modules'))
        .filter(({ value }) => !/^@\/modules\/[^/]+$/.test(value))
        .map(({ line, value }) => `${repositoryPath(file)}:${line} -> ${value}`)
    })
}

it('requires app consumers to use feature public entrypoints', () => {
  expect(appFeatureInternalImports()).toEqual([])
})
```

- [x] **Step 2: Verify the test is red**

Run:

```powershell
pnpm vitest run tests/architecture-dependency-direction.test.ts
```

Expected: FAIL with deep imports from `AppShell.vue`, `CaptureView.vue`, `DashboardView.vue`, `LibraryView.vue`, `ReportView.vue`, `ReviewHistoryView.vue`, `ReviewView.vue`, and `SettingsView.vue`.

### Task 2: Minimal feature entrypoints

**Files:**
- Create: `src/modules/capture/index.ts`
- Create: `src/modules/dashboard/index.ts`
- Create: `src/modules/export/index.ts`
- Create: `src/modules/legacy/index.ts`
- Create: `src/modules/library/index.ts`
- Create: `src/modules/ocr/index.ts`
- Create: `src/modules/profiles/index.ts`
- Create: `src/modules/report/index.ts`
- Create: `src/modules/review/index.ts`
- Create: `src/modules/review-history/index.ts`
- Create: `src/modules/sync/index.ts`

**Interfaces:**
- Consumes: existing feature implementations.
- Produces: the exact public names currently consumed by `src/app`.

- [x] **Step 1: Export the capture surface**

```ts
export { default as CaptureCropEditor } from './components/CaptureCropEditor.vue'
export { default as CaptureWorkspace } from './components/CaptureWorkspace.vue'
export { useCaptureBatchLifecycle } from './composables/useCaptureBatchLifecycle'
export {
  useCaptureDraftSaveQueue,
  type CaptureDraftSaveOutcome,
  type CaptureDraftSaveQueueState,
  type CaptureDraftSaveUpdate,
} from './composables/useCaptureDraftSaveQueue'
export { useCaptureFileImport } from './composables/useCaptureFileImport'
export { useCaptureImportWorkflow } from './composables/useCaptureImportWorkflow'
export {
  useCaptureItemEditing,
  type CaptureCropEditorState,
} from './composables/useCaptureItemEditing'
export { useCaptureLanSession } from './composables/useCaptureLanSession'
export { useCaptureOrganizerActions } from './composables/useCaptureOrganizerActions'
export { useCapturePreviewCache } from './composables/useCapturePreviewCache'
export { useCaptureRefreshScheduler } from './composables/useCaptureRefreshScheduler'
```

- [x] **Step 2: Export dashboard, export, report, and profile surfaces**

```ts
// dashboard/index.ts
export { default as TrainingDashboard } from './components/TrainingDashboard.vue'

// export/index.ts
export { default as ExportCandidatePicker } from './components/ExportCandidatePicker.vue'
export { default as ExportSnapshotHistory } from './components/ExportSnapshotHistory.vue'
export { default as ExportWorkflowGuide } from './components/ExportWorkflowGuide.vue'
export { useExportCandidateSelection } from './composables/useExportCandidateSelection'
export { useExportSnapshotMutations } from './composables/useExportSnapshotMutations'

// report/index.ts
export { default as DueForecastPanel } from './components/DueForecastPanel.vue'
export { default as WeakAreaPanel } from './components/WeakAreaPanel.vue'

// profiles/index.ts
export { default as ProfileSwitcher } from './components/ProfileSwitcher.vue'
```

- [x] **Step 3: Export library and OCR surfaces**

```ts
// library/index.ts
export { default as LibraryBulkMetadataDialog } from './components/LibraryBulkMetadataDialog.vue'
export { default as LibraryWorkspace } from './components/LibraryWorkspace.vue'
export { default as ProblemDetailDrawer } from './components/ProblemDetailDrawer.vue'
export { useLibraryBatchStatus } from './composables/useLibraryBatchStatus'
export { useLibraryProblemActions } from './composables/useLibraryProblemActions'
export { useLibraryReviewLaunch } from './composables/useLibraryReviewLaunch'
export {
  EMPTY_LIBRARY_FILTERS,
  type LibraryAdvancedFilters,
} from './domain/libraryFilters'

// ocr/index.ts
export { default as OcrCapabilityPanel } from './components/OcrCapabilityPanel.vue'
export { useCaptureRecognitionWorkflow } from './composables/useCaptureRecognitionWorkflow'
export { useOcrComponentManagement } from './composables/useOcrComponentManagement'
```

- [x] **Step 4: Export review and review-history surfaces**

```ts
// review/index.ts
export { default as QuickSessionDialog } from './components/QuickSessionDialog.vue'
export { default as ReviewRoom } from './components/ReviewRoom.vue'
export { default as SchulteFocus } from './components/SchulteFocus.vue'
export { useReviewClock } from './composables/useReviewClock'
export {
  mapSimpleRating,
  type FsrsRating,
  type SimpleRating,
} from './domain/rating'

// review-history/index.ts
export { default as ReviewHistoryDetail } from './components/ReviewHistoryDetail.vue'
export {
  default as ReviewHistoryFilters,
  type HistoryFiltersValue,
} from './components/ReviewHistoryFilters.vue'
export { default as ReviewHistoryTimeline } from './components/ReviewHistoryTimeline.vue'
export { useReviewHistoryList } from './composables/useReviewHistoryList'
```

- [x] **Step 5: Export settings-owned feature panels**

```ts
// legacy/index.ts
export { default as LegacyImportPanel } from './components/LegacyImportPanel.vue'

// sync/index.ts
export { default as SyncConflictCenter } from './components/SyncConflictCenter.vue'
```

- [x] **Step 6: Type-check the entrypoints before changing consumers**

Run:

```powershell
pnpm typecheck
```

Expected: PASS; all re-exported names exist with their current types.

### Task 3: Migrate app consumers

**Files:**
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/DashboardView.vue`
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReviewHistoryView.vue`
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**
- Consumes: the feature entrypoints from Task 2.
- Produces: app imports with no knowledge of feature directory internals.

- [x] **Step 1: Replace CaptureView deep imports**

Use one grouped import:

```ts
import {
  CaptureCropEditor,
  CaptureWorkspace,
  useCaptureBatchLifecycle,
  useCaptureDraftSaveQueue,
  useCaptureFileImport,
  useCaptureImportWorkflow,
  useCaptureItemEditing,
  useCaptureLanSession,
  useCaptureOrganizerActions,
  useCapturePreviewCache,
  useCaptureRefreshScheduler,
  type CaptureCropEditorState,
  type CaptureDraftSaveOutcome,
  type CaptureDraftSaveQueueState,
  type CaptureDraftSaveUpdate,
} from '@/modules/capture'
import { useCaptureRecognitionWorkflow } from '@/modules/ocr'
```

- [x] **Step 2: Replace LibraryView and ReportView deep imports**

Use:

```ts
import {
  EMPTY_LIBRARY_FILTERS,
  LibraryBulkMetadataDialog,
  LibraryWorkspace,
  ProblemDetailDrawer,
  useLibraryBatchStatus,
  useLibraryProblemActions,
  useLibraryReviewLaunch,
  type LibraryAdvancedFilters,
} from '@/modules/library'
```

```ts
import {
  ExportCandidatePicker,
  ExportSnapshotHistory,
  ExportWorkflowGuide,
  useExportCandidateSelection,
  useExportSnapshotMutations,
} from '@/modules/export'
import { DueForecastPanel, WeakAreaPanel } from '@/modules/report'
```

- [x] **Step 3: Replace review, history, dashboard, settings, and shell deep imports**

Use only these module-root specifiers:

```ts
import { ProfileSwitcher } from '@/modules/profiles'
import { TrainingDashboard } from '@/modules/dashboard'
import { QuickSessionDialog, ReviewRoom, SchulteFocus, useReviewClock, mapSimpleRating, type FsrsRating, type SimpleRating } from '@/modules/review'
import { ReviewHistoryDetail, ReviewHistoryFilters, ReviewHistoryTimeline, useReviewHistoryList, type HistoryFiltersValue } from '@/modules/review-history'
import { LegacyImportPanel } from '@/modules/legacy'
import { OcrCapabilityPanel, useOcrComponentManagement } from '@/modules/ocr'
import { SyncConflictCenter } from '@/modules/sync'
```

- [x] **Step 4: Run the architecture test and type checker**

Run:

```powershell
pnpm vitest run tests/architecture-dependency-direction.test.ts
pnpm typecheck
```

Expected: PASS; app-to-feature deep import violations are empty.

### Task 4: Public API verification

**Files:**
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

**Interfaces:**
- Consumes: feature entrypoints and the architecture guard.
- Produces: a documented and regression-tested public API policy.

- [x] **Step 1: Document the module entrypoint rule**

Add:

```markdown
- Every consumed frontend feature exposes one `src/modules/<feature>/index.ts` public API. Production consumers outside that feature import the module root and never traverse `components`, `composables`, `domain`, or `services`.
```

- [x] **Step 2: Run proportional verification**

Run:

```powershell
pnpm contract:architecture
pnpm typecheck
pnpm lint
pnpm test
```

Expected: all commands PASS.

- [x] **Step 3: Audit the public surface**

Run:

```powershell
rg -n "@/modules/[^'\"]+/|\.\./modules/[^'\"]+/" src/app -g "*.ts" -g "*.vue"
git diff --check
```

Expected: `rg` returns no production deep imports and `git diff --check` returns no errors.

- [x] **Step 4: Prepare the App orchestration extraction plan**

The next plan must extract bootstrap/access recovery, synchronization lifecycle, and profile/workspace orchestration from `src/app/App.vue` into focused app controllers while preserving `App.vue` as the composition root.
