# Profile Switcher Focus Continuity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the learning-profile dialog announce its popup relationship, receive focus when opened, focus each mounted subflow input, and return focus to the exact list action after cancelling create, rename, or delete.

**Architecture:** Keep the behavior local to `ProfileSwitcher.vue` because this popup owns a profile-specific list/form mode stack rather than a reusable menu contract. Capture a stable action/profile key before switching modes, locate the newly mounted matching button after the list returns, and use a close-button ref as the deterministic initial dialog focus target.

**Tech Stack:** Vue 3 Composition API, TypeScript, Testing Library Vue, Vitest.

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support operations, account deletion, device migration, updater recovery, or SLA work.
- Preserve all unrelated dirty-worktree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Keep the profile popup non-modal: do not add focus trapping, document inert state, or scroll locking.
- Retain the existing busy-state dismissal guard, Escape close behavior, and post-operation trigger restoration.

---

### Task 1: Profile dialog entry and subflow return focus

**Files:**
- Modify: `src/modules/profiles/components/ProfileSwitcher.test.ts`
- Modify: `src/modules/profiles/components/ProfileSwitcher.vue`

**Interfaces:**
- Consumes: existing `toggle()`, `resetForm()`, `beginCreate()`, `beginRename(profile)`, and `beginDelete(profile)` flow.
- Produces: trigger `aria-haspopup="dialog"`; close-button initial focus; launcher-aware `resetForm({ restoreLauncherFocus?: boolean })`; stable `data-profile-action` and `data-profile-id` return targets.

- [x] **Step 1: Write failing interaction tests**

Add one test proving the trigger exposes `aria-haspopup="dialog"`, opening focuses `关闭档案菜单`, and Escape still restores the trigger. Add another test that enters and cancels create, rename, and delete in sequence, asserting each remounted launcher button receives focus.

```ts
it('announces and moves focus into the profile dialog before restoring the trigger', async () => {
  const user = userEvent.setup()
  render(ProfileSwitcher, {
    props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
  })
  const trigger = screen.getByRole('button', { name: /当前学习档案：日常学习/ })

  expect(trigger).toHaveAttribute('aria-haspopup', 'dialog')
  await user.click(trigger)
  expect(screen.getByRole('button', { name: '关闭档案菜单' })).toHaveFocus()
  await user.keyboard('{Escape}')
  expect(trigger).toHaveFocus()
})

it('returns focus to each profile action after cancelling its subflow', async () => {
  const user = userEvent.setup()
  render(ProfileSwitcher, {
    props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
  })
  await user.click(screen.getByRole('button', { name: /当前学习档案：日常学习/ }))

  await user.click(screen.getByRole('button', { name: '新建学习档案' }))
  expect(screen.getByRole('textbox', { name: '新档案名称' })).toHaveFocus()
  await user.click(screen.getByRole('button', { name: '取消' }))
  expect(screen.getByRole('button', { name: '新建学习档案' })).toHaveFocus()

  await user.click(screen.getByRole('button', { name: '重命名档案：竞赛强化' }))
  expect(screen.getByRole('textbox', { name: '重命名档案' })).toHaveFocus()
  await user.click(screen.getByRole('button', { name: '取消' }))
  expect(screen.getByRole('button', { name: '重命名档案：竞赛强化' })).toHaveFocus()

  await user.click(screen.getByRole('button', { name: '删除档案：竞赛强化' }))
  expect(screen.getByRole('textbox', { name: '输入“竞赛强化”确认删除' })).toHaveFocus()
  await user.click(screen.getByRole('button', { name: '保留档案' }))
  expect(screen.getByRole('button', { name: '删除档案：竞赛强化' })).toHaveFocus()
})
```

- [x] **Step 2: Run the focused test to verify red state**

Run: `pnpm vitest run src/modules/profiles/components/ProfileSwitcher.test.ts`

Expected: the new assertions fail because the trigger lacks `aria-haspopup`, opening leaves focus on the trigger, and cancelling a mode leaves focus on the removed cancel button or document body.

- [x] **Step 3: Implement minimal focus continuity**

Add a ref for the close button and a stable semantic launcher key, make opening asynchronous so the close button is focused after `nextTick`, and locate the newly mounted matching launcher after a cancelled subflow remounts the list. Keep a pending focus key until the `out-in` transition's `after-enter` callback succeeds, because a single `nextTick` can precede the real list mount. Do not retain the original button node because the mode transition removes it from the document.

```ts
const closeButton = ref<HTMLButtonElement>()
type ProfileAction = 'create' | 'rename' | 'delete'
type ModeLauncher = { action: ProfileAction; profileId?: string }
const modeLauncher = ref<ModeLauncher>()
const pendingLauncherFocus = ref<ModeLauncher>()
const pendingNameInputFocus = ref<'focus' | 'select'>()

async function toggle() {
  if (props.busy) return
  open.value = !open.value
  if (!open.value) {
    resetForm()
    return
  }
  await nextTick()
  closeButton.value?.focus()
}

function resetForm({ restoreLauncherFocus = false } = {}) {
  const launcher = restoreLauncherFocus ? modeLauncher.value : undefined
  mode.value = 'list'
  editingProfileId.value = ''
  draftName.value = ''
  localError.value = ''
  modeLauncher.value = undefined
  pendingNameInputFocus.value = undefined
  if (launcher) {
    pendingLauncherFocus.value = launcher
    nextTick(restorePendingModeFocus)
  } else {
    pendingLauncherFocus.value = undefined
  }
}

function restorePendingModeFocus() {
  const launcher = pendingLauncherFocus.value
  if (launcher) {
    const buttons = root.value?.querySelectorAll<HTMLButtonElement>('[data-profile-action]') ?? []
    const button = Array.from(buttons).find(candidate =>
      candidate.dataset.profileAction === launcher.action
      && candidate.dataset.profileId === (launcher.profileId ?? ''),
    )
    if (button) {
      button.focus()
      pendingLauncherFocus.value = undefined
    }
  }

  const inputFocus = pendingNameInputFocus.value
  if (!inputFocus || !nameInput.value) return
  nameInput.value.focus()
  if (inputFocus === 'select') nameInput.value.select()
  pendingNameInputFocus.value = undefined
}
```

Set `modeLauncher` to `{ action: 'create' }`, `{ action: 'rename', profileId }`, or `{ action: 'delete', profileId }` in the corresponding entry function, and queue the matching input focus/select request. Add matching `data-profile-action`/`data-profile-id` attributes to the list buttons, bind `restorePendingModeFocus` to the mode transition's `after-enter`, add `ref="closeButton"` to the close button, add `aria-haspopup="dialog"` to the trigger, and make the cancel/keep buttons call `resetForm({ restoreLauncherFocus: true })`. Run the cancellation test with the real Transition instead of the default test stub, awaiting both form entry and list return.

- [x] **Step 4: Run focused tests and static checks**

Run: `pnpm vitest run src/modules/profiles/components/ProfileSwitcher.test.ts`

Expected: all `ProfileSwitcher` tests pass.

Run: `pnpm lint && pnpm typecheck`

Expected: both commands exit with code 0 and no warnings.

- [x] **Step 5: Run production and regression verification**

Run: `pnpm build`

Expected: mobile asset check, Vue type check, and Vite production build all pass.

Run: `pnpm vitest run`

Expected: the full frontend suite passes with no regressions.

- [x] **Step 6: Review the final diff and workspace isolation**

Run: `git diff --check`

Expected: no whitespace errors.

Run: `git diff -- src/modules/profiles/components/ProfileSwitcher.vue src/modules/profiles/components/ProfileSwitcher.test.ts docs/superpowers/plans/2026-08-08-profile-switcher-focus-continuity.md`

Expected: only the planned focus semantics, focus restoration, tests, and completed plan bookkeeping appear in the target diff; unrelated working-tree files remain untouched.

## Verification Record

- Focused red state: 2 new tests failed before implementation for missing popup semantics and lost cancellation focus.
- Focused final state: `ProfileSwitcher.test.ts` passed 8/8, including the real Vue Transition path.
- Static checks: `pnpm lint` and `pnpm typecheck` passed.
- Production build: `pnpm build` passed with 2055 modules transformed.
- Full frontend regression: 147 test files and 804 tests passed.
- Review fix: focus requests now remain pending through `mode="out-in"` and complete on `after-enter`, covering both form entry and list return in the real transition lifecycle.
