# Settings Data Safety Panels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract storage-location and encrypted-backup presentation from `SettingsView.vue` while preserving native command orchestration, privacy boundaries, transaction confirmation, and focus recovery.

**Architecture:** Add one pure formatter module shared by the parent and both panels. `SettingsView` retains Tauri calls, dialogs, migration/restore transaction state, and receipt-copy decisions. `SettingsStoragePanel` and `SettingsBackupPanel` consume generated binding types, render only bounded labels and counts, emit user intentions, and expose their relevant trigger for focus restoration.

**Tech Stack:** Vue 3, TypeScript with `exactOptionalPropertyTypes`, Vitest, Testing Library, Lucide Vue.

## Global Constraints

- Preserve all unrelated dirty-worktree changes; do not stage or commit.
- Do not edit OCR, recognition, migration persistence, installer, release, or excluded pre-launch behavior.
- Preserve existing Chinese copy, DOM IDs, roles, ARIA labels, button names, dialog flow, and privacy redaction.
- Native commands and `normalizeAppResult` remain in `SettingsView`.
- Keep 760 px responsive layouts and 44 px touch targets.

---

### Task 1: Extract shared safe formatters

**Files:**

- Create: `src/app/settings-formatters.ts`
- Create: `src/app/settings-formatters.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

- Produces: `formatSettingsBytes(bytes: number | null): string`
- Produces: `formatSettingsTime(timestamp: number | null): string`

- [x] Add tests for bytes/B/KB/MB/GB, null/negative/non-finite values, valid timestamps, and out-of-range timestamps.
- [x] Run the focused test and confirm the module is missing.
- [x] Move the existing formatting behavior unchanged into the pure module.
- [x] Replace `formatBytes` and `formatBackupTime` calls in `SettingsView` with the imported functions.
- [x] Run formatter and `SettingsView` tests.

### Task 2: Extract the storage location panel

**Files:**

- Create: `src/app/components/SettingsStoragePanel.vue`
- Create: `src/app/components/SettingsStoragePanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

```ts
export interface SettingsStorageReceiptCopy {
  kind: 'success' | 'warning'
  title: string
  detail: string
}
```

The component requires:

```ts
{
  status: StorageLocationStatus | undefined
  statusMessage: string
  receipt: SettingsStorageReceiptCopy | undefined
  migrating: boolean
}
```

It emits `migrate` and exposes `focusMigrationAction(): void`.

- [x] Test bounded capacity rendering, receipt semantics, pending migration disabling, missing-status copy, safe labels, and the `migrate` event.
- [x] Run the focused test and confirm the component is missing.
- [x] Implement the component with the existing `settings-storage` ID and `资料库存储位置` accessible name.
- [x] Replace the inline section in `SettingsView`; change `openStorageMigration(event)` to `openStorageMigration()` and restore focus through the exposed component method.
- [x] Move only storage panel styles, including the 760 px layout, into the component.
- [x] Run component and parent integration tests.

### Task 3: Extract the encrypted backup panel

**Files:**

- Create: `src/app/components/SettingsBackupPanel.vue`
- Create: `src/app/components/SettingsBackupPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

The component requires:

```ts
{
  created: BackupSummary | undefined
  candidate: BackupRestoreCandidate | undefined
  creating: boolean
  preparing: boolean
  restoring: boolean
}
```

It emits `create`, `prepare`, and `openRestore`, and exposes `focusRestoreAction(): void`.

- [x] Test busy-state mutual exclusion, encrypted summary rendering, candidate isolation copy, explicit restore intent, and omission of unknown extra fields.
- [x] Run the focused test and confirm the component is missing.
- [x] Implement the component with the existing `settings-backup` ID and accessible copy.
- [x] Replace the inline section in `SettingsView`; restore focus after dialog cancellation through the exposed component method.
- [x] Move only backup panel and mobile action styles into the component.
- [x] Run component and parent integration tests.

### Task 4: Verify behavior and layout

- [x] Run `npm run typecheck`.
- [x] Run `npm run lint`.
- [x] Run `npm test -- --run`.
- [x] Run `git diff --check`.
- [x] Verify desktop and 375 px layouts: no horizontal overflow, action targets at least 44 px, directory anchors still reach `settings-storage` and `settings-backup`, and browser logs contain no warnings/errors.
