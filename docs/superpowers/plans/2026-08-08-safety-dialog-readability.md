# Safety Dialog Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make library-lock guidance and reusable high-risk confirmations legible and touch-safe without changing their safety semantics.

**Architecture:** Treat `LibraryLockDialog.vue` and `ActionConfirmDialog.vue` as one safety-dialog UI boundary and enforce it with a source-level Vitest contract. Keep lock/restart commands, safe-action focus, focus trapping, Escape/backdrop behavior, confirmation events, document boundaries, and focus restoration unchanged; modify only scoped CSS font sizes and target dimensions.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Every explicit visible pixel font in the two target components must be at least 12 px.
- Library-lock close and all safety-dialog actions must provide at least a 44 px interaction target.
- Do not change lock commands, restart behavior, confirmation meaning, safe default focus, Escape/backdrop cancellation, focus trapping, document boundaries, or emitted events.
- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the safety-dialog readability contract

**Files:**
- Create: `src/app/SafetyDialogReadability.test.ts`

**Interfaces:**
- Consumes: scoped CSS selectors in `LibraryLockDialog.vue` and `components/ActionConfirmDialog.vue`.
- Produces: a regression contract for the 12 px text floor and 44 px close/action targets.

- [x] **Step 1: Write the failing source contract**

Create the following test:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const lockPath = 'src/app/LibraryLockDialog.vue'
const confirmPath = 'src/app/components/ActionConfirmDialog.vue'
const componentPaths = [lockPath, confirmPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('safety dialog readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )
    expect(violations).toEqual([])
  })

  it('keeps the library-lock close and actions touch-safe', () => {
    expect(declarations(lockPath, '.close-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(lockPath, '.dialog-actions button')).toContain('min-height:44px')
  })

  it('keeps reusable confirmation actions touch-safe', () => {
    expect(declarations(confirmPath, '.dialog-actions button')).toContain('min-height:44px')
  })
})
```

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/app/SafetyDialogReadability.test.ts`

Expected: all three tests FAIL because four declarations use 9–11 px text, the lock close action is 36 px, and dialog actions are 42 px.

### Task 2: Raise both safety dialogs to the commercial baseline

**Files:**
- Modify: `src/app/LibraryLockDialog.vue:168-334`
- Modify: `src/app/components/ActionConfirmDialog.vue:108-223`

**Interfaces:**
- Consumes: existing safety-dialog scoped CSS selectors.
- Produces: the CSS contract consumed by `SafetyDialogReadability.test.ts`; no script or template changes.

- [x] **Step 1: Expand library-lock controls to 44 px**

Update the existing declarations while preserving their remaining properties:

```css
.close-button {
  width: 44px;
  height: 44px;
}
.dialog-actions button {
  min-height: 44px;
}
```

- [x] **Step 2: Raise library-lock guidance to 12 px**

Change `.eyebrow`, `.lock-steps strong`, and `.lock-steps small` to `font-size: 12px` within their existing declarations.

- [x] **Step 3: Raise reusable confirmation copy and actions**

Change `ActionConfirmDialog.vue`'s `.eyebrow` to `font-size: 12px` and `.dialog-actions button` to `min-height: 44px` within their existing declarations.

- [x] **Step 4: Run the contract and existing safety behavior suites**

Run: `npm test -- src/app/SafetyDialogReadability.test.ts src/app/LibraryLockDialog.test.ts src/app/components/ActionConfirmDialog.test.ts src/app/dialog-focus.test.ts src/app/dialog-scroll-lock.test.ts src/app/dialog-document-boundary.test.ts`

Expected: all six files PASS; lock confirmation, busy-state focus, safe default focus, Escape/backdrop cancellation, focus restoration, nested scroll ownership, and document inert ownership remain intact.

### Task 3: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-safety-dialog-readability.md`

**Interfaces:**
- Consumes: completed styles and tests from Tasks 1–2.
- Produces: exact verification records and a completed checklist.

- [x] **Step 1: Run static validation**

Run: `npm run lint`

Expected: PASS with zero warnings.

Run: `npm run typecheck`

Expected: PASS with zero TypeScript errors.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: PASS through mobile asset verification, Vue type checking, and Vite production bundling.

- [x] **Step 3: Run the full frontend suite**

Run: `npm test`

Expected: all Vitest files and tests PASS.

- [x] **Step 4: Review scope and record completion**

Inspect the three target source files and this plan, run `git diff --check` on the Vue and test files, confirm no script/template behavior or excluded functionality changed, replace every plan checkbox with `[x]`, and append the exact focused, build, and full-suite counts.

## Verification Record

- Red phase: all 3 contract tests failed as expected and reported 4 explicit font declarations below 12 px, a 36 px lock close target, and 42 px dialog actions.
- Focused green phase: 6 test files, 13 tests passed, covering the source contract plus lock confirmation, busy focus, safe default focus, Escape/backdrop cancellation, focus restoration, scroll ownership, and document-boundary ownership.
- `npm run lint`: passed with zero warnings.
- `npm run typecheck`: passed with zero errors.
- `npm run build`: passed; Vite transformed 2054 modules.
- `npm test`: 142 test files, 784 tests passed.
- Final local code review: no Critical or Important findings; `git diff --check` and the plan placeholder scan passed. This batch changed only scoped CSS in the two safety-dialog components and added the contract test and plan; excluded launch-only implementation was not edited.
