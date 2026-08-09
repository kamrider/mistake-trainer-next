# Review Durable Action Navigation Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep users in the training room while rating, exam-navigation, or focus progress is being persisted so a newly opened review page cannot read stale progress.

**Architecture:** Treat `submitting`, `transitioning`, and `focusBusy` as one durable-action boundary in `ReviewView.vue`. The view owns the route-leave guard and user-facing blocked-navigation message, while `ReviewRoom.vue` consumes its existing `submitting` prop to disable its local exit control and Escape shortcut consistently.

**Tech Stack:** Vue 3 Composition API, Vue Router navigation guards, TypeScript, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri command inputs and generated bindings unchanged.
- Do not interrupt or cancel an already-issued durable command.
- Preserve the existing busy behavior for rating, exam navigation, and focus controls.

---

### Task 1: Local Exit Lock in Review Room

**Files:**
- Modify: `src/modules/review/components/ReviewRoom.vue`
- Modify: `src/modules/review/components/ReviewRoom.test.ts`

**Interfaces:**
- Consumes: existing optional `submitting?: boolean` prop.
- Produces: a disabled exit button named `正在保存训练进度` and an Escape shortcut that does not emit `exit` while busy.

- [x] **Step 1: Extend the existing busy-state test and verify RED**

Render with `submitting: true`, press Escape, click-query the exit control, and assert it is disabled, named `正在保存训练进度`, and emits no `exit` event.

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts`

Expected: FAIL because the current exit control remains enabled and Escape emits `exit`.

- [x] **Step 2: Implement the local exit lock**

Set the exit button's `disabled` state and accessible label from `submitting`. In the Escape branch, call `preventDefault()` and return without emitting when `submitting` is true.

- [x] **Step 3: Run the focused component test and verify GREEN**

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts`

Expected: PASS.

### Task 2: Route-Level Durable Action Guard

**Files:**
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReviewView.test.ts`

**Interfaces:**
- Consumes: `submitting`, `transitioning`, and `focusBusy` refs plus Vue Router `onBeforeRouteLeave`.
- Produces: computed `durableActionBusy` and blocked-navigation copy `正在保存训练进度，请稍候再离开。`.

- [x] **Step 1: Write a failing routed integration test**

Render `ReviewView` through `RouterView`, defer `reviewSubmit`, reveal and rate the current problem, attempt to navigate to `dashboard`, and assert the route remains `review` with the blocked-navigation message. Resolve the command, assert completion, then navigate again and assert `dashboard` succeeds.

- [x] **Step 2: Run the focused view test and verify RED**

Run: `pnpm vitest run src/app/views/ReviewView.test.ts`

Expected: FAIL because the current view allows navigation during persistence.

- [x] **Step 3: Implement the shared durable-action boundary**

Add `durableActionBusy = computed(() => submitting.value || transitioning.value || focusBusy.value)`, pass it to `ReviewRoom`, and register `onBeforeRouteLeave` that returns `false` and sets the blocked-navigation message while busy, otherwise returns `true`.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts src/app/views/ReviewView.test.ts`

Expected: PASS.

- [x] **Step 5: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0; all existing review flows remain green; production build succeeds.

- [x] **Step 6: Review the final diff without committing**

Run `git diff --check` for both modified tracked component pairs, inspect the final route guard and shortcut logic, and verify the scoped files remain unstaged.

Expected: no whitespace errors, no unrelated edits, and the existing dirty worktree remains unstaged.
