# Library Interaction Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the library list, problem-detail drawer, and tag editor consistently legible and touch-safe without changing any library behavior.

**Architecture:** Treat the three library components as one daily-workflow UI boundary and enforce it with a source-level Vitest contract. Keep scripts, templates, events, save transactions, navigation guards, and status semantics unchanged; adjust only scoped CSS declarations for font sizes and interactive target dimensions.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Every explicit visible pixel font in the three target components must be at least 12 px.
- High-frequency library buttons, inputs, and the problem-selection label must provide at least a 44 px interaction target.
- Do not change problem queries, filtering semantics, ordered selection, batch actions, detail saving, discard confirmation, adjacent navigation, status mutations, or emitted events.
- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the library interaction and readability contract

**Files:**
- Create: `src/modules/library/components/LibraryInteractionReadability.test.ts`

**Interfaces:**
- Consumes: scoped CSS selectors in `LibraryWorkspace.vue`, `ProblemDetailDrawer.vue`, and `ProblemTagEditor.vue`.
- Produces: a regression contract for the 12 px text floor and named 44 px interaction targets.

- [x] **Step 1: Write the failing cross-component source contract**

Create the test below. It scans all three components for explicit fonts below 12 px, then checks the exact selectors that own library filtering, batch actions, problem selection, drawer actions, edit fields, and tag editing.

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const workspacePath = 'src/modules/library/components/LibraryWorkspace.vue'
const detailPath = 'src/modules/library/components/ProblemDetailDrawer.vue'
const tagEditorPath = 'src/modules/library/components/ProblemTagEditor.vue'
const componentPaths = [workspacePath, detailPath, tagEditorPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('library interaction readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )
    expect(violations).toEqual([])
  })

  it('keeps library list controls touch-safe', () => {
    expect(declarations(workspacePath, '.select-all-action')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.batch-bar button')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.filter-tabs button')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.search-field input')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.select-problem')).toContain('min-height:44px')
  })

  it('keeps drawer and tag-editor controls touch-safe', () => {
    expect(declarations(detailPath, '.icon-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(detailPath, '.edit-header-button, .more-actions-button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.neighbor-actions button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.status-actions button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.edit-paper input, .edit-paper textarea')).toContain('min-height:44px')
    expect(declarations(tagEditorPath, '.tag-chip')).toContain('min-height:44px')
    expect(declarations(tagEditorPath, '.tag-chip button')).toMatch(/width:44px;height:44px/)
    expect(declarations(tagEditorPath, '.tag-editor > input')).toContain('min-height:44px')
  })
})
```

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/modules/library/components/LibraryInteractionReadability.test.ts`

Expected: all three tests FAIL because the target components still contain 10–11 px text and 20–40 px interaction targets.

### Task 2: Raise the library list to the commercial baseline

**Files:**
- Modify: `src/modules/library/components/LibraryWorkspace.vue:405-476`

**Interfaces:**
- Consumes: existing `.select-all-action`, `.batch-bar button`, `.filter-tabs button`, `.search-field input`, `.select-problem`, `.status-dot`, `.problem-preview small`, `.problem-tags span`, and `.empty-footnote` selectors.
- Produces: the list-side CSS contract consumed by `LibraryInteractionReadability.test.ts`; no script or template changes.

- [x] **Step 1: Expand list controls to 44 px**

Update the existing declarations as follows while retaining their other properties:

```css
.select-all-action { min-height: 44px; }
.batch-bar button { min-height: 44px; }
.batch-bar .start-review-action { min-height: 44px; }
.batch-bar .start-exam-action { min-height: 44px; }
.filter-tabs button { min-height: 44px; }
.search-field input { min-height: 44px; }
.select-problem { min-height: 44px; }
```

- [x] **Step 2: Raise list metadata to 12 px**

Change the existing declarations for `.status-dot`, `.problem-preview small`, `.problem-tags span`, and `.empty-footnote` to `font-size: 12px`.

```css
.status-dot,
.problem-preview small,
.problem-tags span,
.empty-footnote {
  font-size: 12px;
}
```

Implement this by editing the existing declarations so their positioning, colors, and animation remain colocated.

### Task 3: Raise the detail drawer and tag editor to the commercial baseline

**Files:**
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue:403-445`
- Modify: `src/modules/library/components/ProblemTagEditor.vue:106-118`

**Interfaces:**
- Consumes: existing drawer and tag-editor selectors.
- Produces: the drawer/tag CSS contract consumed by `LibraryInteractionReadability.test.ts`; no script or template changes.

- [x] **Step 1: Expand drawer controls and edit fields to 44 px**

Update the existing declarations while retaining their other properties:

```css
.icon-button { width: 44px; height: 44px; }
.edit-header-button, .more-actions-button { min-height: 44px; }
.more-actions-button { min-height: 44px; }
.neighbor-actions button { min-height: 44px; }
.status-actions button { min-height: 44px; }
.edit-paper input, .edit-paper textarea { min-height: 44px; }
```

- [x] **Step 2: Raise drawer metadata to 12 px**

Change `.detail-header p`, `.note-paper span`, `.edit-paper label`, `.edit-tags-field`, and `.detail-tags span` to `font-size: 12px` within their existing declarations.

- [x] **Step 3: Expand the tag editor and raise helper copy**

Update the existing declarations so removable tags and their editor are touch-safe:

```css
.tag-chip { min-height: 44px; padding: 0 2px 0 12px; }
.tag-chip button { width: 44px; height: 44px; }
.tag-editor > input { min-height: 44px; }
.tag-editor small { font-size: 12px; }
```

- [x] **Step 4: Run the focused contract and existing library behavior suites**

Run: `npm test -- src/modules/library/components/LibraryInteractionReadability.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts src/modules/library/components/ProblemTagEditor.test.ts`

Expected: all four files PASS; filtering, search, ordered selection, batch locking, drawer focus, dirty-state guards, saving, time-limit validation, and tag keyboard behavior remain intact.

### Task 4: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-library-interaction-readability.md`

**Interfaces:**
- Consumes: the completed styles and tests from Tasks 1–3.
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

Inspect the four target source files and this plan, run `git diff --check` on the Vue and test files, confirm no script/template behavior or excluded functionality changed, replace every plan checkbox with `[x]`, and append the exact focused, build, and full-suite counts.

## Verification Record

- Red phase: all 3 new contract tests failed as expected and reported 10 explicit font declarations below 12 px plus undersized list, drawer, and tag-editor targets.
- Focused green phase: 4 test files, 22 tests passed, covering the source contract and existing library list, drawer, and tag keyboard behavior.
- Static correction: the first typecheck identified strict indexed-access errors in the test's path array. The test and plan were changed to named path constants; focused tests, lint, and typecheck were rerun and passed.
- `npm run lint`: passed with zero warnings after the correction.
- `npm run typecheck`: passed with zero errors after the correction.
- `npm run build`: passed; Vite transformed 2054 modules.
- `npm test`: 140 test files, 778 tests passed.
- Final local code review: no Critical or Important findings; `git diff --check` and the plan placeholder scan passed. This batch changed only scoped CSS in the three product components and added the contract test and plan; excluded launch-only implementation was not edited.
