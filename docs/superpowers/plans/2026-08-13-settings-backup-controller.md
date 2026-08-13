# Settings Backup Controller Consolidation Plan

> Execute in order. Extend the existing tested backup composable; do not create a second competing backup store.

**Goal:** Turn `useSettingsBackupOperations` into the complete settings backup controller by adding automatic-backup policy, restore-dialog lifecycle, focus restoration, readonly state, and cross-mode single-flight; remove the remaining backup workflow branching from `SettingsView.vue`.

**Architecture:** The controller consumes normalized manual, portable, restore, and automatic-backup operations. It owns backup state and policy. `SettingsView.vue` adapts commands, supplies a focus callback and a page-message cleanup callback, and uses one combined busy signal for navigation admission.

**Why next:** After storage extraction, `SettingsView.vue` remains 958 lines. Backup/restore is the largest remaining destructive cluster and spans five manual commands, three automatic-backup commands, two busy states, one confirmation dialog, focus restoration, and the navigation guard. Windows update and diagnostics are lower-risk independent reads/actions.

## Required behavior

- Manual creation, portable creation, package preparation, restore startup, automatic configuration, and automatic disable are mutually exclusive.
- Supplementary automatic-status load never blocks the page.
- Restore success deliberately stays busy until restart; restore failure closes the dialog, retains the validated candidate, and restores focus.
- Native picker cancellation is neutral.
- Automatic settings failures preserve current status and use existing exact safe-failure copy.
- Navigation busy covers both manual and automatic operations.

### Task 1: Extend tests red

**Files:**
- Modify: `src/app/composables/useSettingsBackupOperations.test.ts`

- [x] Add automatic status/configure/disable fixtures and options.
- [x] Test mutual exclusion between automatic and manual operations.
- [x] Test automatic backend/transport failures and durable status application.
- [x] Test restore dialog open/close/focus behavior and failed-restore candidate retention.
- [x] Test combined navigation busy and readonly state.
- [x] Run `pnpm vitest run src/app/composables/useSettingsBackupOperations.test.ts` and verify the new API fails first.

### Task 2: Implement the complete controller

**Files:**
- Modify: `src/app/composables/useSettingsBackupOperations.ts`
- Modify: `src/app/composables/useSettingsBackupOperations.test.ts`

- [x] Export one explicit options interface containing normalized manual and automatic operations plus `restoreFocus` and `onOperationStart` capabilities.
- [x] Add automatic status/busy state and operations with exact existing messages.
- [x] Add restore-dialog state/actions; keep successful restore busy, close/focus on non-success.
- [x] Make manual and automatic paths share one admission boundary and expose readonly state/computeds.
- [x] Run the focused controller tests green.

### Task 3: Simplify SettingsView composition

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Verify: `src/app/views/SettingsView.test.ts`

- [x] Adapt `backupAutomaticStatus`, `backupAutomaticConfigure`, and `backupAutomaticDisable` alongside existing backup operations.
- [x] Supply `restoreFocus` and page-message cleanup capabilities.
- [x] Delete `restoreDialogOpen`, automatic backup refs/functions, four manual wrapper functions, and restore dialog functions from the view.
- [x] Use combined backup navigation busy in `useUnsavedChangesGuard`.
- [x] Preserve current template props/events through controller destructuring.
- [x] Run backup controller and full SettingsView tests.

### Task 4: Lock ownership and verify

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan checkboxes.

- [x] Assert backup workflow state/functions do not return to `SettingsView.vue` and are present in the controller.
- [x] Document complete settings backup ownership.
- [x] Run `pnpm contract:architecture`, `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `git diff --check`.
- [x] Recount SettingsView and choose Windows update, diagnostics, or device-lock as the next cohesive settings slice.
