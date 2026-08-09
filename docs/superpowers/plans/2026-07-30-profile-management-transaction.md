# Profile Management Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make profile loading, creation, renaming, deletion, and selection one shared single-flight transaction so stale refreshes cannot unlock the UI early or overwrite a newer profile overview.

**Architecture:** Extract profile list and mutation orchestration from `App.vue` into `useProfileManagement`. The composable owns the profile overview, busy state, and operation error; it treats the Tauri command result as the durable boundary, coalesces a sync-triggered refresh behind an active mutation, and keeps navigation or sync scheduling failures from being misreported as a failed profile mutation. `ProfileSwitcher.vue` remains open and announces progress while an operation is active so feedback cannot disappear mid-command.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vue Router, Vitest, Testing Library Vue, ESLint, Vite.

## Global Constraints

- Do not modify the excluded pre-launch work: licensing, privacy/legal, support operations, account deletion, device migration, update recovery, or support SLA.
- Preserve all existing user changes and do not stage or commit files.
- Keep Tauri command signatures and generated bindings unchanged.
- Preserve exact-name confirmation and permanent-delete copy for profile deletion.
- Do not make navigation or sync scheduling part of the durable command-success boundary.
- A profile list refresh and a profile mutation must never run concurrently.

---

### Task 1: Shared Profile Management Controller

**Files:**
- Create: `src/app/composables/useProfileManagement.ts`
- Create: `src/app/composables/useProfileManagement.test.ts`

**Interfaces:**
- Consumes: `enabled`, normalized list and mutation operations returning `Promise<AppResult<ProfileOverview>>`, `scheduleSync()`, and `refreshWorkspace()`.
- Produces: readonly `profiles`, `activeProfileId`, `busy`, `errorMessage`; `loadProfiles()`; and `mutateProfile(operation, { refreshWorkspace, scheduleSync })`.

- [x] **Step 1: Write failing controller tests**

Cover a deferred list request that rejects a concurrent mutation, a deferred mutation that rejects a concurrent list refresh and second mutation, successful overview replacement, application failures, thrown command failures, and durable mutation success when sync scheduling or workspace navigation throws.

Use an options harness shaped like:

```ts
const current = createHarness()
const mutation = current.controller.mutateProfile(
  () => gate.promise,
  { refreshWorkspace: true, scheduleSync: true },
)
expect(current.controller.busy.value).toBe(true)
await current.controller.loadProfiles()
expect(current.listProfiles).not.toHaveBeenCalled()
```

For durable success, assert that the returned overview remains applied, `errorMessage` stays empty, and `busy` is released even when both optional side effects throw.

- [x] **Step 2: Run the focused controller test and verify RED**

Run: `pnpm vitest run src/app/composables/useProfileManagement.test.ts`

Expected: FAIL because `useProfileManagement.ts` does not exist.

- [x] **Step 3: Implement the minimal controller**

Define:

```ts
interface ProfileManagementOptions {
  enabled: boolean
  listProfiles: () => Promise<AppResult<ProfileOverview>>
  scheduleSync: () => void
  refreshWorkspace: () => Promise<unknown>
}

interface ProfileMutationPolicy {
  refreshWorkspace: boolean
  scheduleSync: boolean
}
```

Both public operations must return immediately when disabled or busy. When a list refresh arrives during a mutation, queue exactly one silent refresh for the end of that mutation instead of running it concurrently or dropping it. Set `busy` before awaiting any command, clear the prior error only when a real foreground operation starts, apply a successful overview atomically, and release `busy` in `finally`.

For `mutateProfile`, keep the command call in its own `try/catch`. After a successful command result is applied, run `scheduleSync` and `refreshWorkspace` in isolated guarded blocks; failures in those optional side effects must not replace durable success with `学习档案没有完成这次操作，请稍后重试。`.

- [x] **Step 4: Run the focused controller test and verify GREEN**

Run: `pnpm vitest run src/app/composables/useProfileManagement.test.ts`

Expected: PASS with complete statement/function/line coverage for the controller.

### Task 2: App Integration

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: `useProfileManagement`, normalized profile commands, `syncController.scheduleMutation()`, and `router.push({ name: 'dashboard' })`.
- Produces: the existing `AppShell` profile props/events with shared single-flight semantics and unchanged `profileEpoch` refresh behavior.

- [x] **Step 1: Add an App integration regression test**

Reuse the existing desktop mocks in `App.profile.test.ts`. Replace `profileSelect` with a deferred promise, start a switch from `日常学习` to `竞赛强化`, and assert every profile action and the menu close button stay disabled while exactly one `profileSelect('contest')` command is pending. Resolve the command and verify the shell updates to `竞赛强化`, the route becomes `dashboard`, and the menu closes.

- [x] **Step 2: Run the App regression test and verify RED**

Run: `pnpm vitest run src/app/App.profile.test.ts`

Expected: FAIL because `App.vue` still owns independent `loadProfiles` and `mutateProfile` functions.

- [x] **Step 3: Replace App-owned profile transaction state**

Instantiate the controller after `syncController` is available:

```ts
const profileManagement = useProfileManagement({
  enabled: desktopRuntime,
  listProfiles: async () => normalizeAppResult(await commands.profileList()),
  scheduleSync: () => syncController.scheduleMutation(),
  refreshWorkspace: async () => {
    profileEpoch.value += 1
    await router.push({ name: 'dashboard' })
  },
})
```

Destructure the readonly state and public operations, keep the four existing event wrappers, and remove the local profile refs, `applyOverview`, `loadProfiles`, and `mutateProfile`. The wrappers normalize their respective Tauri command results and pass the existing policies:

```ts
return mutateProfile(
  async () => normalizeAppResult(await commands.profileCreate({ name })),
  { refreshWorkspace: true, scheduleSync: true },
)
```

Selection uses `{ refreshWorkspace: true, scheduleSync: false }`; renaming uses `{ refreshWorkspace: false, scheduleSync: true }`.

- [x] **Step 4: Run controller and App tests and verify GREEN**

Run: `pnpm vitest run src/app/composables/useProfileManagement.test.ts src/app/App.profile.test.ts`

Expected: PASS.

### Task 3: Non-Dismissable Busy Feedback in Profile Switcher

**Files:**
- Modify: `src/modules/profiles/components/ProfileSwitcher.vue`
- Modify: `src/modules/profiles/components/ProfileSwitcher.test.ts`

**Interfaces:**
- Consumes: the existing `busy` prop.
- Produces: `aria-busy` on the dialog, a polite list-mode progress status, and close/Escape/outside-click controls that cannot dismiss the operation while busy.

- [x] **Step 1: Write a failing interaction regression test**

Open the switcher, start selection, rerender with `busy: true`, and assert:

```ts
expect(screen.getByRole('dialog', { name: '切换学习档案' })).toHaveAttribute('aria-busy', 'true')
expect(screen.getByRole('status')).toHaveTextContent('正在处理学习档案')
await user.keyboard('{Escape}')
expect(screen.getByRole('dialog', { name: '切换学习档案' })).toBeVisible()
expect(screen.getByRole('button', { name: '关闭档案菜单' })).toBeDisabled()
```

Then rerender with `busy: false` and no error and verify the existing watcher closes the panel and restores focus.

- [x] **Step 2: Run the focused component test and verify RED**

Run: `pnpm vitest run src/modules/profiles/components/ProfileSwitcher.test.ts`

Expected: FAIL because the close button and Escape can currently dismiss the busy panel and no progress status exists in list mode.

- [x] **Step 3: Implement persistent busy feedback**

Guard `close()` with `if (props.busy) return`, disable the header close button while busy, bind `:aria-busy="busy"` to the dialog, and render a `role="status" aria-live="polite"` message reading `正在处理学习档案，请稍候…` in list mode while busy. Preserve successful auto-close when the prop transitions from true to false without an error.

Raise the close, rename, delete, create, retry, and form-action hit targets to at least 44 CSS pixels and keep supporting copy at 12 CSS pixels or above; constrain the panel to the viewport on narrow screens.

- [x] **Step 4: Run the focused component test and verify GREEN**

Run: `pnpm vitest run src/modules/profiles/components/ProfileSwitcher.test.ts`

Expected: PASS.

### Task 4: Commercial-Quality Gates and Review

**Files:**
- Modify: `docs/superpowers/plans/2026-07-30-profile-management-transaction.md`
- Modify: `vite.config.ts`

**Interfaces:**
- Consumes: all implementation and test files from Tasks 1-3.
- Produces: verified, unstaged changes with completed plan checkboxes.

- [x] **Step 1: Run focused profile tests**

Run: `pnpm vitest run src/app/composables/useProfileManagement.test.ts src/app/App.profile.test.ts src/modules/profiles/components/ProfileSwitcher.test.ts`

Expected: PASS.

- [x] **Step 2: Run commercial-quality gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test:coverage`, and `pnpm build`.

Expected: every command exits 0; the new controller has complete statement/branch/function/line coverage; production build succeeds without the 350 kB chunk warning. If the profile controller pushes the eager shell over the existing threshold, use supported Rolldown `output.codeSplitting.groups` to extract shared framework dependencies instead of raising the threshold or changing the eager core-route contract.

- [x] **Step 3: Review the final diff without committing**

Run `git diff --check` for modified tracked files, inspect both new controller files and the plan, and verify scoped files remain unstaged.

Expected: no whitespace errors, no unrelated edits, no generated-binding or Rust changes from this batch, and the existing dirty worktree remains unstaged.
