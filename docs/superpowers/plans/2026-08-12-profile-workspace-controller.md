# Profile and Workspace Controller Extraction Plan

> Execute this plan in order. Preserve unrelated dirty-worktree changes and keep generated-command adaptation and provider wiring in `App.vue`.

**Goal:** Move profile projection, create/rename/delete/select policy, workspace-transition admission, workspace refresh selection, and profile-sync scheduling out of `App.vue` into the existing profile application controller.

**Architecture:** `useProfileManagement` becomes the complete profile/workspace application controller. It consumes normalized profile operations plus three application capabilities—transition admission, workspace refresh, and mutation-sync scheduling—and exposes readonly shell state and named user actions. `App.vue` remains the composition root: it adapts generated Tauri commands, provides the shared workspace guard, and supplies navigation/remount mechanics without deciding profile policy.

**Tech Stack:** Vue 3 composables, strict TypeScript, Vitest, Testing Library, generated Tauri bindings, architecture source contracts.

## Boundary decisions

- `App.vue` may create and provide `workspaceTransitionGuard`; the profile controller decides which actions require it.
- `App.vue` may implement the mechanical workspace refresh callback (`profileEpoch` plus dashboard navigation); the controller decides when it runs.
- `App.vue` may adapt `commands.profile*` to normalized operations; it must not retain profile mutation branching or side-effect flags.
- Browser-preview fallback profile projection belongs to the controller because it is shell-facing profile state, not root composition.
- Sync-triggered `loadProfiles()` remains a composition callback between the sync and profile controllers; mutation scheduling policy belongs to the profile controller.
- Durable native success must never be reported as failure only because sync scheduling or dashboard navigation fails.

### Task 1: Specify the complete controller with failing tests

**Files:**
- Modify: `src/app/composables/useProfileManagement.test.ts`
- Modify: `src/app/composables/useProfileManagement.ts` only after the tests fail for the intended missing API.

- [x] **Step 1: Replace the low-level mutation harness with normalized profile operations**

Create default `list`, `create`, `rename`, `remove`, and `select` spies plus `attemptWorkspaceTransition`, `scheduleSync`, and `refreshWorkspace` capabilities.

- [x] **Step 2: Test shell projection**

Assert desktop mode exposes the real empty overview while browser-preview mode exposes exactly one `preview-profile` named `本机学习档案`; after a successful load, real profiles replace any fallback.

- [x] **Step 3: Test named mutation policy**

Assert:

- create requires transition admission, refreshes the workspace, and schedules sync;
- rename does not request transition admission or workspace refresh, but schedules sync;
- deleting the active profile requires admission and refreshes; deleting another profile does neither;
- selecting the current profile is a no-op; selecting another profile requires admission and refreshes but does not schedule mutation sync.

- [x] **Step 4: Preserve concurrency and failure semantics**

Keep coverage for load/mutation single-flight, a silently queued sync refresh, native application errors, thrown transport errors, and best-effort side-effect failures.

- [x] **Step 5: Run the focused test red**

```powershell
pnpm vitest run src/app/composables/useProfileManagement.test.ts
```

Expected: FAIL because the controller does not yet accept normalized named operations or expose named profile actions and shell projections.

### Task 2: Implement the profile/workspace controller

**Files:**
- Modify: `src/app/composables/useProfileManagement.ts`
- Modify: `src/app/composables/useProfileManagement.test.ts`

**Interfaces:**

```ts
export interface ProfileOperations {
  list: () => Promise<AppResult<ProfileOverview>>
  create: (name: string) => Promise<AppResult<ProfileOverview>>
  rename: (profileId: string, name: string) => Promise<AppResult<ProfileOverview>>
  remove: (profileId: string, confirmationName: string) => Promise<AppResult<ProfileOverview>>
  select: (profileId: string) => Promise<AppResult<ProfileOverview>>
}

export interface ProfileManagementOptions {
  enabled: boolean
  operations: ProfileOperations
  attemptWorkspaceTransition: () => Promise<boolean>
  scheduleSync: () => void
  refreshWorkspace: () => Promise<unknown>
}
```

- [x] **Step 1: Add readonly shell projections**

Expose `shellProfiles` and `shellActiveProfileId` as readonly computed refs. Keep raw `profiles` and `activeProfileId` available for controller composition and diagnostics.

- [x] **Step 2: Keep one internal mutation runner**

Retain the existing application/transport error behavior, overview application, best-effort side effects, queued refresh, and busy-state release. Do not expose policy flags to `App.vue`.

- [x] **Step 3: Implement named actions**

Implement `createProfile`, `renameProfile`, `deleteProfile`, and `selectProfile` with the policy matrix from Task 1. Reject disabled, busy, cancelled, and same-profile work before invoking native operations.

- [x] **Step 4: Run the controller tests green**

```powershell
pnpm vitest run src/app/composables/useProfileManagement.test.ts
```

Expected: PASS.

### Task 3: Compose normalized operations in App.vue

**Files:**
- Modify: `src/app/App.vue`
- Verify: `src/app/App.profile.test.ts`

- [x] **Step 1: Adapt generated profile commands**

Pass normalized `list`, `create`, `rename`, `remove`, and `select` operations into `useProfileManagement`. Pass `workspaceTransitionGuard.attempt`, `syncController.scheduleMutation`, and the existing dashboard/remount callback as capabilities.

- [x] **Step 2: Consume readonly controller state and named actions**

Destructure `shellProfiles`, `shellActiveProfileId`, `profileBusy`, `profileError`, `loadProfiles`, and the four named actions for the existing `AppShell` bindings.

- [x] **Step 3: Delete root-owned profile policy**

Remove `previewProfile`, the two shell projection computeds, `mutateProfile`, and the four local profile action functions from `App.vue`.

- [x] **Step 4: Preserve sync/profile composition**

Keep sync success calling `loadProfiles()`. A pulled non-mutation update may still bump `profileEpoch`; the profile controller must remain independent from sync report semantics.

- [x] **Step 5: Run profile integration tests**

```powershell
pnpm vitest run src/app/App.profile.test.ts src/app/composables/useProfileManagement.test.ts
```

Expected: PASS, including transition cancellation, repeated switches, active/non-active deletion, sync refresh racing, browser preview, and durable-side-effect behavior.

### Task 4: Lock ownership and verify the stage

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

- [x] **Step 1: Add profile ownership source assertions**

Assert `App.vue` contains `useProfileManagement` but does not contain:

```ts
for (const token of [
  'const previewProfile',
  'const shellProfiles = computed',
  'const shellActiveProfileId = computed',
  'mutateProfile',
  'function createProfile',
  'function renameProfile',
  'function deleteProfile',
  'function selectProfile',
  'workspaceTransitionGuard.attempt()',
]) {
  expect(appSource).not.toContain(token)
}
```

Assert `useProfileManagement.ts` contains readonly shell projections, all four named actions, transition admission, the active-delete distinction, and the explicit refresh/sync policies.

- [x] **Step 2: Document profile/workspace ownership**

Add to `docs/architecture.md`:

```markdown
- `useProfileManagement` owns profile loading, shell projection, mutation admission and single-flight, workspace-transition policy, dashboard refresh policy, and profile mutation sync scheduling. `App.vue` only adapts commands and supplies router, guard, and sync capabilities.
```

- [x] **Step 3: Run complete verification**

```powershell
pnpm contract:architecture
pnpm typecheck
pnpm lint
pnpm test
git diff --check
```

Expected: all commands PASS.

- [x] **Step 4: Prepare the next giant-view extraction plan**

Audit `CaptureView.vue` and `SettingsView.vue` by line count, state cluster, command fan-out, and test coverage. Select the higher-risk cohesive workflow for the next extraction rather than splitting by arbitrary line ranges.
