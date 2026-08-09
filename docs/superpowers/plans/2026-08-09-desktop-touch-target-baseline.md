# Desktop Touch Target Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure the nine audited desktop settings and report actions expose a minimum 44px click target, then publish the verified change on `main` and rebuild the Windows installer.

**Architecture:** Keep each component's existing visual design and add only a `min-height: 44px` constraint to its existing scoped button baseline. Extend the source-contract readability test so future desktop-only regressions fail before packaging.

**Tech Stack:** Vue 3 single-file components, scoped CSS, Vitest, pnpm, Tauri 2, NSIS.

## Global Constraints

- Preserve existing colors, spacing, labels, and component behavior.
- Apply the 44px baseline on desktop as well as mobile.
- Do not change native checkbox or radio geometry when their wrapping label already provides the hit target.
- Commit to the explicitly requested `main` branch only after tests and review pass.

---

### Task 1: Add the desktop click-target regression contract

**Files:**
- Modify: `src/app/AppChromeReadability.test.ts`

**Interfaces:**
- Consumes: existing `declarations(componentPath, selector)` source-style helper.
- Produces: a Vitest contract covering the five scoped CSS baselines that own all nine audited actions.

- [x] **Step 1: Write the failing test**

Add this test to the existing `application chrome readability contract` suite:

```ts
it('keeps desktop settings and report actions at the 44px baseline', () => {
  const baselines = [
    ['src/app/views/SettingsView.vue', 'button'],
    ['src/app/components/SettingsBackupPanel.vue', 'button'],
    ['src/app/components/SettingsStoragePanel.vue', 'button'],
    ['src/app/components/SettingsUpdatePanel.vue', 'button'],
    ['src/app/views/ReportView.vue', '.page-heading button'],
  ] as const

  for (const [componentPath, selector] of baselines) {
    expect(declarations(componentPath, selector), `${componentPath} ${selector}`)
      .toContain('min-height:44px')
  }
})
```

- [x] **Step 2: Run the focused test and verify it fails**

Run: `pnpm vitest run src/app/AppChromeReadability.test.ts`

Expected: FAIL because the five desktop rules do not yet declare `min-height: 44px`.

### Task 2: Apply the 44px desktop baseline

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/components/SettingsBackupPanel.vue`
- Modify: `src/app/components/SettingsStoragePanel.vue`
- Modify: `src/app/components/SettingsUpdatePanel.vue`
- Modify: `src/app/views/ReportView.vue`

**Interfaces:**
- Consumes: the failing source-contract test from Task 1.
- Produces: `min-height: 44px` on all nine audited action buttons without altering event handlers or layout structure.

- [x] **Step 1: Update the four settings button baselines**

In each existing scoped `button` rule for `SettingsView.vue`, `SettingsBackupPanel.vue`, `SettingsStoragePanel.vue`, and `SettingsUpdatePanel.vue`, add:

```css
min-height: 44px;
```

- [x] **Step 2: Update the report header action**

Change the existing report rule to:

```css
.page-heading button { min-height: 44px; padding: 10px 14px; }
```

- [x] **Step 3: Run the focused test and verify it passes**

Run: `pnpm vitest run src/app/AppChromeReadability.test.ts`

Expected: PASS.

- [x] **Step 4: Run project verification**

Run: `pnpm test`

Expected: all Vitest files and tests pass.

Run: `pnpm lint`

Expected: exit code 0 with no warnings.

Run: `pnpm build`

Expected: type checking and Vite production build complete successfully.

### Task 3: Review, publish, and package

**Files:**
- Verify: all files changed by Tasks 1 and 2.
- Create: Windows NSIS installer under `src-tauri/target/release/bundle/nsis/`.

**Interfaces:**
- Consumes: verified source changes and tests.
- Produces: reviewed commit on `main`, pushed `main`, and a rebuilt x64 installer with SHA-256 receipt.

- [x] **Step 1: Review the diff**

Run: `git diff --check` and request a focused code review against the pre-change SHA.

Expected: no whitespace errors and no Critical or Important review findings.

- [ ] **Step 2: Commit on main**

Run:

```text
git add docs/superpowers/plans/2026-08-09-desktop-touch-target-baseline.md src/app/AppChromeReadability.test.ts src/app/views/SettingsView.vue src/app/components/SettingsBackupPanel.vue src/app/components/SettingsStoragePanel.vue src/app/components/SettingsUpdatePanel.vue src/app/views/ReportView.vue
git commit -m "fix: standardize desktop action hit targets"
```

Expected: one commit created on `main`.

- [ ] **Step 3: Push main**

Run: `git push origin main`

Expected: local `main` and `origin/main` point to the new commit.

- [ ] **Step 4: Build and verify the installer**

Run: `pnpm tauri build --bundles nsis`

Expected: `Mistake Trainer Next_0.1.0-rc.1_x64-setup.exe` is produced. Record its absolute path, byte size, SHA-256 hash, and signature status.

## Self-Review

- Spec coverage: the plan covers all nine audited actions, regression testing, full verification, review, `main` publication, and installer construction.
- Placeholder scan: no unfinished placeholders remain.
- Type consistency: the test reuses the existing `declarations` helper and all selectors exactly match existing scoped rules.
