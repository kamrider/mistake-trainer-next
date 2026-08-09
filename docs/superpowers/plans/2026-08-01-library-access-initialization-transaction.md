# Library Access Initialization Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make library access checks and workspace startup a single-flight state machine so retries cannot duplicate initialization or let stale results replace a newer unlocked state.

**Architecture:** Add a focused `useLibraryAccessLifecycle` composable that owns access phase, error classification, unlock state, access-check coalescing, and exactly-once workspace initialization. `App.vue` supplies normalized command adapters plus its existing startup callback and consumes the controller's readonly state.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue

## Global Constraints

- Do not modify Rust files or the existing `src-tauri/src/infrastructure/recognition_visual_split.rs` change.
- Do not implement launch-gate work for licensing, privacy, support operations, account deletion, device migration, update recovery, or SLA.
- Two access checks started in the same in-flight window must share one command invocation and one result.
- Access checking and native unlock must never run concurrently, even when both error-page actions fire before Vue rerenders.
- Workspace initialization must run at most once after the first successful unlocked result, even when multiple callers await it.
- A failed or locked access result must never start workspace commands.
- After a completed access failure, an explicit retry must be allowed to start a fresh check.
- A successful access check must not be overwritten by a stale failure from a duplicate request.
- Unlock success must remain in `restarting` until the native process restarts.
- Do not stage or commit the existing dirty worktree.

---

### Task 1: Library Access Lifecycle Controller

**Files:**
- Create: `src/app/composables/useLibraryAccessLifecycle.ts`
- Create: `src/app/composables/useLibraryAccessLifecycle.test.ts`

**Interfaces:**
- Consumes: `desktopRuntime`, `checkAccess(): Promise<AppResult<LibraryAccessStatus>>`, `unlock(): Promise<AppResult<LibraryAccessStatus>>`, and `initializeWorkspace(): Promise<void>`.
- Produces: readonly `phase`, `errorMessage`, `errorReason`, and `workspaceInitialized`; actions `checkLibraryAccess()`, `unlockLibrary()`, and `enterRestarting()`.

- [x] **Step 1: Write failing lifecycle tests**

Cover these exact cases: concurrent checks invoke `checkAccess` and `initializeWorkspace` once; a locked result never initializes; a storage error maps to `storage` and permits a later successful retry; unlock is synchronous single-flight; an access retry blocks concurrent unlock; unlock success stays in `restarting`; and browser preview initializes once without calling native access commands.

- [x] **Step 2: Run the focused test and verify failure**

Run: `npm test -- --run src/app/composables/useLibraryAccessLifecycle.test.ts`

Expected: FAIL because `useLibraryAccessLifecycle.ts` does not exist.

- [x] **Step 3: Implement the lifecycle state machine**

Use separate non-reactive `accessTask`, `initializationTask`, and `unlockTask` promises. Assign each task before awaiting its operation, return the same task to concurrent callers, clear retryable tasks only in `finally`, and keep `workspaceInitialized` true after the first successful initialization.

- [x] **Step 4: Run the focused test and verify success**

Run: `npm test -- --run src/app/composables/useLibraryAccessLifecycle.test.ts`

Expected: all lifecycle tests PASS.

### Task 2: Application Shell Integration

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: `useLibraryAccessLifecycle` from Task 1 and normalized `commands.libraryAccessStatus()` / `commands.libraryUnlock()` adapters.
- Produces: application startup and retry wiring that performs one access check, one workspace initialization, and one startup synchronization chain per successful unlock lifecycle.

- [x] **Step 1: Add a failing same-turn retry integration test**

Render an initial access error, fire the retry button twice without awaiting a Vue render, hold the retry response pending, and assert only one additional `libraryAccessStatus` call. Resolve unlocked success and assert the shell appears while `systemStatus`, `backupRestoreStatus`, `authRestore`, and `syncNow` each run once; assert `profileList` runs once during initialization and once after the successful sync refresh.

- [x] **Step 2: Run the App profile test and verify failure**

Run: `npm test -- --run src/app/App.profile.test.ts`

Expected: the new test FAILS because the current root component starts two retry checks.

- [x] **Step 3: Replace root-local access flags with the controller**

Move `LibraryAccessPhase`, `LibraryAccessErrorReason`, access phase/error refs, access-check orchestration, unlock orchestration, and `workspaceInitialized` ownership out of `App.vue`. Keep the existing system/profile/restore/startup-sync body as the injected `initializeWorkspace` callback and route all template events through controller actions.

- [x] **Step 4: Run application access and profile tests**

Run: `npm test -- --run src/app/composables/useLibraryAccessLifecycle.test.ts src/app/App.profile.test.ts src/app/LibraryAccessScreen.test.ts src/app/App.test.ts`

Expected: every focused test passes.

### Task 3: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-library-access-initialization-transaction.md`

**Interfaces:**
- Consumes: completed lifecycle and integration work.
- Produces: checked task boxes and exact verification evidence.

- [x] **Step 1: Run static checks**

Run: `npm run typecheck`

Run: `npm run lint`

Expected: both commands exit 0.

- [x] **Step 2: Run the complete frontend suite**

Run: `npm test -- --run`

Expected: every test file and test passes.

- [x] **Step 3: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust file was edited during this task.

- [x] **Step 4: Record verification without committing**

Check every completed task box and append the exact red/green test, typecheck, lint, complete-suite, hygiene, and scope results. Do not run `git add` or `git commit`.

## Verification Record

- Lifecycle red phase: failed because `useLibraryAccessLifecycle.ts` did not exist.
- App integration red phase: the same-turn retry test observed 3 total `libraryAccessStatus` calls instead of 2.
- Cross-operation red phase: unlock started and returned success while an access retry was still pending.
- Focused green phase: 4 test files passed, 32 tests passed.
- Typecheck: `npm run typecheck` exited 0 after expressing the initialization task's definite-assignment invariant.
- Lint: `npm run lint` exited 0 with zero warnings.
- Complete frontend suite: 100 test files passed, 624 tests passed.
- Patch hygiene: `git diff --check` exited 0; existing LF-to-CRLF notices were warnings only, and the scoped trailing-whitespace scan returned no matches.
- Git index: `git diff --cached --name-only` returned no files.
- Scope: only `App.vue`, its profile integration test, the new lifecycle composable/tests, and this plan were touched; no Rust file was edited during this task, and the pre-existing `recognition_visual_split.rs` modification was left untouched.
