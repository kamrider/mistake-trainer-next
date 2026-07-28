# Directional Navigation Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every desktop and compact-screen page change a clear spatial direction, animate one shared paper-tab navigation indicator, and remove all route motion when Windows requests reduced motion.

**Architecture:** `AppShell` maps the six stable navigation destinations to one CSS custom-property index and renders a single inert indicator behind the buttons. `App` compares the previous and next shell-page index, exposes `forward | backward` on the route container, and selects bounded 180/100 ms Vue transition classes. No route, command, or persisted state changes.

**Tech Stack:** Vue 3 Composition API, Vue Router, CSS transform/opacity transitions, Vitest, Testing Library.

## Global Constraints

- Navigation order is exactly `dashboard`, `inbox`, `library`, `review`, `report`, `settings`.
- The shared indicator animates only `transform`; page motion animates only `transform` and `opacity`.
- Forward means a higher navigation index; backward means a lower navigation index.
- Route leave lasts at most 100 ms and route enter lasts at most 180 ms.
- `prefers-reduced-motion: reduce` removes indicator and page transitions and resets transformed states.
- Active-page semantics remain `aria-current="page"`; the indicator is `aria-hidden="true"`.
- Existing route-error recovery and rapid sidebar-cycle tests must continue to pass.

---

### Task 1: Lock shared-indicator and direction contracts

**Files:**
- Modify: `src/app/AppShell.test.ts`
- Modify: `src/app/App.test.ts`

**Interfaces:**
- Consumes: `AppShell.activePage`, router names, existing `.route-page`.
- Produces: `.nav-indicator`, navigation style `--active-index`, and route attribute `data-direction="forward|backward"`.

- [x] **Step 1: Add failing shared-indicator assertions**

In `AppShell.test.ts`, assert that:

```ts
const navigation = screen.getByRole('navigation', { name: '主导航' })
expect(navigation).toHaveStyle({ '--active-index': '0' })
expect(view.container.querySelector('.nav-indicator')).toHaveAttribute('aria-hidden', 'true')
await view.rerender({ activePage: 'library' })
expect(navigation).toHaveStyle({ '--active-index': '2' })
```

- [x] **Step 2: Add failing route-direction assertions**

In `App.test.ts`, start on the dashboard, click `设置`, wait for `.route-page[data-direction="forward"]`, click `题库`, then wait for `.route-page[data-direction="backward"]`.

- [x] **Step 3: Run focused tests and verify failure**

Run:

```powershell
corepack pnpm exec vitest run src/app/AppShell.test.ts src/app/App.test.ts
```

Expected: FAIL because neither indicator state nor route direction exists.

---

### Task 2: Implement directional page and shared navigation motion

**Files:**
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/App.vue`

**Interfaces:**
- Consumes: the stable navigation array and `activePage`.
- Produces: `activeNavigationIndex`, `pageDirection`, `pageTransitionName`, `.nav-indicator`, and direction-specific transition classes.

- [x] **Step 1: Add one shared navigation indicator**

Capture props and compute the index:

```ts
const props = defineProps<...>()
const activeNavigationIndex = computed(() =>
  Math.max(0, navigation.findIndex(item => item.id === props.activePage)),
)
```

Render the navigation with `:style="{ '--active-index': activeNavigationIndex }"` and one `<span class="nav-indicator" aria-hidden="true" />` before the buttons.

- [x] **Step 2: Add bounded desktop and mobile indicator motion**

Desktop uses a 43 px indicator and a Vue-computed `--active-y` offset in 48 px steps. At `max-width: 760px`, use width `calc(100% / 6)`, height `50px`, and a Vue-computed `--active-x` offset in 100% steps. This avoids newer CSS multiplication syntax while buttons remain above the indicator and retain `aria-current`.

- [x] **Step 3: Track page direction**

Add a stable index map and watch `activePage`:

```ts
type PageDirection = 'forward' | 'backward'
const pageDirection = ref<PageDirection>('forward')
watch(activePage, (next, previous) => {
  if (next === previous) return
  pageDirection.value = pageOrder[next] >= pageOrder[previous] ? 'forward' : 'backward'
})
const pageTransitionName = computed(() => `page-${pageDirection.value}`)
```

Bind `:name="pageTransitionName"` to the existing transition and `:data-direction="pageDirection"` to `.route-page`.

- [x] **Step 4: Replace the generic route transition**

Forward enter starts at `translateX(14px)` and forward leave ends at `translateX(-6px)`; backward mirrors those values. Enter duration is 180 ms, leave duration is 100 ms. Keep `mode="out-in"` so two stateful pages are never live simultaneously.

- [x] **Step 5: Add reduced-motion coverage**

Inside `@media (prefers-reduced-motion: reduce)`, set all forward/backward page transition classes and `.nav-indicator` to `transition: none`; set all enter/leave transformed states to `transform: none` and keep the final page visible.

- [x] **Step 6: Run focused tests**

Run:

```powershell
corepack pnpm exec vitest run src/app/AppShell.test.ts src/app/App.test.ts
```

Expected: PASS.

---

### Task 3: Quality gate and local baseline

**Files:**
- Verify all files above.

**Interfaces:**
- Consumes: the complete motion increment.
- Produces: a clean local commit.

- [x] **Step 1: Run repository checks**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
git diff --check
```

Expected: all commands exit 0 and the initial application chunk remains below 300 KB gzip.

- [x] **Step 2: Commit**

```powershell
git add src/app/App.vue src/app/App.test.ts src/app/AppShell.vue src/app/AppShell.test.ts docs/superpowers/plans/2026-07-23-directional-navigation-motion.md
git commit -m "feat: add directional navigation motion"
```

---

## Self-Review

- Spec coverage: desktop/mobile indicator, forward/backward direction, short route durations, reduced motion, accessibility semantics, and rapid navigation each have explicit implementation or verification.
- Placeholder scan: no `TBD`, `TODO`, unspecified behavior, or deferred test remains.
- Type consistency: all page IDs use the existing `AppPage` union and the same six-item navigation order.
