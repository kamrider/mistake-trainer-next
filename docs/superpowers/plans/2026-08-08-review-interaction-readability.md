# Review Interaction Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make review-room guidance, focus-training status, keyboard hints, and image-viewer controls legible and touch-safe throughout the training workflow.

**Architecture:** Treat `ReviewRoom.vue`, `SchulteFocus.vue`, and `ReviewMediaLightbox.vue` as one training-interaction boundary and enforce it with a source-level Vitest contract. Keep all scripts, templates, review state transitions, timers, ratings, exam navigation, focus persistence, dialog focus handling, and emitted events unchanged; adjust only scoped CSS font sizes and target dimensions.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Every explicit visible pixel font in the three target components must be at least 12 px.
- Training exit, rating-mode toggle, focus actions, and lightbox controls must provide at least a 44 px interaction target.
- Do not change review timing, rating semantics, exam grading, adjacent exam navigation, Schulte sequencing, focus persistence, lightbox navigation, keyboard behavior, or emitted events.
- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the training interaction and readability contract

**Files:**
- Create: `src/modules/review/components/ReviewInteractionReadability.test.ts`

**Interfaces:**
- Consumes: scoped CSS selectors in `ReviewRoom.vue`, `SchulteFocus.vue`, and `ReviewMediaLightbox.vue`.
- Produces: a regression contract for the 12 px text floor and named 44 px interaction targets.

- [x] **Step 1: Write the failing source contract**

Create the following test with named paths so it satisfies strict indexed-access checking:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const roomPath = 'src/modules/review/components/ReviewRoom.vue'
const focusPath = 'src/modules/review/components/SchulteFocus.vue'
const lightboxPath = 'src/modules/review/components/ReviewMediaLightbox.vue'
const componentPaths = [roomPath, focusPath, lightboxPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('review interaction readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )
    expect(violations).toEqual([])
  })

  it('keeps review-room and focus actions touch-safe', () => {
    expect(declarations(roomPath, '.icon-action')).toMatch(/width:44px;height:44px/)
    expect(declarations(roomPath, '.review-header')).toContain('grid-template-columns:44px')
    expect(source(roomPath)).not.toContain('grid-template-columns: 42px')
    expect(declarations(roomPath, '.mode-toggle')).toContain('min-height:44px')
    expect(declarations(focusPath, '.exit-focus')).toContain('min-height:44px')
    expect(declarations(focusPath, '.skip-focus')).toContain('min-height:44px')
  })

  it('keeps lightbox controls touch-safe', () => {
    expect(declarations(lightboxPath, '.lightbox-close')).toMatch(/width:44px;height:44px/)
    expect(declarations(lightboxPath, '.lightbox-controls button')).toContain('min-height:44px')
  })
})
```

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/modules/review/components/ReviewInteractionReadability.test.ts`

Expected: all three tests FAIL because the components contain seven 10–11 px declarations, the review exit and mode toggle are undersized, the review-header track does not match the required exit target, and lightbox controls are 40 px.

### Task 2: Raise the review room to the commercial baseline

**Files:**
- Modify: `src/modules/review/components/ReviewRoom.vue:483-625`

**Interfaces:**
- Consumes: existing `.icon-action`, `.paper-label`, `.media-button span`, `kbd`, and `.mode-toggle` selectors.
- Produces: the review-room CSS contract consumed by `ReviewInteractionReadability.test.ts`; no script or template changes.

- [x] **Step 1: Expand the exit and rating-mode controls**

Update the existing declarations while retaining their remaining properties. The header track must match the 44 px exit action at desktop and mobile widths:

```css
.icon-action {
  width: 44px;
  height: 44px;
}
.review-header {
  grid-template-columns: 44px minmax(180px, 1fr) auto;
}
@media (max-width: 780px) {
  .review-header {
    grid-template-columns: 44px 1fr;
  }
}
.mode-toggle {
  min-height: 44px;
}
```

- [x] **Step 2: Raise review guidance and shortcut hints**

Change `.paper-label`, `.media-button span`, and `kbd` to `font-size: 12px` in their existing declarations.

### Task 3: Raise focus training and the image viewer to the commercial baseline

**Files:**
- Modify: `src/modules/review/components/SchulteFocus.vue:219-249`
- Modify: `src/modules/review/components/ReviewMediaLightbox.vue:146-254`

**Interfaces:**
- Consumes: existing focus and lightbox scoped CSS selectors.
- Produces: the focus/lightbox CSS contract consumed by `ReviewInteractionReadability.test.ts`; no script or template changes.

- [x] **Step 1: Raise every focus-training label to 12 px**

Change `.focus-kicker`, `.focus-status strong, .focus-status span`, `footer`, and `.completion-seal span` to `font-size: 12px` within their existing declarations.

- [x] **Step 2: Expand image-viewer controls to 44 px**

Update the existing declarations:

```css
.lightbox-close {
  width: 44px;
  height: 44px;
}
.lightbox-controls button {
  min-height: 44px;
}
```

- [x] **Step 3: Run the focused contract and existing behavior suites**

Run: `npm test -- src/modules/review/components/ReviewInteractionReadability.test.ts src/modules/review/components/ReviewRoom.test.ts src/modules/review/components/SchulteFocus.test.ts src/modules/review/components/ReviewMediaLightbox.test.ts`

Expected: all four files PASS; timer, reveal, rating, exam navigation, focus sequence, persistence, lightbox keyboard navigation, focus trapping, and close restoration remain intact.

### Task 4: Verify and review the regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-review-interaction-readability.md`

**Interfaces:**
- Consumes: completed styles and tests from Tasks 1–3.
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

- Initial red phase: all 3 contract tests failed as expected and reported 7 explicit font declarations below 12 px plus undersized review-room and lightbox controls.
- Initial focused green phase: 4 test files, 19 tests passed, covering the source contract and existing room, focus, and lightbox behavior.
- Review correction: local review found that the 44 px exit action still occupied a 42 px header track at desktop and mobile widths. A new contract assertion reproduced the failure; both tracks were corrected to 44 px and every gate was rerun.
- Final focused phase: 4 test files, 19 tests passed after the review correction.
- `npm run lint`: passed with zero warnings after the review correction.
- `npm run typecheck`: passed with zero errors after the review correction.
- `npm run build`: passed after the review correction; Vite transformed 2054 modules.
- `npm test`: 141 test files, 781 tests passed after the review correction.
- Final local code review: no remaining Critical or Important findings; `git diff --check` and the plan placeholder scan passed. This batch changed only scoped CSS in the three product components and added the contract test and plan; excluded launch-only implementation was not edited.
