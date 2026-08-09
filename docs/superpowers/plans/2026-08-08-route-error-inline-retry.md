# Route Error Inline Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The active workspace policy forbids sub-agents unless the user explicitly requests them, so this session uses local review.

**Goal:** Let users retry a transiently failed route in place while preserving a safe exit to the training dashboard and clearly warning about unsaved page input.

**Architecture:** `App.vue` remains the route-level error boundary. A private `routeRenderEpoch` participates in the routed subtree key; retry clears only the captured route error and increments that epoch, forcing a fresh component instance without restarting the app, reloading the window, or changing the current route.

**Tech Stack:** Vue 3 error capture and keyed rendering, Vue Router, Testing Library, Vitest, scoped application CSS.

## Global Constraints

- Preserve the current route, active profile, app shell, sync controller, library access state, and saved local data during retry.
- Do not call `window.location.reload`, restart Tauri, or touch updater recovery.
- Keep “回到训练台” as a second safe action.
- State that saved local data is unchanged and unsaved page input may need to be entered again.
- A retry must create a fresh routed component instance; simply hiding the alert is insufficient.
- A route that throws again must return to the same recovery UI instead of producing a blank page or unhandled exception.
- Keep both recovery actions at least 44 px high and keyboard-accessible.
- Do not add dependencies, change APIs, stage, or commit; preserve unrelated dirty-worktree edits.

---

### Task 1: Lock route retry behavior

**Files:**
- Modify: `src/app/App.test.ts`

**Interfaces:**
- Consumes: route-level `onErrorCaptured` fallback in `App.vue`.
- Verifies: button named `重试当前页面` remounts the same route.

- [x] **Step 1: Add a transient route regression**

Register a `/transient-route` component whose `setup()` throws on its first instance and renders `页面已恢复` on its second. Navigate there, assert the route error and unsaved-input warning, click `重试当前页面`, then assert the heading appears and `router.currentRoute.value.name` remains `transient-route`.

- [x] **Step 2: Extend the persistent-error regression**

In the existing always-broken route test, assert both `重试当前页面` and `回到训练台` are visible. Click retry, assert the same recovery alert returns, then click the dashboard action and assert navigation succeeds.

- [x] **Step 3: Run the targeted test in red state**

Run: `npm test -- src/app/App.test.ts`

Expected: FAIL because `重试当前页面` does not exist and the routed subtree has no retry epoch.

---

### Task 2: Implement safe in-place remount

**Files:**
- Modify: `src/app/App.vue`

**Interfaces:**
- Produces: private `routeRenderEpoch: Ref<number>` and `retryCurrentRoute(): void`.
- Preserves: existing route navigation, `profileEpoch`, transition direction, and route error capture.

- [x] **Step 1: Add the retry epoch and reset function**

Initialize `routeRenderEpoch` to zero. `retryCurrentRoute()` must clear `routeError` and `routeErrorDetail`, then increment the epoch.

- [x] **Step 2: Key the routed subtree by retry epoch**

Change the `.route-page` key to include `routeRenderEpoch` after `route.fullPath` and `profileEpoch`, so retry reconstructs only the routed page.

- [x] **Step 3: Improve recovery copy and actions**

Use the desktop detail `已保存的本地资料没有被修改。重试会重新打开此页面，未保存的页面输入可能需要重新填写。` and equivalent generic browser-preview copy. Add a primary `重试当前页面` button before the existing dashboard action.

- [x] **Step 4: Style the two-action group**

Wrap actions in `.route-error-actions`; give both buttons at least 44 px height and style the dashboard action as secondary without reducing contrast or touch target size.

- [x] **Step 5: Run App tests**

Run: `npm test -- src/app/App.test.ts`

Expected: all App tests pass, including transient and persistent retry paths.

---

### Task 3: Quality gates and review

**Files:**
- Verify: `src/app/App.vue`
- Verify: `src/app/App.test.ts`
- Modify: `docs/superpowers/plans/2026-08-08-route-error-inline-retry.md`

**Interfaces:**
- Preserves: all application tests and production build.

- [x] **Step 1: Run static gates**

Run: `npm run lint`

Run: `npm run typecheck`

Expected: no warnings or errors.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: Vite production build passes.

- [x] **Step 3: Run the complete regression suite**

Run: `npm test`

Expected: every Vitest file and test passes.

- [x] **Step 4: Review exact recovery invariants**

Verify retry does not call reload or change routes, repeated failure returns to the fallback, the successful retry creates a fresh instance, saved/unsaved copy is honest, both actions are accessible, the index is unstaged, excluded files are untouched, and `dist/` is ignored. Append exact command results in a `Verification Record` section.

## Verification Record

- Red phase: `src/app/App.test.ts` ran 7 tests with exactly the 2 new recovery tests failing because `重试当前页面`, retry remount state, and the unsaved-input warning did not exist.
- App green phase: 1 file, 7 tests passed.
- ESLint: passed with zero warnings.
- Vue/TypeScript typecheck: passed.
- Production build: passed; Vite transformed 2054 modules.
- Complete Vitest: 137 files, 768 tests passed.
- Transient route evidence: the current route stayed `transient-route`, the component instance count increased from 1 to 2, and `页面已恢复` rendered after retry.
- Persistent route evidence: retry returned to the same recovery alert without a blank page, then `回到训练台` navigated successfully.
- Source audit: no `window.location.reload`, `location.reload`, restart, API, updater, or outer-controller mutation was added. `routeRenderEpoch` only participates in the routed page key.
- Accessibility: both recovery buttons are native buttons with a 44 px minimum height; saved-data and unsaved-input consequences are stated explicitly.
- Diff check passed. The staged index remains empty, `dist/` remains ignored, and the existing `recognition_visual_split.rs` modification was not touched.
- Local review found no Critical or Important issues.
