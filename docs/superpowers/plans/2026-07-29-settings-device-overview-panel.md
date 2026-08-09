# Settings Device Overview Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the current-device protection and library overview UI from `SettingsView.vue` while preserving truthful degraded states, security copy, lock confirmation, and focus restoration.

**Architecture:** `SettingsDeviceOverviewPanel.vue` is a presentation component that consumes typed snapshots and emits a `requestLock` intention. It never imports Tauri commands. It exposes `focusLockAction()` so `SettingsView.vue` can restore focus after the confirmation dialog without passing a browser `Event` through the component boundary.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Vitest, Testing Library, Lucide Vue, generated Tauri bindings.

## Global Constraints

- Do not modify OCR, recognition, migration, account deletion, privacy, licensing, support, updater recovery, or SLA behavior.
- Do not stage or commit the existing dirty worktree.
- Preserve the `settings-overview` anchor, accessible names, local-first copy, and truthful unavailable states.
- Keep all `commands.*` calls and lock orchestration in `SettingsView.vue`.

---

### Task 1: Define the device overview presentation contract

**Files:**
- Create: `src/app/components/SettingsDeviceOverviewPanel.test.ts`

**Interfaces:**
- Consumes: `SettingsOverview`, `LibraryAccessStatus | undefined`, `CloudAuthState | undefined`, `WindowsCompatibilityStatus | undefined`, `loading: boolean`, `accessError: string`.
- Produces: behavioral requirements for `requestLock` and honest state rendering.

- [x] **Step 1: Write the normal-state test**

Render encrypted local state, a trusted Windows account, connected cloud state, compatibility details, counts, and the lock button. Click the button and assert `requestLock` emits once.

- [x] **Step 2: Write the degraded-state test**

Render `localEncryptionReady: false`, no access snapshot, no cloud snapshot, and an access error. Assert the UI says “正在检查加密状态”, “状态暂不可用”, and “正在检查”, and never claims the Windows account can unlock.

- [x] **Step 3: Run the focused test**

Run:

```powershell
npm test -- --run src/app/components/SettingsDeviceOverviewPanel.test.ts
```

Expected: failure because `SettingsDeviceOverviewPanel.vue` does not exist.

### Task 2: Implement the presentation component

**Files:**
- Create: `src/app/components/SettingsDeviceOverviewPanel.vue`

**Interfaces:**
- Produces:

```ts
defineEmits<{ requestLock: [] }>()
defineExpose<{ focusLockAction: () => void }>()
```

- [x] **Step 1: Add typed props and derived transition key**

Compute the transition key exclusively from non-sensitive status categories:

```ts
const deviceStatusKey = computed(() => [
  props.accessStatus?.trustedWindowsAccount ? 'trusted' : 'unavailable',
  props.cloudAuth?.status.kind ?? 'checking',
  props.accessError ? 'error' : 'ready',
].join(':'))
```

- [x] **Step 2: Move the complete overview template**

Move the security card, problem counts, Windows compatibility, cloud configuration, and attention counts. Preserve `id="settings-overview"` and existing copy.

- [x] **Step 3: Own scoped responsive styles**

Move `.settings-grid`, `.setting-card`, device status, lock action, transition, 760px grid/target, and reduced-motion styles. Include base `button` and `h2` styles because parent scoped rules do not style child internals.

- [x] **Step 4: Run the focused component tests**

Run the Task 1 command. Expected: all component tests pass.

### Task 3: Reconnect lock orchestration without DOM events

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Verify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `focusLockAction()` from the new component.

- [x] **Step 1: Replace inline markup**

Render:

```vue
<SettingsDeviceOverviewPanel
  v-if="overview"
  ref="deviceOverviewPanel"
  :overview="overview"
  :access-status="deviceAccessStatus"
  :access-error="deviceAccessError"
  :cloud-auth="cloudAuth"
  :windows-compatibility="windowsCompatibility"
  :loading="loading"
  @request-lock="openLibraryLock('lock')"
/>
```

- [x] **Step 2: Remove browser-event focus state**

Delete `lockReturnFocus`, make `openLibraryLock(mode)` event-free, and restore focus through `cloudAuthPanel.focusSignOutAction()` or `deviceOverviewPanel.focusLockAction()`.

- [x] **Step 3: Remove only migrated styles**

Keep page, roadmap, migration, loading-state, and conflict styles in the parent.

- [x] **Step 4: Run integration tests**

Run:

```powershell
npm test -- --run src/app/views/SettingsView.test.ts src/app/components/SettingsDeviceOverviewPanel.test.ts
```

Expected: existing lock-dialog focus and degraded-state tests pass unchanged.

### Task 4: Verify commercial quality

- [x] **Step 1: Run static and full automated gates**

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run contract:rust-boundaries
```

Expected: every command exits with code 0.

- [x] **Step 2: Perform self-review**

Inspect the scoped diff for command leakage, sensitive identifiers, fabricated ready states, event objects crossing the component boundary, missing focus restoration, and unrelated files.

- [x] **Step 3: Exercise real responsive behavior**

Use the actual component in a temporary local harness. Verify desktop and 375px layouts, no horizontal overflow, a minimum 44px narrow-screen lock target, cancel-focus behavior through the real settings integration tests, and no browser warnings/errors. Remove the harness after verification.
