# Capture Layout Input Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The active workspace policy forbids sub-agents unless the user explicitly requests them, so review is local in this session.

**Goal:** Prevent invalid layout-template values from reaching Tauri and give immediate, accessible, mode-specific feedback beside the controls.

**Architecture:** Validation remains inside `CaptureLayoutTemplatePanel.vue`, the component that owns the layout draft values and apply interaction. Computed normalized integers form the only values allowed into the existing `apply` event; the workspace, organizer controller, Tauri bindings, and Rust transaction contracts remain unchanged.

**Tech Stack:** Vue 3 computed state, TypeScript, native number inputs, ARIA, Testing Library, Vitest, scoped CSS.

## Global Constraints

- Alternating mode accepts integer question and answer counts from 1 through 10, matching the Rust transaction boundary.
- Split mode accepts an integer split position from 1 through `itemCount`; zero must not silently regenerate no cards.
- Questions-only and manual modes must not be blocked by stale values from hidden alternating or split inputs.
- Invalid input must not open the destructive confirmation or emit `apply`.
- Preserve all valid payloads, confirmation behavior, focus/document boundary behavior, and public events.
- Keep the message at least 12 px, use `role="alert"`, and associate it with the invalid input using `aria-invalid` and `aria-describedby`.
- Do not change APIs, Rust, dependencies, launch-only items, backup/storage migration, or recognition algorithms.
- Do not stage or commit; preserve all unrelated dirty-worktree changes.

---

### Task 1: Lock invalid-value behavior

**Files:**
- Modify: `src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts`

**Interfaces:**
- Consumes: existing `itemCount`, `draftCount`, `affectedNoteCount`, and `busy` props.
- Verifies: existing `apply(mode, questions, answers, splitIndex)` event is absent while active-mode input is invalid.

- [x] **Step 1: Add an alternating-mode validation regression**

Clear “题图/题”, then assert an alert says `题图/题和答案/题必须是 1–10 的整数。`, both invalid state and apply-button disablement are exposed, no confirmation appears, and no event emits.

- [x] **Step 2: Add a split-mode validation regression**

Select split mode; enter `0`, `1.5`, and a value above `itemCount`; for each assert `分开位置必须是 1–{itemCount} 的整数。`, `aria-invalid="true"`, a disabled apply button, and no event.

- [x] **Step 3: Add hidden-field isolation coverage**

Make alternating values invalid, switch to manual, and assert the alert clears and the valid manual payload emits with safe numeric defaults and `null` split index.

- [x] **Step 4: Run the targeted test and confirm red state**

Run: `npm test -- src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts`

Expected: FAIL because invalid values currently keep the apply action enabled and have no local alert.

---

### Task 2: Add normalized mode-specific validation

**Files:**
- Modify: `src/modules/capture/components/CaptureLayoutTemplatePanel.vue`

**Interfaces:**
- Produces: `normalizedQuestionImages`, `normalizedAnswerImages`, `normalizedSplitIndex`, `layoutValidationMessage`, and `canApply` computed state.
- Preserves: `emit('apply', CaptureLayoutMode, number, number, number | null)`.

- [x] **Step 1: Represent editable number fields honestly**

Use `ref<number | ''>` for the three editable values and a local helper that returns a number only when `typeof value === 'number' && Number.isInteger(value)`.

- [x] **Step 2: Compute active-mode validity**

For alternating mode require both normalized values within 1–10. For split mode require the normalized split within 1–`itemCount`. Return an empty message for questions-only and manual.

- [x] **Step 3: Guard both request paths and normalize emitted values**

Define `canApply` as `itemCount > 0 && !busy && !layoutValidationMessage`; make both `requestApply` and `applyRequestedLayout` return when false. Emit validated active values and safe `1` defaults for irrelevant hidden fields.

- [x] **Step 4: Render accessible local feedback**

Bind relevant inputs to `aria-invalid` and `aria-describedby="layout-validation-message"`; render the message with that id and `role="alert"`; disable the apply launcher when `!canApply`.

- [x] **Step 5: Add compact validation styling**

Give `.layout-validation` a full-row flex basis, 12 px readable type, and destructive/error color without changing the panel’s responsive layout.

- [x] **Step 6: Run component tests**

Run: `npm test -- src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts`

Expected: all component tests pass.

---

### Task 3: Validate integration and quality gates

**Files:**
- Verify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Verify: `src/modules/capture/components/CaptureWorkspaceReadability.test.ts`
- Modify: `docs/superpowers/plans/2026-08-03-capture-layout-input-validation.md`

**Interfaces:**
- Preserves: workspace `applyLayout` forwarding and readability contract.

- [x] **Step 1: Run affected regressions**

Run: `npm test -- src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/modules/capture/components/CaptureWorkspaceReadability.test.ts`

Expected: all tests pass.

- [x] **Step 2: Run static gates**

Run: `npm run lint`

Run: `npm run typecheck`

Expected: both pass with no warnings or errors.

- [x] **Step 3: Run production and full regression gates**

Run: `npm run build`

Run: `npm test`

Expected: production build and complete Vitest suite pass.

- [x] **Step 4: Review and record evidence**

Check active-mode isolation, emitted numeric types, confirmation guards, ARIA associations, scoped CSS, excluded-file status, and staged index. Append a `Verification Record` section containing the exact command results before completing the plan.

## Verification Record

- Red phase: the targeted component file ran 8 tests with exactly the 3 new validation tests failing because no local alert, ARIA invalid state, or disabled launcher existed.
- Component green phase: 1 file, 8 tests passed.
- Affected regressions: 3 files, 42 tests passed.
- ESLint: passed with zero warnings.
- Vue/TypeScript typecheck: passed; editable `number | ''` values do not cross the numeric emit boundary.
- Production build: passed; Vite transformed 2054 modules.
- Complete Vitest: 137 files, 767 tests passed.
- Local review: no Critical or Important findings. Both the launcher and modal confirmation use `canApply`; manual/questions-only modes ignore hidden invalid values; split zero, fractions, and values above `itemCount` are blocked; alternating blank, zero, fractions, and values above 10 are blocked; 10 remains accepted.
- Diff check: passed. Staged index remains empty; `dist/` remains ignored. Existing `recognition_visual_split.rs` modification was not touched.
