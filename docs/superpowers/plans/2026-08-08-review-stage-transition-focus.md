# Review Stage Transition Focus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep keyboard and screen-reader context on the newly mounted review heading after real card transitions without stealing focus that the user deliberately moved elsewhere during the animation.

**Architecture:** Keep the policy local to `ReviewRoom.vue`, where the review-stage key and transition lifecycle are owned. Queue a focus request containing the intended stage key and the active element at request time, ignore the outgoing keyed heading, and resolve the request after the new stage enters; retain an immediate `nextTick` attempt for initial render and transition-stub tests.

**Tech Stack:** Vue 3 Composition API, TypeScript, Testing Library Vue, Vitest.

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support operations, account deletion, device migration, updater recovery, or SLA work.
- Preserve all unrelated dirty-worktree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve the existing rule that a deliberate user focus change during a stage transition is not overridden.
- Preserve answer secrecy, rating shortcuts, submission guards, lightbox behavior, and forward/backward transition direction.

---

### Task 1: Transition-aware review heading focus

**Files:**
- Modify: `src/modules/review/components/ReviewRoom.test.ts`
- Modify: `src/modules/review/components/ReviewRoom.vue`

**Interfaces:**
- Consumes: the existing keyed review-stage Transition and `current`/`examPhase` watcher.
- Produces: computed `stageKey`; pending `{ stageKey, previousActive }` focus request; `requestHeadingFocus()` and `resolveHeadingFocus()` lifecycle functions; real-Transition regression coverage.

- [x] **Step 1: Write the failing real-Transition test**

Add a test that disables the default Transition stub, captures the first mounted heading, advances to the next exam question, and proves the replacement heading—not the outgoing node—receives focus. Then advance again, deliberately focus the persistent exit button while the stage changes, and prove the newly mounted heading does not steal focus.

```ts
it('moves context to the entered stage without stealing deliberate focus during a real transition', async () => {
  const user = userEvent.setup()
  const view = render(ReviewRoom, {
    props: { ...baseProps, mode: 'exam', examPhase: 'answering', current: 1, total: 3 },
    global: { stubs: { transition: false } },
  })

  const headingName = '先独立完成整组，再统一看答案'
  const firstHeading = screen.getByRole('heading', { name: headingName })
  await waitFor(() => expect(firstHeading).toHaveFocus())

  await user.click(screen.getByRole('button', { name: '下一题' }))
  await view.rerender({ current: 2 })
  await waitFor(() => {
    const enteredHeading = screen.getByRole('heading', { name: headingName })
    expect(enteredHeading).not.toBe(firstHeading)
    expect(enteredHeading).toHaveFocus()
  })

  const secondHeading = screen.getByRole('heading', { name: headingName })
  await view.rerender({ current: 3 })
  const exit = screen.getByRole('button', { name: '退出训练' })
  exit.focus()
  await waitFor(() => expect(screen.getByRole('heading', { name: headingName })).not.toBe(secondHeading))
  expect(exit).toHaveFocus()
})
```

- [x] **Step 2: Run focused test and verify red state**

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts`

Expected: the new test fails because `focusHeadingIfIdle()` targets the outgoing heading before the `mode="out-in"` replacement mounts.

- [x] **Step 3: Implement keyed pending focus resolution**

Create a computed stage key, store pending focus intent, verify the current heading belongs to the intended stage before focusing, and retry from the Transition's `after-enter` event.

```ts
const stageKey = computed(() => `${props.current}-${props.examPhase || 'review'}`)
type HeadingFocusRequest = { stageKey: string; previousActive: Element | null }
const pendingHeadingFocus = ref<HeadingFocusRequest>()

function resolveHeadingFocus() {
  const request = pendingHeadingFocus.value
  const heading = headingElement.value
  if (!request || !heading) return
  const mountedStageKey = heading.closest<HTMLElement>('[data-review-stage-key]')?.dataset.reviewStageKey
  if (mountedStageKey !== request.stageKey) return

  const active = document.activeElement
  if (
    active !== request.previousActive
    && active !== document.body
    && active !== document.documentElement
  ) {
    pendingHeadingFocus.value = undefined
    return
  }
  heading.focus({ preventScroll: true })
  pendingHeadingFocus.value = undefined
}

function requestHeadingFocus() {
  pendingHeadingFocus.value = {
    stageKey: stageKey.value,
    previousActive: document.activeElement,
  }
  void nextTick(resolveHeadingFocus)
}
```

Replace calls to `focusHeadingIfIdle()` with `requestHeadingFocus()`. Bind `:key="stageKey"` and `:data-review-stage-key="stageKey"` on the review stage, and bind `@after-enter="resolveHeadingFocus"` on its Transition.

- [x] **Step 4: Run focused and static verification**

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts`

Expected: all ReviewRoom tests pass, including the real Transition and deliberate-focus cases.

Run: `pnpm lint`

Expected: ESLint exits with code 0 and no warnings.

Run: `pnpm typecheck`

Expected: Vue TypeScript checking exits with code 0.

- [x] **Step 5: Run production and full regression verification**

Run: `pnpm build`

Expected: mobile asset verification, Vue type checking, and the Vite production build pass.

Run: `pnpm vitest run`

Expected: the complete frontend suite passes without regressions.

- [x] **Step 6: Review final diff and isolation**

Run: `git diff --check -- src/modules/review/components/ReviewRoom.vue src/modules/review/components/ReviewRoom.test.ts`

Expected: no whitespace errors.

Run: `git diff --cached --name-only`

Expected: no staged files.

Inspect the target diff and confirm only keyed transition focus behavior, its tests, and completed plan bookkeeping were added; preserve every unrelated working-tree change.

## Verification Record

- Focused red state: the real Transition test failed because the entered heading was mounted while focus had fallen to `body`.
- Focused final state: `ReviewRoom.test.ts` passed 10/10, including replacement-heading focus and deliberate-focus protection.
- Static checks: `pnpm lint` and `pnpm typecheck` passed.
- Production build: `pnpm build` passed with 2055 modules transformed.
- Full frontend regression: 147 test files and 805 tests passed.
- Review result: no unresolved Critical or Important findings; keyed requests overwrite stale intent and only resolve against the matching entered stage.
