# Shared Menu Button Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every current “more actions” control one reusable commercial menu-button keyboard, focus, dismissal, and semantic contract.

**Architecture:** Extract the already-proven capture menu mechanics into `useMenuButton`, a DOM-ID-driven composable with no business knowledge. `CaptureWorkspace` and `ProblemDetailDrawer` keep ownership of their business actions and markup, while the composable owns active-menu state, launcher tracking, focus movement, keyboard handling, outside-pointer dismissal, and lifecycle cleanup.

**Tech Stack:** Vue 3 Composition API, TypeScript, Testing Library, user-event, Vitest.

## Global Constraints

- Keep menu state local to each consumer; do not introduce a global store or dependency.
- Resolve the active menu from the launcher's `aria-controls` DOM ID; do not depend on consumer CSS classes.
- Click, ArrowDown, and ArrowUp open the menu and focus an enabled item; ArrowUp opens on the last item.
- Arrow keys cycle enabled menu items; Home/End focus first/last.
- Escape closes only the menu and restores launcher focus; it must not bubble and close the containing drawer.
- Tab closes without preventing native forward or backward movement.
- Outside pointer dismissal must preserve the target click and must not add modal inert or scroll-lock behavior.
- Menu items use `role="menuitem"` and `tabindex="-1"`; triggers expose `aria-haspopup="menu"`, `aria-controls`, and truthful `aria-expanded`.
- Close the detail menu when the problem identity changes; preserve all status emissions, navigation guards, save flows, confirmation copy, and modal drawer behavior.
- Do not change storage migration, updater recovery, licensing, privacy, support, account deletion, device migration, SLA, or `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve unrelated dirty-worktree changes; do not add dependencies, stage, commit, or reformat unrelated lines.

---

### Task 1: Prove the reusable menu contract at both consumers

**Files:**
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Verify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: the visible “更多题目操作” button and its status actions.
- Produces: integration coverage for menu semantics, open focus, Arrow/Home/End navigation, Escape isolation, Tab dismissal, outside dismissal, and context-change dismissal.

- [x] **Step 1: Add a failing library detail menu test**

Add a test that renders an active problem, asserts the launcher has `aria-haspopup="menu"`, `aria-controls="problem-detail-actions-menu"`, and `aria-expanded="false"`; opens with ArrowDown; verifies `role="menuitem"`, focus, ArrowDown/ArrowUp/Home/End cycling; verifies Escape closes the menu, restores the launcher, and does not emit drawer `close`; verifies ArrowUp opens on the last item; verifies Tab closes; verifies click open focuses the first item; verifies an outside pointer closes without activating a status action; and verifies rerendering a different problem closes the menu.

- [x] **Step 2: Run the focused library test and verify red**

Run: `npm test -- src/modules/library/components/ProblemDetailDrawer.test.ts`

Expected: the new test fails because the current control has no menu role, `aria-haspopup`, `aria-controls`, focus movement, menu Escape boundary, or outside/context dismissal.

### Task 2: Extract the shared menu-button state machine

**Files:**
- Create: `src/app/composables/useMenuButton.ts`

**Interfaces:**
- Produces: `useMenuButton(): { activeMenuKey, closeMenu, getMenuLauncher, toggleMenu, handleMenuButtonKeydown, handleMenuKeydown }`.
- `toggleMenu(event: MouseEvent, key: string): Promise<void>` opens or closes from a button launcher.
- `handleMenuButtonKeydown(event: KeyboardEvent, key: string): Promise<void>` handles ArrowDown, ArrowUp, and Escape.
- `handleMenuKeydown(event: KeyboardEvent): void` handles arrows, Home, End, Escape, and Tab.
- `closeMenu(options?: { restoreFocus?: boolean }): void` closes and optionally restores a connected launcher.
- `getMenuLauncher(): HTMLButtonElement | null` exposes the connected focus anchor needed by confirmation consumers.

- [x] **Step 1: Implement DOM-ID-driven open and close**

Create `useMenuButton.ts` with a `ref<string>('')` active key and one non-reactive launcher. On open, store the launcher and key, await `nextTick`, read `launcher.getAttribute('aria-controls')`, resolve the menu with `document.getElementById`, query enabled `[role="menuitem"]` elements, and focus the requested edge. On close, clear state and optionally focus the saved launcher on the next tick only when `isConnected` is true.

- [x] **Step 2: Implement the keyboard policy**

Handle ArrowDown/ArrowUp on launchers. In the menu handler, prevent defaults for Escape and handled navigation keys, cycle ArrowDown/ArrowRight and ArrowUp/ArrowLeft, support Home/End, and close on Tab without preventing default. Return without side effects for unrelated keys or missing items.

- [x] **Step 3: Implement outside dismissal and cleanup**

Register one document `pointerdown` listener in `onMounted`; keep the menu open only when the target is inside either the active launcher or the `aria-controls` menu; remove the listener and close without focus restoration in `onBeforeUnmount`.

### Task 3: Migrate both menu consumers

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`

**Interfaces:**
- Consumes: `useMenuButton` from Task 2.
- Preserves: capture `requestDiscard`, library `requestStatus`, drawer `handleKeydown`, and existing component events.

- [x] **Step 1: Replace the capture-local implementation**

Remove the capture-only launcher variable, open/close/toggle/keydown/outside-pointer functions, and their document lifecycle registration. Destructure the shared composable using aliases that preserve the existing template names. Keep `requestDiscard` behavior by reading `getMenuLauncher()`, closing, focusing the connected launcher before `askDiscardConfirmation`, and retaining the existing batch-exists guard.

- [x] **Step 2: Wire the library detail menu**

Destructure the shared composable in `ProblemDetailDrawer`. Add `aria-haspopup="menu"`, `aria-controls="problem-detail-actions-menu"`, truthful expanded state, click and launcher-keydown handlers. Give the action container its ID, `role="menu"`, label, and a stopped menu-keydown handler. Give each action `role="menuitem"` and `tabindex="-1"`. Call `closeMenu()` before emitting a successful status change.

- [x] **Step 3: Isolate drawer Escape and reset context**

Ensure menu keydown propagation is stopped so Escape closes only the menu. Watch `props.detail?.id` and close the menu when the active problem changes, without restoring focus to a launcher belonging to the previous context.

- [x] **Step 4: Run both focused integration suites**

Run: `npm test -- src/modules/library/components/ProblemDetailDrawer.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: both files pass; capture remains at 32 tests and library includes the new menu contract test.

### Task 4: Verify the commercial regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-shared-menu-button-boundary.md`

**Interfaces:**
- Produces: exact verification record and completed checklist.

- [x] **Step 1: Run static and production validation**

Run `npm run lint`, `npm run typecheck`, and `npm run build`.

- [x] **Step 2: Run the full frontend suite**

Run: `npm test`

- [x] **Step 3: Perform local code review and record completion**

Review the composable for CSS independence, missing/disconnected launchers, stale DOM IDs, outside-pointer containment, listener cleanup, native Tab behavior, Escape propagation, multiple consumers, capture confirmation integration, problem context changes, dirty-worktree scope, excluded features, whitespace, and staged state. Resolve every Critical or Important finding, check every plan item, and append exact verification counts.

## Verification Record

- Red test: `ProblemDetailDrawer.test.ts` reported the expected missing `aria-haspopup="menu"` failure, with 11 existing tests passing.
- Focused regression: `ProblemDetailDrawer.test.ts` and `CaptureWorkspace.test.ts` passed 44/44 tests across 2 files.
- Static checks: `npm run lint` and `npm run typecheck` passed on the final implementation.
- Production build: `npm run build` passed with 2,055 modules transformed.
- Full frontend regression: the first run executed concurrently with build and hit one unrelated `App.test.ts` navigation timeout; the isolated file then passed 7/7 and the final standalone full run passed 147/147 files and 801/801 tests.
- Local code review: no Critical or Important findings. The final selector excludes both native-disabled and `aria-disabled="true"` menu items; forward and backward native Tab flow, Escape isolation, CSS-independent DOM lookup, outside containment, lifecycle cleanup, both consumers, context reset, capture confirmation focus, whitespace, dirty-worktree scope, excluded launch-only work, and empty staged state were verified.
