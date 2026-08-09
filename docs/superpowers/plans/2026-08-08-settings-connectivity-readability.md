# Settings Connectivity Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make account, sync-backend, and device-status settings consistently legible and touch-safe at every viewport width.

**Architecture:** Treat `SettingsCloudAuthPanel.vue`, `SettingsSyncBackendPanel.vue`, and `SettingsDeviceOverviewPanel.vue` as one connectivity-settings UI boundary and enforce it with a source-level Vitest contract. Keep authentication intents, credentials, sync commands, backend selection, compatibility reporting, lock intentions, status semantics, and emitted events unchanged; modify only scoped CSS font sizes and control minimum heights.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Every explicit visible pixel font in the three target components must be at least 12 px.
- Cloud-auth buttons and inputs, sync-backend buttons, and device-overview buttons must provide at least a 44 px target at every viewport width.
- Do not change authentication, credentials, sync operations, backend availability/selection, Windows compatibility, device protection status, lock behavior, or emitted events.
- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the connectivity-settings commercial UI contract

**Files:**
- Create: `src/app/components/SettingsConnectivityReadability.test.ts`

**Interfaces:**
- Consumes: scoped CSS selectors in the three connectivity-settings components.
- Produces: a regression contract for the 12 px text floor and named 44 px control targets.

- [x] **Step 1: Write the failing source contract**

Create the following test:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const cloudPath = 'src/app/components/SettingsCloudAuthPanel.vue'
const backendPath = 'src/app/components/SettingsSyncBackendPanel.vue'
const devicePath = 'src/app/components/SettingsDeviceOverviewPanel.vue'
const componentPaths = [cloudPath, backendPath, devicePath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('settings connectivity readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )
    expect(violations).toEqual([])
  })

  it('keeps cloud account controls touch-safe at every viewport width', () => {
    expect(declarations(cloudPath, 'button')).toContain('min-height:44px')
    expect(declarations(cloudPath, '.cloud-auth-form input')).toContain('min-height:44px')
    expect(declarations(cloudPath, '.auth-mode-toggle')).toContain('min-height:44px')
  })

  it('keeps backend and device controls touch-safe at every viewport width', () => {
    expect(declarations(backendPath, 'button')).toContain('min-height:44px')
    expect(declarations(devicePath, 'button')).toContain('min-height:44px')
  })
})
```

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/app/components/SettingsConnectivityReadability.test.ts`

Expected: all three tests FAIL because thirteen declarations use 9–11 px text and the controls rely on a mobile-only 44 px rule.

### Task 2: Raise cloud authentication to the commercial baseline

**Files:**
- Modify: `src/app/components/SettingsCloudAuthPanel.vue:220-377`

**Interfaces:**
- Consumes: existing cloud-auth scoped CSS selectors.
- Produces: the cloud side of `SettingsConnectivityReadability.test.ts`; no script or template changes.

- [x] **Step 1: Make account controls touch-safe at every width**

Add `min-height: 44px` to the existing `button` and `.cloud-auth-form input` declarations, and change `.auth-mode-toggle` from `min-height: 32px` to `44px`.

```css
button { min-height: 44px; }
.cloud-auth-form input { min-height: 44px; }
.auth-mode-toggle { min-height: 44px; }
```

- [x] **Step 2: Raise all cloud-auth guidance to 12 px**

Change `.regional-sync-note`, `.regional-sync-note strong`, `.cloud-auth-form label`, `.auth-mode-toggle`, and `.backend-message` to `font-size: 12px` within their existing declarations.

### Task 3: Raise backend and device settings to the commercial baseline

**Files:**
- Modify: `src/app/components/SettingsSyncBackendPanel.vue:158-343`
- Modify: `src/app/components/SettingsDeviceOverviewPanel.vue:154-347`

**Interfaces:**
- Consumes: existing backend/device scoped CSS selectors.
- Produces: the backend/device side of `SettingsConnectivityReadability.test.ts`; no script or template changes.

- [x] **Step 1: Make backend and device actions touch-safe at every width**

Add `min-height: 44px` to each component's existing base `button` declaration. The backend provider cards keep their existing `min-height: 92px` override.

```css
button {
  min-height: 44px;
}
```

- [x] **Step 2: Raise backend metadata to 12 px**

Change `.backend-current small`, `.backend-option small`, `.capability-badge`, and `.backend-message` to `font-size: 12px` within their existing declarations.

- [x] **Step 3: Raise device metadata to 12 px**

Change `.setting-card p`, `.setting-card > strong`, `.device-security-list dt`, and `.lock-now` to `font-size: 12px` within their existing declarations.

- [x] **Step 4: Run the contract and existing behavior suites**

Run: `npm test -- src/app/components/SettingsConnectivityReadability.test.ts src/app/components/SettingsCloudAuthPanel.test.ts src/app/components/SettingsSyncBackendPanel.test.ts src/app/components/SettingsDeviceOverviewPanel.test.ts`

Expected: all four files PASS; credential intent privacy, authentication/sync intentions, unavailable provider locking, backend failures, truthful device status, compatibility information, and lock intentions remain intact.

### Task 4: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-settings-connectivity-readability.md`

**Interfaces:**
- Consumes: completed styles and tests from Tasks 1–3.
- Produces: exact verification records and a completed checklist.

- [x] **Step 1: Run static validation**

Run: `npm run lint`

Expected: PASS with zero warnings.

Run: `npm run typecheck`

Expected: PASS with zero TypeScript errors.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: PASS through mobile asset verification, Vue type checking, and Vite production bundling.

- [x] **Step 3: Run the full frontend suite**

Run: `npm test`

Expected: all Vitest files and tests PASS.

- [x] **Step 4: Review scope and record completion**

Inspect the four target source files and this plan, run `git diff --check` on the Vue and test files, confirm no script/template behavior or excluded functionality changed, replace every plan checkbox with `[x]`, and append the exact focused, build, and full-suite counts.

## Verification Record

- Red contract: `SettingsConnectivityReadability.test.ts` failed all 3 tests as expected, identifying 13 declarations below 12 px plus missing base 44 px control targets.
- Focused regression: 4 test files passed, 12 tests passed.
- Static validation: `npm run lint` passed with zero warnings; `npm run typecheck` passed with zero TypeScript errors.
- Production build: `npm run build` passed; Vite transformed 2054 modules.
- Full frontend regression: 143 test files passed, 787 tests passed.
- Local code review: no Critical or Important findings. All three target components have a 12 px explicit font floor, base controls retain a 44 px minimum target, backend provider cards retain their 92 px minimum height, and narrow layouts remain single-column.
- Scope review: only scoped styles and the source-level readability contract changed in the target components; authentication, sync, backend selection, device status, lock behavior, excluded launch-only functionality, and `src-tauri/src/infrastructure/recognition_visual_split.rs` were not changed by this batch.
- Hygiene: target files passed `git diff --check`; no files were staged or committed.
