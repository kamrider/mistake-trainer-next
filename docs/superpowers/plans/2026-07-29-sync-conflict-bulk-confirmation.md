# Sync Conflict Bulk Confirmation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent accidental multi-field overwrite or local deletion by requiring an explicit, accessible confirmation before either card-level “全部采用” sync-conflict action.

**Architecture:** Keep conflict loading and mutations in `SyncConflictCenter.vue`. Extract the confirmation surface into a presentation-only `SyncConflictBulkDialog.vue`; the parent owns the pending group/choice, calls the existing generated command only after confirmation, and restores focus after cancel, success, or failure. Do not change generated bindings, Rust commands, or single-field resolution behavior.

**Tech Stack:** Vue 3, TypeScript, Testing Library, Vitest, scoped CSS.

## Global Constraints

- Preserve `syncConflictResolveEntity({ entityType, entityId, choice })` and all persistence behavior.
- Confirm both bulk choices; use stronger copy when the remote choice includes `__deleted__`.
- Put initial focus on the cancel action, trap Tab within the modal, support Escape/backdrop cancellation, and restore focus to the originating bulk button.
- Keep controls at least 44px high, support narrow Windows windows, and remove dialog motion under `prefers-reduced-motion`.
- Do not change single-field choices or add a default-selected resolution.
- Do not implement licensing, privacy/legal, support operations, account deletion, device migration, update-failure recovery, or SLA work.
- Do not edit generated `src/shared/api/bindings.ts`, stage, or commit the dirty worktree.

---

### Task 1: Add the bulk-resolution confirmation boundary

**Files:**
- Create: `src/modules/sync/components/SyncConflictBulkDialog.vue`
- Create: `src/modules/sync/components/SyncConflictBulkDialog.test.ts`

**Interfaces:**
- Consumes:

```ts
type SyncConflictChoice = 'local' | 'remote'

defineProps<{
  entityLabel: string
  conflictCount: number
  choice: SyncConflictChoice
  includesRemoteDeletion: boolean
}>()

defineEmits<{
  cancel: []
  confirm: []
}>()
```

- Produces: a modal `dialog` whose cancel action receives initial focus and whose confirm copy explicitly names the selected side and deletion consequence.

- [x] **Step 1: Write failing component tests**

```ts
it('defaults focus to cancel and names a local bulk overwrite', async () => {
  render(SyncConflictBulkDialog, {
    props: {
      entityLabel: '数学',
      conflictCount: 2,
      choice: 'local',
      includesRemoteDeletion: false,
    },
  })

  expect(screen.getByRole('dialog', { name: '确认数学的批量选择' }))
    .toHaveTextContent('2 处冲突全部采用本机版本')
  expect(screen.getByRole('button', { name: '取消，逐项确认' })).toHaveFocus()
})

it('states that a remote deletion removes the local entity', () => {
  render(SyncConflictBulkDialog, {
    props: {
      entityLabel: '数学',
      conflictCount: 1,
      choice: 'remote',
      includesRemoteDeletion: true,
    },
  })

  expect(screen.getByRole('dialog')).toHaveTextContent('本机这条内容将被删除')
  expect(screen.getByRole('button', { name: '确认采用云端并删除本机内容' })).toBeVisible()
})
```

Also prove Escape emits `cancel`, confirm emits `confirm`, and Tab/Shift+Tab stay inside the modal.

- [x] **Step 2: Run the new tests and verify they fail**

Run:

```powershell
pnpm test -- src/modules/sync/components/SyncConflictBulkDialog.test.ts
```

Expected: FAIL because `SyncConflictBulkDialog.vue` does not exist.

- [x] **Step 3: Implement the dialog**

Use `onMounted` plus `nextTick` to focus the cancel button. The root backdrop closes only on `mousedown.self`; the dialog uses `role="dialog"`, `aria-modal="true"`, stable `aria-labelledby`/`aria-describedby`, and a keydown handler that traps Tab and handles Escape. Derive copy from `choice` and `includesRemoteDeletion`; never expose entity IDs.

Use a responsive action layout and these floors:

```css
.dialog-actions button {
  min-height: 44px;
}

@media (max-width: 560px) {
  .dialog-actions {
    display: grid;
  }
}

@media (prefers-reduced-motion: reduce) {
  .bulk-backdrop,
  .bulk-dialog {
    animation: none;
  }
}
```

- [x] **Step 4: Run the component tests**

Run:

```powershell
pnpm test -- src/modules/sync/components/SyncConflictBulkDialog.test.ts
```

Expected: all dialog tests PASS.

### Task 2: Require confirmation in the conflict center

**Files:**
- Modify: `src/modules/sync/components/SyncConflictCenter.vue`
- Modify: `src/modules/sync/components/SyncConflictCenter.test.ts`

**Interfaces:**
- Consumes: `SyncConflictBulkDialog` from Task 1.
- Produces:

```ts
interface PendingGroupResolution {
  group: ConflictGroup
  choice: SyncConflictChoice
}

function requestGroupResolution(group: ConflictGroup, choice: SyncConflictChoice): void
function cancelGroupResolution(): Promise<void>
function confirmGroupResolution(): Promise<void>
```

- [x] **Step 1: Update integration tests before parent implementation**

Change the existing all-local test to prove the first click only opens the confirmation:

```ts
await userEvent.click(within(card).getByRole('button', {
  name: '数学全部采用本机版本',
}))
expect(api.syncConflictResolveEntity).not.toHaveBeenCalled()

await userEvent.click(screen.getByRole('button', {
  name: '确认全部采用本机版本',
}))
expect(api.syncConflictResolveEntity).toHaveBeenCalledWith({
  entityType: 'problem',
  entityId: 'problem-1',
  choice: 'local',
})
```

Add a cancellation test that closes the dialog, leaves the command untouched, and returns focus to the originating bulk button. Update the existing failure test to confirm through the dialog before checking that the card remains.

- [x] **Step 2: Run the center tests and verify the new assertions fail**

Run:

```powershell
pnpm test -- src/modules/sync/components/SyncConflictCenter.test.ts
```

Expected: FAIL because bulk actions still invoke the generated command immediately.

- [x] **Step 3: Wire pending resolution and focus restoration**

Import the dialog and add `pendingGroupResolution`. Replace both card-level `@click="resolveGroup(...)"` handlers with `requestGroupResolution(...)`, add `data-choice` to the originating buttons, and render the dialog once at the end of the center.

`cancelGroupResolution` must clear pending state, wait for `nextTick`, and focus:

```ts
const selector = `[data-conflict-group="${CSS.escape(group.key)}"] [data-choice="${choice}"]`
centerElement.value?.querySelector<HTMLElement>(selector)?.focus()
```

`confirmGroupResolution` must clear the dialog before awaiting the existing `resolveGroup`; the existing busy state, mutation scheduling, error retention, card removal, and next-card focus behavior remain authoritative.

- [x] **Step 4: Run focused sync UI tests**

Run:

```powershell
pnpm test -- src/modules/sync/components/SyncConflictBulkDialog.test.ts src/modules/sync/components/SyncConflictCenter.test.ts
```

Expected: all sync component tests PASS.

### Task 3: Run commercial frontend gates and self-review

**Files:**
- Modify: `docs/superpowers/plans/2026-07-29-sync-conflict-bulk-confirmation.md`

**Interfaces:**
- Consumes: completed Tasks 1–2.
- Produces: verified implementation and checked execution record.

- [x] **Step 1: Run frontend quality gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test:coverage
pnpm build
```

Expected: every command exits 0; all existing tests remain green.

- [x] **Step 2: Review the diff**

Confirm:

- the generated command call and payload are unchanged;
- no single-field action opens the bulk dialog;
- both bulk actions require confirmation;
- remote deletion copy explicitly states the local deletion consequence;
- cancel, success, and failure each leave keyboard focus in a valid location;
- opaque IDs are absent from visible and accessible copy;
- the new dialog has 12px minimum supporting text and 44px controls;
- no excluded pre-launch feature or unrelated source file was changed.

- [x] **Step 3: Mark this plan complete**

Only check the tasks above after their exact verification has passed. Do not stage or commit.
