# Settings Maintenance Panels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `SettingsView.vue` presentation complexity by extracting the application-update and safe-diagnostics panels without changing their native command orchestration, privacy guarantees, focus recovery, or responsive behavior.

**Architecture:** `SettingsView` remains the stateful application boundary: it invokes Tauri commands, normalizes errors, owns retry state, and formats timestamps. Two focused presentational components receive typed state through props, emit user intentions, and expose one focus-recovery method. Each component owns its markup, accessibility contract, icons, and scoped responsive styles.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Testing Library, Vitest, Lucide Vue.

## Global Constraints

- Preserve all existing worktree changes and do not stage or commit.
- Do not edit OCR, recognition, migration, installer, release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Preserve the existing Chinese copy, DOM IDs, ARIA labels, button names, privacy redaction, update-version pinning, and focus restoration.
- Do not move Tauri command calls into presentational components.
- Keep mobile action targets at least 44 px high and preserve reduced-motion handling.

---

### Task 1: Extract the application update panel

**Files:**

- Create: `src/app/components/SettingsUpdatePanel.vue`
- Create: `src/app/components/SettingsUpdatePanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

- Consumes: required props whose `status` and `report` values are `WindowsUpdateStatus | undefined` and `WindowsUpdateCheckReport | undefined`, plus `checking`, `installing`, `message`, and `publicationLabel`.
- Produces: `check` and `install` events plus `focusPrimaryAction(): void`.

- [x] **Step 1: Add component tests**

Test that a disabled build shows no network action, an enabled build emits `check`, an available exact version emits `install`, busy buttons are disabled, and updater-private fields are never rendered.

- [x] **Step 2: Run the focused test and confirm the component is missing**

Run:

```powershell
npm test -- --run src/app/components/SettingsUpdatePanel.test.ts
```

Expected: FAIL because `SettingsUpdatePanel.vue` does not exist.

- [x] **Step 3: Implement the typed presentational component**

Use this public contract:

```ts
const props = defineProps<{
  status: WindowsUpdateStatus | undefined
  report: WindowsUpdateCheckReport | undefined
  checking: boolean
  installing: boolean
  message: string
  publicationLabel: string
}>()

const emit = defineEmits<{
  check: []
  install: []
}>()

defineExpose({ focusPrimaryAction })
```

Keep `id="settings-updates"` and `aria-label="应用更新状态"` unchanged. Copy the panel’s scoped desktop, 760 px mobile, button, and typography styles into the component.

- [x] **Step 4: Wire the component into `SettingsView`**

Replace the inline update `<section>` with:

```vue
<SettingsUpdatePanel
  ref="windowsUpdatePanel"
  :status="windowsUpdateStatus"
  :report="windowsUpdateReport"
  :checking="checkingWindowsUpdate"
  :installing="installingWindowsUpdate"
  :message="windowsUpdateMessage"
  :publication-label="formatUpdatePublication(windowsUpdateReport?.publishedAt ?? null)"
  @check="checkWindowsUpdate"
  @install="installWindowsUpdate"
/>
```

Replace the native button ref with:

```ts
const windowsUpdatePanel = ref<{ focusPrimaryAction: () => void }>()
```

and restore focus with `windowsUpdatePanel.value?.focusPrimaryAction()`.

- [x] **Step 5: Run component and parent integration tests**

Run:

```powershell
npm test -- --run src/app/components/SettingsUpdatePanel.test.ts src/app/views/SettingsView.test.ts
```

Expected: PASS.

### Task 2: Extract the safe diagnostics panel

**Files:**

- Create: `src/app/components/SettingsDiagnosticsPanel.vue`
- Create: `src/app/components/SettingsDiagnosticsPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

- Consumes: a required `receipt: DiagnosticExportReceipt | undefined` prop plus `exporting`, `message`, `nativeAvailable`, and `generatedAtLabel`.
- Produces: an `export` event plus `focusPrimaryAction(): void`.

- [x] **Step 1: Add component tests**

Test that the export event is emitted once per click, native-unavailable and exporting states disable the action, the receipt renders only its safe label/report ID/count, and an injected path-like extra property is never rendered.

- [x] **Step 2: Run the focused test and confirm the component is missing**

Run:

```powershell
npm test -- --run src/app/components/SettingsDiagnosticsPanel.test.ts
```

Expected: FAIL because `SettingsDiagnosticsPanel.vue` does not exist.

- [x] **Step 3: Implement the typed presentational component**

Use this public contract:

```ts
const props = defineProps<{
  receipt: DiagnosticExportReceipt | undefined
  exporting: boolean
  message: string
  nativeAvailable: boolean
  generatedAtLabel: string
}>()

const emit = defineEmits<{
  export: []
}>()

defineExpose({ focusPrimaryAction })
```

Keep `id="settings-diagnostics"`, the success `role="status"`, failure `role="alert"`, both ARIA labels, and the privacy copy unchanged. Copy the receipt transition, responsive layout, touch target, and reduced-motion styles into the component.

- [x] **Step 4: Wire the component into `SettingsView`**

Replace the inline diagnostics `<section>` with:

```vue
<SettingsDiagnosticsPanel
  ref="diagnosticsPanel"
  :receipt="diagnosticsReceipt"
  :exporting="exportingDiagnostics"
  :message="diagnosticsMessage"
  :native-available="isTauri()"
  :generated-at-label="formatBackupTime(diagnosticsReceipt?.generatedAtUtcMs ?? null)"
  @export="exportDiagnostics"
/>
```

Replace the native button ref with:

```ts
const diagnosticsPanel = ref<{ focusPrimaryAction: () => void }>()
```

and restore focus with `diagnosticsPanel.value?.focusPrimaryAction()`.

- [x] **Step 5: Run component and parent integration tests**

Run:

```powershell
npm test -- --run src/app/components/SettingsDiagnosticsPanel.test.ts src/app/views/SettingsView.test.ts
```

Expected: PASS.

### Task 3: Remove migrated styles and verify the frontend

**Files:**

- Modify: `src/app/views/SettingsView.vue`
- Verify: `src/app/components/SettingsUpdatePanel.vue`
- Verify: `src/app/components/SettingsDiagnosticsPanel.vue`

**Interfaces:**

- Consumes: the two component contracts from Tasks 1 and 2.
- Produces: a smaller settings orchestration view with unchanged behavior.

- [x] **Step 1: Remove only migrated parent styles**

Remove update/diagnostics selectors from the shared panel selector, their dedicated style blocks, their 760 px child-layout rules, and their reduced-motion selectors. Retain page scroll margins and every unrelated selector.

- [x] **Step 2: Run static checks**

Run:

```powershell
npm run typecheck
npm run lint
```

Expected: both commands exit 0 with no warnings.

- [x] **Step 3: Run the full frontend test suite**

Run:

```powershell
npm test -- --run
```

Expected: all test files pass.

- [x] **Step 4: Review the scoped diff**

Run:

```powershell
git diff --check
git diff -- src/app/views/SettingsView.vue src/app/components/SettingsUpdatePanel.vue src/app/components/SettingsDiagnosticsPanel.vue
```

Confirm that command orchestration remains in `SettingsView`, private updater/diagnostic fields are not passed into visible copy, and no unrelated worktree changes were modified.
