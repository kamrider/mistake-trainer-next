# Settings Storage Lifecycle Controller Extraction Plan

> Execute this plan in order. Preserve unrelated dirty-worktree work, especially installer, storage-location, and Rust recovery changes.

**Goal:** Move settings-page storage status, migration receipt projection, migration dialog state, migration execution, restart admission, failure reporting, and focus restoration out of the 1,033-line `SettingsView.vue` into one focused controller.

**Architecture:** `useSettingsStorageLifecycle` consumes already-normalized storage operations and two application capabilities (`enterRestarting` and `restoreMigrationFocus`). It owns the complete local-library storage settings workflow. `SettingsView.vue` remains a composition view that adapts generated Tauri commands, supplies injected app capabilities, registers load tasks, and binds readonly state/actions to `SettingsStoragePanel` and `StorageMigrationDialog`.

**Tech Stack:** Vue 3 composables, strict TypeScript, Vitest, Testing Library, generated Tauri bindings, source-level architecture contracts.

## Why this slice is next

- `SettingsView.vue` is currently 1,033 lines and fans out to 30 generated commands; `CaptureView.vue` is 976 lines but already delegates most stateful workflows to capture feature composables.
- Storage migration is the highest-risk remaining cohesive settings workflow: it copies encrypted data, distinguishes native cancellation from failure, deliberately remains busy across a scheduled restart, and must restore focus on non-success exits.
- This extraction is bounded to storage lifecycle behavior. It does not mix backup, cloud auth, Windows update, OCR, preference, or library-lock policy into one generic settings store.

## Boundary decisions

- Generated command envelope checks stay in `SettingsView.vue`; the controller receives only `AppResult` values.
- Receipt-to-copy projection belongs to the controller because it is deterministic storage workflow presentation state.
- `SettingsView.vue` may retain the component ref, but focus restoration is injected as a capability and invoked by the controller.
- A `null` migration result means native folder selection was cancelled: close without an error, then restore focus.
- A successful scheduled migration calls `enterRestarting()` and deliberately keeps `busy` and the dialog visible until the root access boundary unmounts the settings page.
- Supplementary status/receipt load failures never block the rest of settings.

### Task 1: Specify storage lifecycle behavior with failing tests

**Files:**
- Create: `src/app/composables/useSettingsStorageLifecycle.test.ts`
- Create: `src/app/composables/useSettingsStorageLifecycle.ts` only after the test fails because the module is absent.

**Interfaces:**

```ts
export interface SettingsStorageOperations {
  status: () => Promise<AppResult<StorageLocationStatus>>
  receipt: () => Promise<AppResult<StorageMigrationReceipt | null>>
  migrate: () => Promise<AppResult<StorageMigrationReceipt | null>>
}

export interface SettingsStorageLifecycleOptions {
  operations: SettingsStorageOperations
  enterRestarting: () => void
  restoreMigrationFocus: () => Promise<unknown> | unknown
}
```

- [x] **Step 1: Create default operations and receipt fixtures**

Cover `moved`, `cleanup_required`, `rolled_back`, and `scheduled` receipts without absolute paths.

- [x] **Step 2: Test status and browser-preview messages**

Assert successful status application, backend user-message propagation, the stable exception copy, and explicit browser-preview copy. A failed load clears stale status.

- [x] **Step 3: Test receipt projection**

Assert the controller produces `SettingsStorageReceiptCopy` with bounded labels, asset counts, and formatted bytes for each outcome.

- [x] **Step 4: Test migration cancellation, failure, and success**

Assert native cancellation closes and restores focus without an error; backend/exception failures keep the dialog open and retryable; success calls `enterRestarting()` and deliberately remains busy.

- [x] **Step 5: Test cross-click single-flight and explicit close behavior**

Assert competing confirms invoke migration once and close is ignored while busy.

- [x] **Step 6: Run the new test red**

```powershell
pnpm vitest run src/app/composables/useSettingsStorageLifecycle.test.ts
```

Expected: FAIL because the controller module does not exist.

### Task 2: Implement the storage lifecycle controller

**Files:**
- Create: `src/app/composables/useSettingsStorageLifecycle.ts`
- Modify: `src/app/composables/useSettingsStorageLifecycle.test.ts`

- [x] **Step 1: Own readonly storage and dialog state**

Expose readonly `status`, `statusMessage`, `receipt`, `receiptCopy`, `dialogOpen`, `busy`, and `migrationMessage` refs/computeds plus explicit actions.

- [x] **Step 2: Implement supplementary loads**

Implement `loadStatus`, `loadReceipt`, and `showBrowserPreview`. Receipt failure is silent; status failure uses the exact existing fail-safe copy.

- [x] **Step 3: Implement migration lifecycle**

Implement `openMigration`, `closeMigration`, and `confirmMigration` with one busy guard, exact backend messages, exact generic exception copy, cancellation focus restoration, and restart-on-success behavior.

- [x] **Step 4: Run the controller tests green**

```powershell
pnpm vitest run src/app/composables/useSettingsStorageLifecycle.test.ts
```

Expected: PASS.

### Task 3: Compose the controller in SettingsView.vue

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Verify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: Adapt generated storage commands**

Pass normalized `storageStatus`, `storageMigrationReceipt`, and `storageMigrateSelect` operations into the controller. Pass optional `libraryAccessController?.enterRestarting()` and a `nextTick`-based focus callback as capabilities.

- [x] **Step 2: Bind controller state and actions**

Keep the existing panel/dialog props and events unchanged by destructuring the controller under the current template-facing names.

- [x] **Step 3: Delete view-owned storage workflow**

Remove six storage refs, `storageReceiptCopy`, `loadStorageStatus`, `loadStorageMigrationReceipt`, `openStorageMigration`, `closeStorageMigration`, and `confirmStorageMigration` from the view.

- [x] **Step 4: Preserve page-load composition**

Keep `loadStorageStatus` and `loadStorageMigrationReceipt` in `useSettingsPageLoad.supplementaryTasks`; browser preview calls the controller's explicit preview action.

- [x] **Step 5: Run storage integration tests**

```powershell
pnpm vitest run src/app/composables/useSettingsStorageLifecycle.test.ts src/app/views/SettingsView.test.ts
```

Expected: PASS, including bounded capacity, cancellation focus, scheduled restart, retryable failure, and prior receipt announcement.

### Task 4: Lock ownership and verify the stage

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

- [x] **Step 1: Add settings storage ownership assertions**

Assert `SettingsView.vue` contains `useSettingsStorageLifecycle` but does not contain:

```ts
for (const token of [
  'const storageStatus = ref',
  'const storageMigrationReceipt = ref',
  'const storageDialogOpen = ref',
  'const storageMigrating = ref',
  'const storageMigrationError = ref',
  'const storageReceiptCopy = computed',
  'async function loadStorageStatus',
  'async function loadStorageMigrationReceipt',
  'function openStorageMigration',
  'async function closeStorageMigration',
  'async function confirmStorageMigration',
]) {
  expect(settingsSource).not.toContain(token)
}
```

Assert the controller owns all state/actions, receipt outcome branches, exact safe-failure copy, `enterRestarting`, focus restoration, and readonly exposure.

- [x] **Step 2: Document settings storage ownership**

Add:

```markdown
- `useSettingsStorageLifecycle` owns settings-side storage status, migration receipt projection, migration dialog lifecycle, safe cancellation/failure handling, and restart transition. `SettingsView.vue` only adapts commands and supplies app capabilities.
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

- [x] **Step 4: Audit the next settings workflow**

Recount `SettingsView.vue` and compare the remaining backup/automatic-backup, Windows update, diagnostics, and device-lock clusters. Select the next cohesive workflow by destructive-risk and command fan-out.
