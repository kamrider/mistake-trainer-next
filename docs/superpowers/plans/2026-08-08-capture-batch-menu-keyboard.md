# Capture Batch Menu Keyboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the capture batch secondary-action menu follow the complete commercial menu-button keyboard and focus model.

**Architecture:** Keep menu ownership local to `CaptureWorkspace.vue` and preserve the existing `batchMenuId` state plus `ActionConfirmDialog` deletion boundary. Add explicit open/close/focus helpers around the existing trigger/menu DOM, use one document pointer listener for outside dismissal, and hand focus back to the launcher before opening the destructive confirmation so the reusable dialog can restore it reliably.

**Tech Stack:** Vue 3 single-file components, TypeScript, Testing Library, Vitest.

## Global Constraints

- The batch action trigger must expose `aria-haspopup="menu"`, `aria-controls`, and truthful `aria-expanded`.
- Click, ArrowDown, and ArrowUp must open the menu and focus a menu item; ArrowUp opens on the last item.
- Arrow keys must cycle enabled menu items; Home/End must focus first/last.
- Escape must close and restore trigger focus; Tab must close without preventing native focus movement; an outside pointer must close the menu.
- Menu items must not create extra Tab stops; focus enters programmatically from the menu button.
- Before opening the destructive confirmation, focus must return to the menu launcher so cancellation restores a connected element.
- Do not change deletion copy, `useActionConfirmation`, discard emission, batch identity, collection/organization behavior, recognition flows, or capture persistence.
- Do not change storage migration, updater recovery, launch-only work, or `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve unrelated dirty-worktree changes and do not add dependencies or stage/commit files.

---

### Task 1: Lock the batch menu button contract

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue:1-285, 547-584`

**Interfaces:**
- Consumes: existing `batchMenuId`, batch IDs, and `requestDiscard(batch)`.
- Produces: `openBatchMenu`, `closeBatchMenu`, `toggleBatchMenu`, `handleBatchMenuButtonKeydown`, `handleBatchMenuKeydown`, and one document pointer handler.

- [x] **Step 1: Add failing menu semantics and keyboard tests**

Assert `aria-haspopup`, ArrowDown open/focus, Escape close/trigger return, ArrowUp open, Tab close, click open/focus, and outside-pointer dismissal. Extend the existing discard confirmation test to verify cancellation returns focus to the connected launcher.

- [x] **Step 2: Run the capture workspace test and verify red**

Run: `npm test -- src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: the new assertions fail because the current click toggles only visibility and leaves focus on the trigger; there is no keyboard or outside-dismissal policy.

- [x] **Step 3: Implement the local menu boundary**

Track the active launcher outside reactive state, open with `nextTick` focus, close with optional focus restoration, query only enabled menu items in the current menu, implement arrows/Home/End/Escape/Tab, and register/remove one document `pointerdown` listener in mount lifecycle hooks.

- [x] **Step 4: Wire semantic attributes and handlers**

Add menu IDs, `aria-haspopup`, `aria-controls`, click/keydown handlers, menu keydown handling, and `tabindex="-1"` on menu items. Preserve click propagation isolation and the existing confirmation copy.

- [x] **Step 5: Preserve deletion confirmation focus return**

In `requestDiscard`, save the active launcher, close the menu, focus the launcher, then call `askDiscardConfirmation`. Do not emit deletion until the existing confirmation resolves true and the batch still exists.

- [x] **Step 6: Run the focused capture suite**

Run: `npm test -- src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all capture workspace tests pass.

### Task 2: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-capture-batch-menu-keyboard.md`

- [x] **Step 1: Run static and production validation**

Run `npm run lint`, `npm run typecheck`, and `npm run build`.

- [x] **Step 2: Run the full frontend suite**

Run: `npm test`

- [x] **Step 3: Perform local code review and record completion**

Review lifecycle listener cleanup, connected-focus restoration, handled-key default prevention, native Tab behavior, multiple-batch switching, confirmation integration, existing dirty-worktree scope, excluded features, whitespace, and staged state. Resolve every Critical or Important finding, check every plan item, and append exact verification counts.

## Verification Record

- Red test: `CaptureWorkspace.test.ts` reported 2 expected failures and 30 passes before implementation.
- Focused regression: `CaptureWorkspace.test.ts` passed 32/32 tests after implementation.
- Static checks: `npm run lint` and `npm run typecheck` passed.
- Production build: `npm run build` passed with 2,054 modules transformed.
- Full frontend regression: 147 test files and 800 tests passed.
- Local code review: no Critical or Important findings. Listener cleanup, focus restoration, handled-key prevention, native Tab flow, batch switching, confirmation integration, whitespace, dirty-worktree scope, excluded launch-only work, and empty staged state were verified.
