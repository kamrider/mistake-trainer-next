# Settings Mobile Header Action Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the Settings refresh action readable on narrow screens without changing its behavior or the desktop layout.

**Architecture:** Add a dedicated class to the existing Settings header action and protect its intrinsic width with a small scoped CSS contract. Lock the regression with a source-level layout test because jsdom does not calculate flex wrapping, then verify the actual 390px viewport in the local browser preview.

**Tech Stack:** Vue 3 SFC, scoped CSS, Vitest, Node `fs`, Testing Library, Vite browser preview

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support, account deletion, device migration/recovery, updater recovery, or SLA work.
- Preserve all existing working-tree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Keep the current refresh command, disabled-state expression, icon, and accessible name unchanged.
- The refresh action must remain a minimum 44px touch target at viewport widths up to 760px.

---

### Task 1: Settings mobile header action layout

**Files:**
- Create: `src/app/views/SettingsLayoutReadability.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**
- Consumes: the existing Settings header button that invokes `load` and the existing `@media (max-width: 760px)` touch-target rule.
- Produces: a `.settings-refresh` style contract containing `flex: 0 0 auto` and `white-space: nowrap`.

- [x] **Step 1: Write the failing source-level layout contract**

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const settingsPath = 'src/app/views/SettingsView.vue'

function source() {
  return readFileSync(resolve(settingsPath), 'utf8')
}

function declarations(selector: string) {
  const compact = source().replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('settings page layout readability contract', () => {
  it('keeps the refresh action readable beside the heading on narrow screens', () => {
    expect(source()).toContain('class="settings-refresh"')
    expect(declarations('.settings-refresh')).toContain('flex:00auto')
    expect(declarations('.settings-refresh')).toContain('white-space:nowrap')
  })
})
```

- [x] **Step 2: Run the contract and verify that it fails for the missing class**

Run: `pnpm exec vitest run src/app/views/SettingsLayoutReadability.test.ts`

Expected: FAIL because `SettingsView.vue` does not contain `class="settings-refresh"`.

- [x] **Step 3: Add the dedicated class and minimal intrinsic-width protection**

Add `class="settings-refresh"` to the header refresh button in `SettingsView.vue`, then add this scoped declaration next to the shared button rule:

```css
.settings-refresh { flex: 0 0 auto; white-space: nowrap; }
```

- [x] **Step 4: Run the focused component and layout tests**

Run: `pnpm exec vitest run src/app/views/SettingsLayoutReadability.test.ts src/app/views/SettingsView.test.ts`

Expected: both test files PASS, including the existing refresh behavior and busy-state coverage.

- [x] **Step 5: Verify the real desktop and narrow layouts**

Open `http://127.0.0.1:1420/`, navigate to Settings, and verify at the default desktop viewport and at `390x844` that:

- the action remains named `刷新` and stays on one line;
- the icon and label remain horizontally aligned;
- the heading and description retain usable width;
- `document.documentElement.scrollWidth` does not exceed the content viewport;
- the 44px narrow-screen touch target remains in effect.

- [x] **Step 6: Run the complete frontend quality gate**

Run: `pnpm test`

Expected: all frontend test files PASS.

Run: `pnpm lint`

Expected: PASS with zero warnings.

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm build`

Expected: Vite production build PASS.

- [x] **Step 7: Review only the scoped diff**

Run: `git diff --check`

Expected: PASS with no whitespace errors.

Run: `git diff -- src/app/views/SettingsView.vue src/app/views/SettingsLayoutReadability.test.ts docs/superpowers/plans/2026-08-08-settings-mobile-header-action.md`

Expected: `SettingsView.vue` also contains pre-existing dirty changes, so review this task's exact additions at the `.settings-refresh` template and style declarations; the new regression contract and this plan remain independently inspectable as untracked files.
