# Library Selection Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make library batch selection use valid, non-duplicated accessibility semantics, announce the aggregate selection count, and provide a full 44×44 checkbox target.

**Architecture:** Keep native checkboxes as the single authoritative selected state and retain the card's visual `selected` class only for presentation. Remove unsupported `aria-selected` from article elements, expose the changing aggregate count through one atomic polite status, and complete the existing touch contract by adding minimum width to the checkbox label.

**Tech Stack:** Vue 3, TypeScript, Testing Library Vue, Vitest, source-level readability contracts.

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support operations, account deletion, device migration, updater recovery, or SLA work.
- Preserve all unrelated dirty-worktree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve batch order, visual card selection, training/exam launch actions, busy-state locking, filters, detail opening, and selection events.
- Do not turn cards into listbox options because each card contains multiple interactive descendants.
- Keep native checkbox checked/disabled semantics as the source of truth.

---

### Task 1: Valid selection semantics and touch target

**Files:**
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`
- Modify: `src/modules/library/components/LibraryInteractionReadability.test.ts`
- Modify: `src/modules/library/components/LibraryWorkspace.vue`

**Interfaces:**
- Consumes: `selectedProblemIds`, native selection checkbox, `.selection-summary`, and `.select-problem` label.
- Produces: articles without unsupported `aria-selected`; atomic polite selection-count status; `.select-problem` minimum width and height of 44px.

- [x] **Step 1: Write failing semantic and touch-contract tests**

Extend the ordered-selection test to assert every problem article omits `aria-selected`, both native checkboxes are checked, and the aggregate count is exposed as a status. Rerender with one selected ID and prove the same status reflects the new total.

```ts
const problemCards = screen.getAllByRole('article')
for (const card of problemCards) expect(card).not.toHaveAttribute('aria-selected')
for (const checkbox of screen.getAllByRole('checkbox')) expect(checkbox).toBeChecked()
const selectionStatus = screen.getByRole('status')
expect(selectionStatus).toHaveAttribute('aria-live', 'polite')
expect(selectionStatus).toHaveAttribute('aria-atomic', 'true')
expect(selectionStatus).toHaveTextContent(/2\s*道题已放入本轮卡组/)

await view.rerender({ selectedProblemIds: ['problem-1'] })
expect(selectionStatus).toHaveTextContent(/1\s*道题已放入本轮卡组/)
```

Extend the readability contract:

```ts
expect(declarations(workspacePath, '.select-problem')).toMatch(/min-width:44px.*min-height:44px|min-height:44px.*min-width:44px/)
```

- [x] **Step 2: Run focused tests and verify red state**

Run: `pnpm vitest run src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/LibraryInteractionReadability.test.ts`

Expected: tests fail because selected articles expose `aria-selected`, the aggregate summary has no status/live attributes, and `.select-problem` has no minimum width.

- [x] **Step 3: Implement minimal semantic and touch changes**

Remove the article's `:aria-selected` binding while keeping its visual selected class. Make the aggregate summary an atomic polite status:

```vue
<div
  class="selection-summary"
  role="status"
  aria-live="polite"
  aria-atomic="true"
>
```

Complete the checkbox label target:

```css
.select-problem {
  display: inline-flex;
  min-width: 44px;
  min-height: 44px;
  justify-content: center;
  gap: 9px;
  align-items: center;
  cursor: pointer;
}
```

- [x] **Step 4: Run focused and static verification**

Run: `pnpm vitest run src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/LibraryInteractionReadability.test.ts`

Expected: all focused library interaction and readability tests pass.

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

Run: `git diff --check -- src/modules/library/components/LibraryWorkspace.vue src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/LibraryInteractionReadability.test.ts`

Expected: no whitespace errors.

Run: `git diff --cached --name-only`

Expected: no staged files.

Inspect the target diff and confirm only selection semantics, count announcement, checkbox target sizing, tests, and completed plan bookkeeping were added; preserve every unrelated working-tree change.

## Verification Record

- Focused red state: tests failed on unsupported article `aria-selected` and missing `.select-problem` minimum width; the count status assertions were then exercised after the semantic fix.
- Focused final state: two focused files and eight tests passed, covering native checkbox state, changing aggregate status, and the 44×44 target contract.
- Static checks: `pnpm lint` and `pnpm typecheck` passed.
- Production build: `pnpm build` passed with 2055 modules transformed.
- Full frontend regression: 147 test files and 806 tests passed.
- Review result: no unresolved Critical or Important findings; the checkbox is the only accessibility selection state and the live region reports only aggregate count changes.
