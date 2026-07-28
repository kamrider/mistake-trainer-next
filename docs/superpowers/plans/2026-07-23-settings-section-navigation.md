# Settings Section Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the long settings page immediately navigable, especially the frequently used subject preferences, with a sticky paper-tab directory that follows the visible section.

**Architecture:** A focused `SettingsSectionNav` component owns section selection, smooth/reduced-motion scrolling, and `IntersectionObserver` scroll-spy state. `SettingsView` only declares the sections that currently exist and gives its existing panels stable anchor IDs, so settings persistence and backend commands remain untouched.

**Tech Stack:** Vue 3 Composition API, CSS transform/opacity transitions, IntersectionObserver, Vitest, Testing Library.

## Global Constraints

- The directory exposes only sections currently rendered by `SettingsView`.
- Section order is `sync`, `overview`, `subjects`, `review`, `backup`, `migration`.
- Clicking a directory item calls `scrollIntoView({ block: 'start', behavior: 'smooth' })`.
- When `prefers-reduced-motion: reduce` matches, scrolling uses `behavior: 'auto'` and all indicator/button transitions are disabled.
- The active item uses `aria-current="location"`; the moving indicator is `aria-hidden="true"`.
- The desktop directory remains sticky; compact screens use one horizontally scrollable track without increasing the page's layout width.
- Existing settings commands, conditional panels, error handling, and persistence behavior must not change.

---

### Task 1: Build the reusable settings directory

**Files:**
- Create: `src/app/components/SettingsSectionNav.vue`
- Create: `src/app/components/SettingsSectionNav.test.ts`

**Interfaces:**
- Consumes:

```ts
export interface SettingsSectionLink {
  id: string
  label: string
  hint: string
}

defineProps<{ sections: SettingsSectionLink[] }>()
```

- Produces: one `nav[aria-label="设置目录"]`, `.settings-section-indicator`, and active `aria-current="location"` state.

- [x] **Step 1: Write failing navigation tests**

Render the component beside real test sections and assert:

```ts
await user.click(screen.getByRole('button', { name: /科目配置/ }))
expect(subjectSection.scrollIntoView).toHaveBeenCalledWith({
  block: 'start',
  behavior: 'smooth',
})
expect(screen.getByRole('button', { name: /科目配置/ }))
  .toHaveAttribute('aria-current', 'location')
```

Add a reduced-motion case that expects `behavior: 'auto'`, and an observer case that invokes the captured callback for `settings-review` and expects the training item to become current.

- [x] **Step 2: Run the focused test and verify failure**

Run:

```powershell
corepack pnpm exec vitest run src/app/components/SettingsSectionNav.test.ts
```

Expected: FAIL because the component does not exist.

- [x] **Step 3: Implement selection, scroll-spy, and cleanup**

Implement:

```ts
const activeId = ref(props.sections[0]?.id ?? '')
const activeIndex = computed(() =>
  Math.max(0, props.sections.findIndex(section => section.id === activeId.value)),
)

function selectSection(id: string) {
  activeId.value = id
  document.getElementById(id)?.scrollIntoView({
    block: 'start',
    behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches
      ? 'auto'
      : 'smooth',
  })
}
```

Bind an `IntersectionObserver` with `rootMargin: '-120px 0px -55% 0px'` and thresholds `[0, .25, .6]`. Rebind when the ordered section-ID list changes, choose the intersecting entry nearest the 120 px sticky line, and disconnect on unmount.

Render a six-column desktop track with one transform-only shared indicator. On screens below 760 px, keep the track at a bounded minimum width inside an overflow container. Disable transitions in reduced motion.

- [x] **Step 4: Run the focused test**

Run:

```powershell
corepack pnpm exec vitest run src/app/components/SettingsSectionNav.test.ts
```

Expected: PASS.

---

### Task 2: Integrate anchors into the real settings page

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `SettingsSectionNav`, `SettingsSectionLink`, and existing `overview`, `subjectPreferences`, and `reviewPreferences` load state.
- Produces: stable IDs `settings-sync`, `settings-overview`, `settings-subjects`, `settings-review`, `settings-backup`, and `settings-migration`.

- [x] **Step 1: Write the failing settings integration test**

Stub `HTMLElement.prototype.scrollIntoView`, render `SettingsView`, click the directory item named `科目配置`, and assert that the existing subject panel received the smooth scroll request.

- [x] **Step 2: Run the focused tests and verify failure**

Run:

```powershell
corepack pnpm exec vitest run src/app/components/SettingsSectionNav.test.ts src/app/views/SettingsView.test.ts
```

Expected: FAIL because `SettingsView` does not render the directory or anchor IDs.

- [x] **Step 3: Declare only currently available section links**

Add:

```ts
const settingsSections = computed<SettingsSectionLink[]>(() => [
  { id: 'settings-sync', label: '同步账户', hint: '本地与云端' },
  ...(overview.value ? [{ id: 'settings-overview', label: '本机概况', hint: '题库与冲突' }] : []),
  ...(subjectPreferences.value ? [{ id: 'settings-subjects', label: '科目配置', hint: '采集常用项' }] : []),
  ...(reviewPreferences.value ? [{ id: 'settings-review', label: '训练节奏', hint: '专注插曲' }] : []),
  { id: 'settings-backup', label: '备份恢复', hint: '完整快照' },
  { id: 'settings-migration', label: '旧版迁移', hint: '安全导入' },
])
```

Render `<SettingsSectionNav :sections="settingsSections" />` after the page header. Add the six IDs to existing panels; wrap `LegacyImportPanel` in the migration anchor. Add `scroll-margin-top: 118px` to anchors without changing their visual box model.

- [x] **Step 4: Run focused tests**

Run:

```powershell
corepack pnpm exec vitest run src/app/components/SettingsSectionNav.test.ts src/app/views/SettingsView.test.ts
```

Expected: PASS.

---

### Task 3: Quality gate and local baseline

**Files:**
- Verify all files above and this plan.

**Interfaces:**
- Consumes: complete settings directory increment.
- Produces: one clean local Git commit.

- [x] **Step 1: Run repository checks**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
git diff --check
```

Expected: all commands exit 0; initial application JavaScript remains below 300 KB gzip.

- [x] **Step 2: Verify the rendered page**

Open `/settings` at desktop and 375 px widths. Confirm that the directory stays reachable, the current tab follows scroll, clicking `科目配置` reveals the panel, and neither viewport gains horizontal page overflow.

- [x] **Step 3: Commit**

```powershell
git add src/app/components/SettingsSectionNav.vue src/app/components/SettingsSectionNav.test.ts src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts docs/superpowers/plans/2026-07-23-settings-section-navigation.md
git commit -m "feat: add settings section navigation"
```

---

## Self-Review

- Spec coverage: quick access, dynamic availability, sticky desktop behavior, compact overflow containment, scroll-spy, reduced motion, and accessibility each have implementation and verification steps.
- Placeholder scan: no `TBD`, `TODO`, unspecified error-handling step, or deferred test remains.
- Type consistency: section IDs and `SettingsSectionLink` names are identical across component, integration, tests, and CSS anchors.
