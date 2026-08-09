# Capture Selection Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make capture material selection, batch-subject choice, and material-role choice expose the same state and availability to mouse, keyboard, and assistive-technology users.

**Architecture:** Separate the thumbnail card container from its primary selection control by adding one native full-card button layer, leaving crop and removal as sibling buttons. Keep all business state in existing props and events; expose current choices with `aria-pressed` rather than introducing a new selection store or composite widget.

**Tech Stack:** Vue 3 single-file components, TypeScript, Testing Library, user-event, Vitest, scoped CSS.

## Global Constraints

- A thumbnail containing crop/remove actions must not use `role="button"` on the ancestor because that would create nested interactive semantics.
- The thumbnail selection target must be a native button with a stable filename accessible name, truthful `aria-pressed`, and native `disabled` behavior.
- Enter and Space must activate selection through native button behavior; disabled selection must emit neither `activate` nor `pointerStart`.
- Crop and remove buttons must stay separately focusable and must not activate selection or initiate dragging.
- The full visual card remains the pointer target for selection and drag through the selection button layer; crop/remove controls remain above that layer.
- Batch-subject and material-role buttons must expose their visual selected state with truthful `aria-pressed`.
- Preserve preview lazy loading, drag payloads, crop/revert/remove events, subject confirmation, role persistence, card creation, and all existing copy.
- Do not add dependencies or change storage migration, updater recovery, licensing, privacy, support, account deletion, device migration, SLA, or `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve unrelated dirty-worktree changes; do not stage, commit, or reformat unrelated lines.

---

### Task 1: Prove the thumbnail interaction boundary

**Files:**
- Modify: `src/modules/capture/components/CaptureThumbnail.test.ts`

**Interfaces:**
- Consumes: existing `active`, `disabled`, `cropable`, and `removable` props.
- Verifies: existing `activate`, `pointerStart`, `crop`, and `remove` events.

- [x] **Step 1: Add failing native-selection tests**

Add a test that finds a button named by `item.sourceName`, asserts `aria-pressed="false"`, clicks it, presses Enter and Space, and verifies three `activate` emissions. Rerender with `active: true` and verify `aria-pressed="true"`; rerender with `disabled: true`, verify native disabled state, then confirm click and pointerdown add no selection or drag emissions. Assert the ancestor article has no `tabindex` or button role.

- [x] **Step 2: Add failing sibling-action isolation test**

Render with `cropable` and `removable`, click the crop and remove buttons, and verify their own events fire while `activate` and `pointerStart` remain absent.

- [x] **Step 3: Run the thumbnail test and verify red**

Run: `npm test -- src/modules/capture/components/CaptureThumbnail.test.ts`

Expected: the tests fail because the current focusable article has no native button role or pressed state and still emits activation while disabled.

### Task 2: Build the native full-card selection control

**Files:**
- Modify: `src/modules/capture/components/CaptureThumbnail.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.test.ts`

**Interfaces:**
- Produces: a `.thumbnail-activate` native button whose accessible name is `item.sourceName`.
- Produces: a `selectionKeydown` event carrying the native selection-button `KeyboardEvent`.
- Preserves: all existing component props and business event signatures.

- [x] **Step 1: Separate container and selection semantics**

Remove `aria-label`, `tabindex`, pointer, click, and key handlers from the root article. Add a first-child button with `type="button"`, class `thumbnail-activate`, `:aria-label="item.sourceName"`, `:aria-pressed="Boolean(active)"`, `:disabled="disabled"`, pointerdown emission, and click activation. Rely on native Enter/Space behavior instead of manual key handlers.

- [x] **Step 2: Preserve the full-card pointer surface**

Position `.thumbnail-activate` absolutely at `inset: 0` with `z-index: 1`, transparent background, inherited radius, grab cursor, and `touch-action: none`. Give crop/remove buttons a positioned `z-index: 2`; use parent hover/focus-within styling and a visible selection-button focus ring. Keep disabled cursor and opacity behavior.

- [x] **Step 3: Avoid duplicate assistive descriptions**

Mark purely visual thumbnail media and copy as `aria-hidden="true"` while retaining the stable filename on the selection button and explicit labels on crop/remove buttons.

- [x] **Step 4: Preserve filmstrip keyboard routing**

Emit `selectionKeydown` only from the native selection button and consume it in `CaptureDraftCard`. After direction-key image selection, focus the new item's `.thumbnail-activate`; let Ctrl/Command/Alt-modified keys bypass thumbnail shortcuts so the containing card can handle navigation. Update drag and keyboard tests to target the real selection button rather than the non-interactive article.

- [x] **Step 5: Run the thumbnail and card suites**

Run: `npm test -- src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureDraftCard.test.ts`

Expected: preview lifecycle, interaction-boundary, drag, image navigation, reorder, role shortcut, and card-navigation tests pass.

### Task 3: Expose capture choice state

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: existing `pendingBatchSubject` and `selectedMaterial.stagedRole` values.
- Produces: truthful `aria-pressed` attributes without changing events or persistence.

- [x] **Step 1: Add failing choice-state assertions**

Extend the whole-batch subject test to assert the current subject is pressed, another subject is unpressed, and clicking the other subject updates both states before confirmation. Extend the material role test to assert “设为题面” is pressed and “设为答案” is unpressed for a selected question material.

- [x] **Step 2: Run the capture workspace test and verify red**

Run: `npm test -- src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: the new assertions fail because selected styles are not reflected through `aria-pressed`.

- [x] **Step 3: Bind the existing visual states to ARIA**

Add `:aria-pressed="pendingBatchSubject === subject"` to subject buttons and the matching `:aria-pressed` bindings to the question and answer role buttons. Do not change disabled conditions or click handlers.

- [x] **Step 4: Run all focused suites**

Run: `npm test -- src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/modules/capture/components/CaptureDraftCard.test.ts`

Expected: all three files pass with existing behavior and keyboard continuity preserved.

### Task 4: Verify the commercial regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-capture-selection-semantics.md`

**Interfaces:**
- Produces: completed checklist and exact verification record.

- [x] **Step 1: Run static and production validation**

Run `npm run lint`, `npm run typecheck`, and `npm run build`.

- [x] **Step 2: Run the full frontend suite without parallel build load**

Run: `npm test`

- [x] **Step 3: Perform local code review and record completion**

Review native semantics, nested-control separation, disabled emissions, pointer stacking, keyboard activation, focus visibility, accessible-name stability, duplicate descriptions, selected-state truthfulness, existing drag/crop/remove behavior, dirty-worktree scope, excluded features, whitespace, and staged state. Resolve every Critical or Important finding, check every plan item, and append exact verification counts.

## Verification Record

- Red test: the first two-file run reported 4 expected failures and 32 passes, proving the missing native button, missing pressed states, disabled activation, and remove-pointer drag leak.
- Focused regression: `CaptureThumbnail.test.ts`, `CaptureWorkspace.test.ts`, and `CaptureDraftCard.test.ts` passed 44/44 tests after migrating drag sources and keyboard focus to `.thumbnail-activate`.
- Static checks: `npm run lint` and `npm run typecheck` passed on the final implementation.
- Production build: `npm run build` passed with 2,055 modules transformed.
- Full frontend regression: an intermediate run exposed two direct filmstrip-keyboard regressions plus the known `App.test.ts` one-second navigation timeout. The keyboard regressions were fixed; `App.test.ts` passed 7/7 in isolation; the final standalone full run passed 147/147 files and 803/803 tests.
- Local code review: no Critical findings. One Important modifier-key conflict was fixed so Ctrl/Command/Alt bypass image shortcuts and card navigation remains exclusive. Native selection semantics, nested-control separation, explicit disabled pointer protection, pointer stacking, Enter/Space activation, focus migration, stable names, hidden duplicate visuals, pressed-state truthfulness, drag/crop/remove isolation, whitespace, dirty-worktree scope, excluded launch-only work, and empty staged state were verified.
