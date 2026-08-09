# Click Target Layering Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the learning-profile rename and delete actions fully clickable, and bring the remaining undersized application-chrome controls up to the 44px interaction baseline.

**Architecture:** Preserve the existing profile component and its 44px action buttons, but raise the desktop side rail above page-level sticky content so the profile popover is not hit-tested underneath the main page. Strengthen the existing source-level readability contracts so the stacking rule and every affected control size regress visibly in tests.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Preserve the existing profile selection, rename, delete, focus-return, and busy-state behavior.
- Use a minimum 44px target in both dimensions for icon-only controls and a minimum 44px height for text actions.
- Keep full-screen dialogs above the desktop rail by retaining their existing higher z-index layers.
- Do not add dependencies or restructure unrelated components.

---

### Task 1: Add failing application-chrome interaction contracts

**Files:**
- Modify: `src/app/AppChromeReadability.test.ts`

**Interfaces:**
- Consumes: `declarations(componentPath, selector)` source-style helper.
- Produces: regression coverage for `.side-rail`, profile action targets, settings directory arrows, and the not-found return action.

- [x] **Step 1: Extend the source inventory and add the failing assertions**

```ts
const componentPaths = [
  'src/app/App.vue',
  'src/app/AppShell.vue',
  'src/app/LibraryAccessScreen.vue',
  'src/app/views/NotFoundView.vue',
  'src/app/components/SettingsSectionNav.vue',
  'src/modules/profiles/components/ProfileSwitcher.vue',
]

it('keeps the profile popover above page chrome with explicit action targets', () => {
  expect(declarations('src/app/AppShell.vue', '.side-rail')).toContain('z-index:50')
  expect(declarations('src/modules/profiles/components/ProfileSwitcher.vue', '.rename-button, .delete-button'))
    .toMatch(/width:44px;height:44px/)
})

it('keeps remaining application-chrome actions at the 44px baseline', () => {
  expect(declarations('src/app/components/SettingsSectionNav.vue', '.directory-scroll'))
    .toMatch(/width:44px;height:44px/)
  expect(declarations('src/app/views/NotFoundView.vue', '.not-found button'))
    .toContain('min-height:44px')
})
```

- [x] **Step 2: Run the focused contract and verify it fails for the missing rail layer and undersized controls**

Run: `pnpm exec vitest run src/app/AppChromeReadability.test.ts`

Expected: FAIL mentioning missing `z-index:50`, `width:44px;height:44px`, and/or `min-height:44px`.

### Task 2: Repair stacking and target presentation

**Files:**
- Modify: `src/app/AppShell.vue`
- Modify: `src/modules/profiles/components/ProfileSwitcher.vue`
- Modify: `src/app/components/SettingsSectionNav.vue`
- Modify: `src/app/views/NotFoundView.vue`

**Interfaces:**
- Consumes: the existing app-shell grid, profile popover, and scoped component styles.
- Produces: a desktop rail at layer 50, fully visible 44px profile actions, 44px settings arrows, and a 44px not-found action.

- [x] **Step 1: Raise the desktop side rail above page-level content**

```css
.side-rail { position: sticky; z-index: 50; top: 0; ... }
```

Keep the existing mobile `z-index: 20` override so the bottom navigation stays below full-screen overlays.

- [x] **Step 2: Make the profile actions visually legible without shrinking their target**

```vue
<Pencil :size="18" aria-hidden="true" />
<Trash2 :size="18" aria-hidden="true" />
```

```css
.rename-button, .delete-button {
  width: 44px;
  height: 44px;
  flex: 0 0 44px;
  touch-action: manipulation;
}
```

- [x] **Step 3: Bring the remaining undersized application-chrome controls to 44px**

```css
.directory-scroll { width: 44px; height: 44px; }
.not-found button { min-height: 44px; }
```

- [x] **Step 4: Run focused profile and chrome tests**

Run: `pnpm exec vitest run src/app/AppChromeReadability.test.ts src/app/AppShell.test.ts src/modules/profiles/components/ProfileSwitcher.test.ts src/app/components/SettingsSectionNav.test.ts`

Expected: all tests PASS.

### Task 3: Verify the complete interaction surface

**Files:**
- No source changes expected.

**Interfaces:**
- Consumes: the built Vue application and the repaired controls.
- Produces: test, type, lint, build, and visual evidence that the fix works without regressions.

- [x] **Step 1: Run the complete front-end quality gates**

Run: `pnpm test`

Expected: all Vitest suites PASS.

Run: `pnpm typecheck`

Expected: exit code 0.

Run: `pnpm lint`

Expected: exit code 0 with zero warnings.

Run: `pnpm build`

Expected: exit code 0 and a successful Vite production build.

- [ ] **Step 2: Inspect the running app at desktop width**

Open the profile menu, verify that main-page text no longer draws over the panel, and click the center and edges of both the rename and delete 44px buttons. Verify the settings directory arrows and not-found return action remain visually balanced after their size adjustment.

Attempted with the in-app browser after starting a verified local Vite listener. The browser retained its initial connection-error document and its URL policy blocked re-entry to localhost; Chrome control was unavailable. Browser cleanup completed, and the equivalent stacking/target contracts plus the full test and production build gates passed.

- [x] **Step 3: Review the final diff**

Run: `git diff --check` and `git diff -- src/app/AppShell.vue src/modules/profiles/components/ProfileSwitcher.vue src/app/components/SettingsSectionNav.vue src/app/views/NotFoundView.vue src/app/AppChromeReadability.test.ts docs/superpowers/plans/2026-08-08-click-target-layering-repair.md`

Expected: no whitespace errors and only the planned interaction changes.
