# Export Picker Keyboard and Touch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the export candidate source selector behave like a commercial keyboard-accessible radio group and make its search control touch-safe at every viewport width.

**Architecture:** Keep `ExportCandidatePicker.vue` as a controlled component: arrow-key navigation emits the same `source` intent as a click and never mutates the source prop locally. Add roving tab stops and DOM-local radio navigation to the existing source cards, then enforce the 44 px/12 px interaction baseline with a source-level Vitest contract.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Testing Library, Vitest.

## Global Constraints

- Exactly one enabled source radio must be in the Tab order; when disabled/loading, no source radio may be in the Tab order.
- ArrowRight/ArrowDown move to the next source, ArrowLeft/ArrowUp move to the previous source, and Home/End move to the first/last source, with wraparound for arrows.
- Keyboard navigation must focus the destination card and emit `source` with its existing `ExportCandidateSource` value; it must not mutate props.
- Search input, toolbar buttons, source cards, and candidate rows must retain at least a 44 px interaction target.
- Every explicit visible pixel font in the component must remain at least 12 px.
- Do not change candidate filtering, selection, all/clear behavior, loading/empty states, asset completeness, export generation, snapshot history, storage migration, updater recovery, or launch-only work.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies or stage/commit files.

---

### Task 1: Lock standard radio-group keyboard behavior

**Files:**
- Modify: `src/modules/export/components/ExportCandidatePicker.test.ts`
- Modify: `src/modules/export/components/ExportCandidatePicker.vue:1-91`

**Interfaces:**
- Consumes: the existing controlled `source: ExportCandidateSource` prop and `source` emit.
- Produces: `handleSourceKeydown(event: KeyboardEvent)` with no local source state.

- [x] **Step 1: Add the failing roving-tab/arrow-key behavior test**

Render the picker with `source: 'due'`; assert only the selected card has `tabindex="0"`, press ArrowRight and End, and assert destination focus plus emitted `latest_review_session` and `all_active` values.

- [x] **Step 2: Run the picker test and verify red**

Run: `npm test -- src/modules/export/components/ExportCandidatePicker.test.ts`

Expected: the new test fails because all role-radio buttons currently use the browser's default Tab behavior and ignore radio arrow keys.

- [x] **Step 3: Implement controlled roving radio navigation**

Add `data-source`, a controlled `tabindex`, and `@keydown="handleSourceKeydown"` to each source card. The handler must query enabled sibling role-radio buttons, calculate the destination for arrows/Home/End, prevent default scrolling, focus the destination, and emit its typed source value.

- [x] **Step 4: Run the picker behavior suite and verify green**

Run: `npm test -- src/modules/export/components/ExportCandidatePicker.test.ts`

Expected: all picker behavior tests pass.

### Task 2: Lock the export-picker commercial interaction baseline

**Files:**
- Create: `src/modules/export/components/ExportCandidatePickerReadability.test.ts`
- Modify: `src/modules/export/components/ExportCandidatePicker.vue:189-243`

**Interfaces:**
- Consumes: scoped CSS selectors in `ExportCandidatePicker.vue`.
- Produces: a source contract for the 12 px font floor and named minimum heights.

- [x] **Step 1: Write and run the failing source contract**

Run: `npm test -- src/modules/export/components/ExportCandidatePickerReadability.test.ts`

Expected: the font-floor assertion passes and the interaction-target assertion fails because `.search-field input` uses 38 px.

- [x] **Step 2: Raise the search input to 44 px**

Change only `.search-field input` from `min-height: 38px` to `min-height: 44px`; retain the toolbar layout, pill styling, input semantics, and responsive wrapping.

- [x] **Step 3: Run focused export-picker suites**

Run: `npm test -- src/modules/export/components/ExportCandidatePicker.test.ts src/modules/export/components/ExportCandidatePickerReadability.test.ts`

Expected: both files and all tests pass.

### Task 3: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-export-picker-keyboard-touch.md`

- [x] **Step 1: Run static and production validation**

Run `npm run lint`, `npm run typecheck`, and `npm run build`.

- [x] **Step 2: Run the full frontend suite**

Run: `npm test`

- [x] **Step 3: Perform local code review and record completion**

Review controlled-prop integrity, key mapping, wraparound, disabled Tab behavior, selectors, dirty-worktree scope, excluded features, whitespace including new files, and staged state. Resolve every Critical or Important finding, check every plan item, and append exact verification counts.

## Verification Record

- Red phase: 2 test files ran; 2 tests failed and 6 passed as expected. The failures proved the missing roving Tab policy/keyboard navigation and the 38 px search input.
- Focused regression: 2 test files passed, 8 tests passed. Coverage includes ArrowRight/ArrowDown, ArrowLeft/ArrowUp, Home/End, both wraparound directions, destination focus, typed source intents, and disabled radios leaving the Tab order.
- Static validation: `npm run lint` passed with zero warnings; `npm run typecheck` passed with zero TypeScript errors.
- Production build: `npm run build` passed; Vite transformed 2054 modules.
- Full frontend regression: 147 test files passed, 799 tests passed.
- Local code review: no Critical or Important findings. Navigation queries only enabled sibling radios in the current group, preserves the controlled source prop, focuses before emitting the existing source intent, ignores unrelated keys, and prevents native arrow-key scrolling only for handled keys.
- Interaction review: the selected enabled source is the sole Tab stop; all radios use `-1` while disabled/loading. Search input is 44 px, while source cards, toolbar actions, and candidate rows retain their existing 68–84 px/44 px minimum targets and responsive layout.
- Scope review: candidate filtering, selection, all/clear, loading/empty states, prior disabled-state work, snapshot history, export generation, excluded launch-only work, and `src-tauri/src/infrastructure/recognition_visual_split.rs` were not changed by this batch.
- Hygiene: tracked target files passed `git diff --check`; no-index checks on new files reported no whitespace errors (only LF-to-CRLF conversion warnings). No files were staged or committed.
