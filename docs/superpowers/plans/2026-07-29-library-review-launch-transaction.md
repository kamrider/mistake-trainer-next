# Library Review Launch Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent late training or exam creation results from hijacking navigation after the user leaves the library, while preserving clear persistence and retry semantics.

**Architecture:** Extract review-session creation from `LibraryView.vue` into a focused transaction controller. The controller captures the submitted IDs, enforces one in-flight launch, distinguishes command failure from post-persistence navigation failure, and checks route ownership before writing errors, clearing selection, or navigating.

**Tech Stack:** Vue 3 Composition API, Vue Router, TypeScript, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri command inputs and generated bindings unchanged.
- Preserve manual-review and exam preview routes in development mode.
- Use test-driven development and retain the current Chinese error and recovery copy.

---

### Task 1: Review Launch Transaction Controller

**Files:**
- Create: `src/modules/library/composables/useLibraryReviewLaunch.ts`
- Create: `src/modules/library/composables/useLibraryReviewLaunch.test.ts`

**Interfaces:**
- Consumes: current selection, current launch state, route ownership, error/selection/state callbacks, manual and exam operations, and navigation.
- Produces: `startReview(problemIds?: string[], fromDetail?: boolean, experience?: 'review' | 'exam'): Promise<void>`.

- [x] **Step 1: Write failing controller tests**

Cover exact ordered IDs for manual and exam commands, single-flight behavior, list/detail error routing, selection preservation on command failure, safe persisted-session copy on navigation failure, and suppression of navigation and errors after route ownership is lost.

- [x] **Step 2: Run the focused controller test and verify RED**

Run: `pnpm vitest run src/modules/library/composables/useLibraryReviewLaunch.test.ts`

Expected: FAIL because `useLibraryReviewLaunch.ts` does not exist.

- [x] **Step 3: Implement the minimal controller**

Capture a cloned problem-ID array before awaiting. Return for an empty request or existing launch. Set the active experience and clear the appropriate error only while the library owns the route. Execute the matching operation, and on success remove only submitted IDs from the latest list selection, then navigate only if route ownership remains. Use the backend message for `AppResult` failures, `训练卡组/模拟考试没有创建成功，请保持当前选择并稍后重试。` for thrown command failures, and `训练卡组/模拟考试已安全保存，可从侧边栏“训练室”继续。` when navigation fails after persistence.

- [x] **Step 4: Run the focused controller test and verify GREEN**

Run: `pnpm vitest run src/modules/library/composables/useLibraryReviewLaunch.test.ts`

Expected: PASS.

### Task 2: Library View Integration and Route-Ownership Regression

**Files:**
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`

**Interfaces:**
- Consumes: `useLibraryReviewLaunch`, normalized Tauri commands, the existing development preview route, and `route.name === 'library'` ownership.
- Produces: the same template event handlers with guarded asynchronous behavior.

- [x] **Step 1: Write a failing route-ownership regression test**

Defer `reviewManualStart`, begin a selected training deck, navigate the test router to `dashboard`, resolve successful session creation, then assert the router remains on `dashboard` and no late navigation to `review` occurs.

- [x] **Step 2: Run the focused view test and verify RED**

Run: `pnpm vitest run src/app/views/LibraryView.test.ts`

Expected: FAIL because the current inline function unconditionally pushes `review` after persistence.

- [x] **Step 3: Replace the inline launch transaction with the controller**

Instantiate the controller with live refs and route ownership. Wrap manual/exam commands in a single operation callback, preserve preview-mode navigation queries in the navigation callback, remove the inline transaction body, and retain the existing `trainProblem` and template call signatures.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `pnpm vitest run src/modules/library/composables/useLibraryReviewLaunch.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/app/views/LibraryView.test.ts`

Expected: PASS.

- [x] **Step 5: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0; the new controller has complete statement/function/line coverage; production build succeeds.

- [x] **Step 6: Review the final diff without committing**

Run `git diff --check` for modified tracked files, inspect all new controller files and this plan, and verify the scoped files remain unstaged.

Expected: no whitespace errors, no unrelated edits, and the existing dirty worktree remains unstaged.
