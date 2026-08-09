# Route Transition Focus Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move keyboard and screen-reader context to the entered page heading after SPA navigation or route-error replacement, while preserving focus the user deliberately moves during the transition.

**Architecture:** Own the policy in `App.vue`, where the complete route render key, error boundary, Suspense lifecycle, and page Transition meet. Queue a request keyed by the exact route-page instance, reject the outgoing wrapper, focus the entered page's first `h1` (or an explicitly labelled region fallback), and cancel when focus changed to a persistent control.

**Tech Stack:** Vue 3 Composition API, Vue Router, TypeScript, Testing Library Vue, Vitest.

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support operations, account deletion, device migration, updater recovery, or SLA work.
- Preserve all unrelated dirty-worktree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve route direction animation, error retry behavior, Suspense loading feedback, profile-epoch remounts, and workspace navigation guards.
- Do not steal focus if the user deliberately moves it to a persistent control while a page is entering.
- Do not add a nested `main` landmark around route views that already render their own `main` elements.

---

### Task 1: Route-page context and transition-aware focus

**Files:**
- Modify: `src/app/App.test.ts`
- Modify: `src/app/App.vue`

**Interfaces:**
- Consumes: route `fullPath`, `profileEpoch`, `routeRenderEpoch`, route error state, RouterView, Suspense, and the existing keyed `mode="out-in"` Transition.
- Produces: computed `routePageKey`; pending route focus request; `requestRouteFocus()` and `resolveRouteFocus()` lifecycle functions; labelled route region fallback; real-Transition navigation coverage.

- [x] **Step 1: Write failing navigation and error-context tests**

Add a real-Transition test that navigates from the dashboard to the library and proves the entered `题库` heading receives focus. Then start navigating to settings, deliberately focus a persistent navigation button before the settings page enters, and prove the new heading does not steal focus. Extend the recoverable route-error test to prove its error heading receives focus, and extend the transient-error retry test to prove the recovered heading receives focus after the render epoch remount.

```ts
it('focuses entered page context without stealing deliberate focus during a real transition', async () => {
  const user = userEvent.setup()
  const router = createAppRouter(createMemoryHistory())
  await router.push('/')
  await router.isReady()
  render(App, {
    global: { plugins: [router], stubs: { transition: false } },
  })

  await user.click(screen.getByRole('button', { name: '题库' }))
  await waitFor(() => expect(screen.getByRole('heading', { name: '题库' })).toHaveFocus())

  await user.click(screen.getByRole('button', { name: '设置' }))
  await waitFor(() => expect(router.currentRoute.value.name).toBe('settings'))
  const dashboardNavigation = screen.getByRole('button', { name: '训练台' })
  dashboardNavigation.focus()
  await screen.findByRole('heading', { name: '设置' })
  expect(dashboardNavigation).toHaveFocus()
})
```

In the existing route-error test, add:

```ts
const routeErrorHeading = await screen.findByRole('heading', { name: '这个页面暂时打不开' })
await waitFor(() => expect(routeErrorHeading).toHaveFocus())
```

- [x] **Step 2: Run focused App tests and verify red state**

Run: `pnpm vitest run src/app/App.test.ts`

Expected: the new focus assertions fail because the route wrapper has no transition-aware focus policy and route errors do not request context focus.

- [x] **Step 3: Implement keyed route focus lifecycle**

Import `nextTick`, compute the exact route-page key, and add a pending request that only resolves against the matching entered wrapper.

```ts
const routePage = ref<HTMLElement>()
const routePageKey = computed(() => `${route.fullPath}:${profileEpoch.value}:${routeRenderEpoch.value}`)
type RouteFocusRequest = { routePageKey: string; previousActive: Element | null }
const pendingRouteFocus = ref<RouteFocusRequest>()

function requestRouteFocus() {
  pendingRouteFocus.value = {
    routePageKey: routePageKey.value,
    previousActive: document.activeElement,
  }
  void nextTick(resolveRouteFocus)
}

function resolveRouteFocus({ allowPageFallback = false } = {}) {
  const request = pendingRouteFocus.value
  const page = routePage.value
  if (!request || !page || page.dataset.routePageKey !== request.routePageKey) return

  const active = document.activeElement
  if (
    active !== request.previousActive
    && active !== document.body
    && active !== document.documentElement
  ) {
    pendingRouteFocus.value = undefined
    return
  }

  const heading = page.querySelector<HTMLElement>('h1')
  if (!heading && !allowPageFallback) return
  const target = heading ?? page
  if (heading && !heading.hasAttribute('tabindex')) heading.setAttribute('tabindex', '-1')
  target.focus({ preventScroll: true })
  pendingRouteFocus.value = undefined
}
```

Watch `routePageKey` to request focus for navigation, profile remount, and retry remount. Watch truthy `routeError` to request focus when the error boundary replaces route content without changing the outer key.

Bind the Transition's `after-enter` to a no-argument `handleRoutePageEntered()` wrapper; bind the route wrapper `ref`, computed key, `data-route-page-key`, `role="region"`, `aria-label="页面内容"`, and `tabindex="-1"`; bind Suspense `resolve` to a no-argument `handleRouteContentResolved()` wrapper that enables the labelled region fallback. The wrappers prevent Vue lifecycle event arguments from being interpreted as focus options.

- [x] **Step 4: Run focused and static verification**

Run: `pnpm vitest run src/app/App.test.ts`

Expected: all App tests pass, including real page entry, deliberate-focus protection, and route-error heading focus.

Run: `pnpm lint`

Expected: ESLint exits with code 0 and no warnings.

Run: `pnpm typecheck`

Expected: Vue TypeScript checking exits with code 0.

- [x] **Step 5: Run production and full regression verification**

Run: `pnpm build`

Expected: mobile asset verification, Vue type checking, and Vite production build pass.

Run: `pnpm vitest run`

Expected: the complete frontend suite passes without regressions.

- [x] **Step 6: Review final diff and isolation**

Run: `git diff --check -- src/app/App.vue src/app/App.test.ts`

Expected: no whitespace errors.

Run: `git diff --cached --name-only`

Expected: no staged files.

Inspect the target diff and confirm only route context focus, its tests, and completed plan bookkeeping were added; preserve every unrelated working-tree change.

## Verification Record

- Focused red state: App tests failed because the entered library heading remained unfocused and the route-error heading left focus on `body`.
- Focused final state: `App.test.ts` passed 8/8, covering real navigation, deliberate-focus protection, route-error focus, and recovered-route focus.
- Static checks: `pnpm lint` and `pnpm typecheck` passed.
- Production build: `pnpm build` passed with 2055 modules transformed.
- Full frontend regression: 147 test files and 806 tests passed.
- Review result: no unresolved Critical or Important findings; exact render keys reject stale page/Suspense callbacks and latest navigation overwrites earlier intent.
