# Legacy Import History Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make legacy-import history refreshes latest-request-wins and distinguish a failed history read from a genuine empty migration history.

**Architecture:** Extract history-list request sequencing, loading, loaded, error, stale, and retained-data semantics from `LegacyImportPanel.vue` into `useLegacyImportHistory`. The panel continues to own scan/import/rollback transactions, but all startup and post-mutation history refreshes pass through one controller so a slow startup response cannot overwrite the list returned after a successful import or rollback.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri commands, generated bindings, legacy candidate validation, and exact import/rollback confirmation semantics unchanged.
- A failed history refresh must preserve the last successful rows.
- An initial history failure must never render the genuine-empty copy `还没有完成过旧版迁移。`.
- Only the newest history request may change rows, loaded/error state, or loading state.

---

### Task 1: Legacy Import History Controller

**Files:**
- Create: `src/modules/legacy/composables/useLegacyImportHistory.ts`
- Create: `src/modules/legacy/composables/useLegacyImportHistory.test.ts`

**Interfaces:**
- Consumes: `listImports: () => Promise<AppResult<LegacyImportSummary[]>>`.
- Produces: readonly `imports`, `loading`, `loaded`, `errorMessage`, computed `stale`, and `reload(): Promise<boolean>`.

- [x] **Step 1: Write failing controller tests**

Cover two overlapping deferred requests resolved newest-first and oldest-last, application failure before any successful read, thrown failure fallback copy, successful empty history, and a failed refresh that preserves previously loaded rows while setting `stale`.

The overlap test must assert:

```ts
const firstLoad = controller.reload()
const secondLoad = controller.reload()
second.resolve(success([newImport]))
expect(await secondLoad).toBe(true)
first.resolve(success([]))
expect(await firstLoad).toBe(false)
expect(controller.imports.value).toEqual([newImport])
```

- [x] **Step 2: Run the focused controller test and verify RED**

Run: `pnpm vitest run src/modules/legacy/composables/useLegacyImportHistory.test.ts`

Expected: FAIL because `useLegacyImportHistory.ts` does not exist.

- [x] **Step 3: Implement the minimal controller**

Use a monotonic request epoch. Each `reload()` increments the epoch, sets `loading`, and clears only the current visible request error. After every await and in `catch`/`finally`, mutate state only when the captured epoch still matches the latest epoch.

On success, replace `imports`, set `loaded = true`, clear the error, and return `true`. On application or thrown failure, keep existing rows and the previous successful `loaded` state, expose the exact application error or `迁移记录暂时无法读取，请稍后重试。`, and return `false`. Derive `stale` as `loaded && imports.length > 0 && Boolean(errorMessage)`.

- [x] **Step 4: Run the focused controller test and verify GREEN**

Run: `pnpm vitest run --coverage src/modules/legacy/composables/useLegacyImportHistory.test.ts`

Expected: PASS with 100% statement/branch/function/line coverage for the controller.

### Task 2: Legacy Import Panel Integration

**Files:**
- Modify: `src/modules/legacy/components/LegacyImportPanel.vue`
- Modify: `src/modules/legacy/components/LegacyImportPanel.test.ts`

**Interfaces:**
- Consumes: `useLegacyImportHistory` configured with normalized `commands.legacyImportList()`.
- Produces: unchanged scan/import/rollback behavior plus accurate initial-error, retry, loading, empty, retained-row, and stale-history states.

- [x] **Step 1: Write failing panel regression tests**

Add one test where the mount-time `legacyImportList` call remains deferred, a successful import triggers a second list call, the second call returns the new import, and the original request later returns an empty list. Assert the new import row remains visible after both promises settle.

Add one initial-failure test that asserts a `role="alert"` message and `重新读取迁移记录` button are visible, the genuine-empty copy is absent, and a successful retry with an empty array then renders the genuine-empty copy.

- [x] **Step 2: Run the focused panel test and verify RED**

Run: `pnpm vitest run src/modules/legacy/components/LegacyImportPanel.test.ts`

Expected: FAIL because the current mount response can overwrite the post-import refresh and failures are silently rendered as empty history.

- [x] **Step 3: Integrate the controller and accurate history states**

Remove the panel-owned `imports`, `historyLoading`, and `loadImports` implementation. Instantiate:

```ts
const importHistory = useLegacyImportHistory({
  listImports: async () => normalizeAppResult(await commands.legacyImportList()),
})
const {
  imports,
  loading: historyLoading,
  loaded: historyLoaded,
  errorMessage: historyError,
  stale: historyStale,
  reload: loadImports,
} = importHistory
```

Bind `aria-busy` to the history section. Render a dedicated alert containing `historyError` and a 44-pixel `重新读取迁移记录` button. Render `当前仍显示上一次成功读取的迁移记录。` as a polite status when `historyStale` is true. Show the initial loading copy only while `historyLoading && !historyLoaded`; show the genuine-empty copy only when `historyLoaded && imports.length === 0`; otherwise render retained rows.

Start `void loadImports()` after successful import and rollback so the newest command-adjacent refresh owns the final list without keeping the already-durable primary transaction busy or delaying progress-listener cleanup.

- [x] **Step 4: Run focused controller and panel tests and verify GREEN**

Run: `pnpm vitest run src/modules/legacy/composables/useLegacyImportHistory.test.ts src/modules/legacy/components/LegacyImportPanel.test.ts`

Expected: PASS.

### Task 3: Commercial-Quality Gates and Review

**Files:**
- Modify: `docs/superpowers/plans/2026-07-30-legacy-import-history-resilience.md`

**Interfaces:**
- Consumes: all files from Tasks 1-2.
- Produces: verified, unstaged changes with completed plan checkboxes.

- [x] **Step 1: Run focused legacy-import tests**

Run: `pnpm vitest run src/modules/legacy/composables/useLegacyImportHistory.test.ts src/modules/legacy/components/LegacyImportPanel.test.ts`

Expected: PASS.

- [x] **Step 2: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0, the full production build remains below the configured chunk warning threshold, and the new controller retains 100% coverage.

- [x] **Step 3: Review the final diff without committing**

Run `git diff --check` for modified tracked files, inspect both new controller files and the plan, and verify all scoped files remain unstaged.

Expected: no whitespace errors, no unrelated edits, and no Rust or generated-binding changes from this batch.
