# Route Context Focus Appearance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep SPA route-context focus available to assistive technology without drawing an input-like outline around non-interactive page headings or the route region.

**Architecture:** Preserve the existing `App.vue` route focus lifecycle and the global `:focus-visible` rule for interactive controls. Add a narrowly scoped visual exception only for the two programmatic route-context targets: `.route-page` and its `h1[tabindex="-1"]`, then lock both sides of the contract with a source-level readability test and real Chromium checks.

**Tech Stack:** Vue 3 scoped CSS, Vitest, Node `fs`, Vite local preview, Chromium browser verification

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support, account deletion, device migration/recovery, updater recovery, or SLA work.
- Preserve all existing working-tree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve the existing route heading/region focus transfer, `tabindex="-1"`, deliberate-focus protection, Suspense handling, route errors, retry behavior, and route animation.
- Keep the global 3px `:focus-visible` outline for buttons, links, inputs, selects, textareas, and every other interactive control.
- Scope the quiet focus appearance to programmatic route-context targets only.

---

### Task 1: Quiet programmatic route context without weakening keyboard focus

**Files:**
- Modify: `src/app/AppChromeReadability.test.ts`
- Modify: `src/app/App.vue`

**Interfaces:**
- Consumes: `App.vue`'s `.route-page` wrapper, dynamically assigned heading `tabindex="-1"`, and `src/shared/styles/tokens.css` global `:focus-visible` declaration.
- Produces: `.route-page:focus,.route-page h1[tabindex="-1"]:focus { outline: none; }` while leaving the global interactive focus rule unchanged.

- [x] **Step 1: Write the failing visual-focus contract**

Add the token file constant and this test to `src/app/AppChromeReadability.test.ts`:

```ts
const tokenPath = 'src/shared/styles/tokens.css'

it('keeps programmatic route context quiet without weakening interactive focus', () => {
  expect(declarations('src/app/App.vue', '.route-page:focus,.route-page h1[tabindex="-1"]:focus'))
    .toContain('outline:none')
  expect(source(tokenPath)).toContain(':focus-visible { outline: 3px solid')
})
```

- [x] **Step 2: Run the focused contract and verify the red state**

Run: `pnpm exec vitest run src/app/AppChromeReadability.test.ts`

Expected: FAIL because `App.vue` does not yet define the route-context focus exception.

- [x] **Step 3: Add the minimal scoped route-context style**

Add this declaration next to the route loading/error styles in `src/app/App.vue`:

```css
.route-page:focus,.route-page h1[tabindex="-1"]:focus { outline: none; }
```

Do not change `resolveRouteFocus()`, the heading `tabindex`, or `tokens.css`.

- [x] **Step 4: Run the focused behavior and visual contracts**

Run: `pnpm exec vitest run src/app/AppChromeReadability.test.ts src/app/App.test.ts`

Expected: both files PASS; route headings still receive focus, deliberate persistent-control focus is preserved, and the scoped appearance contract passes.

- [x] **Step 5: Verify pointer, keyboard, desktop, and narrow-screen behavior in Chromium**

At the default desktop viewport, click Settings with a pointer and verify:

- the `设置` heading is `document.activeElement`;
- the heading keeps `tabindex="-1"`;
- the heading computed `outline-style` is `none`;
- page geometry and scrolling do not change.

Focus a side-navigation button through a keyboard interaction and verify it still matches `:focus-visible` with a solid 3px outline. Repeat the pointer route change at `390x844` and verify there is no route-heading outline or horizontal overflow. Reset the temporary viewport afterwards.

- [x] **Step 6: Run the complete frontend quality gate**

Run: `pnpm test`

Expected: all frontend tests PASS.

Run: `pnpm lint`

Expected: PASS with zero warnings.

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm build`

Expected: the production build PASS.

- [x] **Step 7: Review isolation and whitespace**

Run: `git diff --check`

Expected: PASS with no whitespace errors.

Inspect the exact additions in `App.vue`, `AppChromeReadability.test.ts`, and this plan. Both tracked source files contain pre-existing uncommitted work, so attribute only the new route-context focus selector and its contract to this task.
