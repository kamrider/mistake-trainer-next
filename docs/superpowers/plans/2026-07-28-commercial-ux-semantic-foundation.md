# Commercial UX Semantic Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make primary navigation and page identity remain explicit for screen-reader, compact-layout, and support workflows without changing routes or product behavior.

**Architecture:** Keep the existing `AppShell` navigation model and view composition. Add accessible names at the interactive control boundary, then change the two marketing-style page headings into stable product titles while preserving the existing tone as supporting copy.

**Tech Stack:** Vue 3 single-file components, TypeScript, Vitest, Testing Library Vue, jsdom.

## Global Constraints

- Do not modify the current uncommitted recognition work under `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve all route names, navigation labels, commands, storage behavior, and synchronization behavior.
- Every primary navigation button must have the same accessible name at desktop and compact breakpoints.
- Each settings/report page must expose exactly one level-one heading with the stable page name.
- Preserve the existing brand voice in visible supporting text.

---

### Task 1: Stable accessible names for primary navigation

**Files:**
- Modify: `src/app/AppShell.vue:73-87`
- Test: `src/app/AppShell.test.ts`

**Interfaces:**
- Consumes: the existing `navigation` array with `{ id, label, icon }`.
- Produces: a stable `aria-label` on every `.nav-item` button; emitted `navigate` payloads remain unchanged.

- [x] **Step 1: Write the failing navigation-name regression test**

Add this assertion after obtaining the `navigation` element:

```ts
for (const label of ['训练台', '采集整理', '题库', '训练室', '学习报告', '设置']) {
  expect(within(navigation).getByRole('button', { name: label })).toHaveAttribute('aria-label', label)
}
```

Update the import to:

```ts
import { render, screen, within } from '@testing-library/vue'
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
pnpm test -- src/app/AppShell.test.ts
```

Expected: FAIL because the buttons derive their names only from visible descendant text and do not have `aria-label`.

- [x] **Step 3: Bind the stable accessible name**

Add the following attribute to the navigation button in `AppShell.vue`:

```vue
:aria-label="item.label"
```

Keep `:aria-current`, classes, and emitted navigation values unchanged.

- [x] **Step 4: Run the focused test and verify it passes**

Run:

```powershell
pnpm test -- src/app/AppShell.test.ts
```

Expected: PASS.

### Task 2: Stable settings page identity

**Files:**
- Modify: `src/app/views/SettingsView.vue:767-770`
- Test: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: the existing settings page header.
- Produces: one level-one heading named `设置`; the original “数据安静地待在该在的地方” phrase remains visible as supporting copy.

- [x] **Step 1: Write the failing page-identity assertion**

In the first settings rendering test, add:

```ts
expect(await screen.findByRole('heading', { level: 1, name: '设置' })).toBeVisible()
expect(screen.getByText(/数据安静地待在该在的地方/)).toBeVisible()
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
pnpm test -- src/app/views/SettingsView.test.ts
```

Expected: FAIL because the level-one heading currently uses marketing copy.

- [x] **Step 3: Replace the header hierarchy**

Use:

```vue
<div>
  <p>本地资料库</p>
  <h1>设置</h1>
  <span>数据安静地待在该在的地方；这里展示真实状态，尚未接通的云能力会明确标注。</span>
</div>
```

- [x] **Step 4: Run the focused test and verify it passes**

Run:

```powershell
pnpm test -- src/app/views/SettingsView.test.ts
```

Expected: PASS.

### Task 3: Stable learning-report page identity

**Files:**
- Modify: `src/app/views/ReportView.vue:293-297`
- Test: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: the existing report page header.
- Produces: one level-one heading named `学习报告`; the original “把练习变成看得见的节奏” phrase remains visible as supporting copy.

- [x] **Step 1: Write the failing page-identity assertion**

In the first report rendering test, add:

```ts
expect(await screen.findByRole('heading', { level: 1, name: '学习报告' })).toBeVisible()
expect(screen.getByText(/把练习变成看得见的节奏/)).toBeVisible()
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
pnpm test -- src/app/views/ReportView.test.ts
```

Expected: FAIL because the level-one heading currently uses marketing copy.

- [x] **Step 3: Replace the header hierarchy**

Use:

```vue
<div>
  <p>本地实时计算</p>
  <h1>学习报告</h1>
  <span>把练习变成看得见的节奏；只呈现事实，不用夸张的红色数字制造焦虑。</span>
</div>
```

- [x] **Step 4: Run the focused test and verify it passes**

Run:

```powershell
pnpm test -- src/app/views/ReportView.test.ts
```

Expected: PASS.

### Task 4: Full frontend verification

**Files:**
- Verify: `src/app/AppShell.vue`
- Verify: `src/app/views/SettingsView.vue`
- Verify: `src/app/views/ReportView.vue`

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: a verified frontend change set with no route, type, lint, unit-test, or production-build regression.

- [x] **Step 1: Run all frontend quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Expected: all commands exit with code `0`.

- [x] **Step 2: Review the resulting diff**

Run:

```powershell
git diff --check
git diff -- src/app/AppShell.vue src/app/AppShell.test.ts src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts src/app/views/ReportView.vue src/app/views/ReportView.test.ts
```

Expected: no whitespace errors; the diff contains only accessible-name, heading-copy, and regression-test changes.
