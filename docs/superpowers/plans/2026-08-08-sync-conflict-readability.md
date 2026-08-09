# Sync Conflict Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every piece of conflict-decision information legible, every conflict action touch-safe, and resolved-card transitions stable at desktop and narrow widths.

**Architecture:** Keep conflict data, resolution commands, confirmation behavior, and focus recovery unchanged. Add a component-local source contract that treats `SyncConflictCenter.vue` as one commercial UI boundary, then adjust only its scoped CSS: 12 px minimum explicit visible text, 44 px minimum buttons, and a positioned transition list whose leaving card fills that list at every viewport width.

**Tech Stack:** Vue 3 single-file components, scoped CSS, TypeScript, Vitest.

## Global Constraints

- Do not change the sync protocol, conflict grouping, local/remote choice semantics, durable resolution commands, bulk confirmation, or focus recovery.
- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the conflict-center commercial UI contract

**Files:**
- Create: `src/modules/sync/components/SyncConflictCenterReadability.test.ts`

**Interfaces:**
- Consumes: scoped CSS selectors in `SyncConflictCenter.vue`.
- Produces: a source-level contract requiring 12 px minimum explicit text, 44 px buttons, and stable transition positioning at every viewport width.

- [x] **Step 1: Write the failing readability and interaction contract**

Create the test with source and declaration helpers, then assert there are no explicit pixel fonts below 12, all component buttons have a 44 px minimum height, `.conflict-list` establishes a relative positioning context, and the leaving card fills that list.

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const componentPath = 'src/modules/sync/components/SyncConflictCenter.vue'
const componentSource = readFileSync(resolve(componentPath), 'utf8')

function declarations(selector: string) {
  const compact = componentSource.replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('sync conflict center readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = [...componentSource.matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
      .filter(match => Number(match[1]) < 12)
      .map(match => match[0])
    expect(violations).toEqual([])
  })

  it('keeps every conflict decision action touch-safe', () => {
    expect(declarations('button')).toContain('min-height:44px')
  })

  it('keeps resolved-card transitions inside the list at every viewport width', () => {
    expect(declarations('.conflict-list')).toContain('position:relative')
    expect(declarations('.conflict-card-leave-active')).toContain('width:100%')
  })
})
```

- [x] **Step 2: Run the focused contract and verify red**

Run: `npm test -- src/modules/sync/components/SyncConflictCenterReadability.test.ts`

Expected: FAIL because the component contains explicit 10–11 px fonts, its shared button rule has no minimum height, the list has no positioning context, and the leaving-card width is calculated against the wrong containing block.

### Task 2: Raise the conflict decision UI to the contract

**Files:**
- Modify: `src/modules/sync/components/SyncConflictCenter.vue:515-770`

**Interfaces:**
- Consumes: existing class names and responsive breakpoint at `max-width: 760px`.
- Produces: the CSS contract consumed by `SyncConflictCenterReadability.test.ts`; no script or template interface changes.

- [x] **Step 1: Make every action touch-safe**

Add the minimum height to the component-scoped shared button rule without changing padding or colors.

```css
button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  justify-content: center;
  padding: 9px 13px;
  border: 1px solid var(--line);
  border-radius: 10px;
  color: var(--green-deep);
  background: var(--paper-raised);
  cursor: pointer;
  transition: transform var(--motion-feedback) var(--ease-standard);
}
```

- [x] **Step 2: Raise all decision information to at least 12 px**

Change the explicit font sizes for `.entity-type`, `.conflict-card-header small`, `.version-panel > header`, `.value-chips span`, `.value-chips em`, `.object-value dt`, `.object-value dd`, and `.group-actions span` to `12px`. Do not change the existing 12 px or larger declarations.

```css
.entity-type,
.conflict-card-header small,
.version-panel > header,
.value-chips span,
.value-chips em,
.object-value dt,
.object-value dd,
.group-actions span {
  font-size: 12px;
}
```

Implement this by updating the existing selector declarations so their other visual responsibilities remain colocated.

- [x] **Step 3: Stabilize leaving-card geometry**

Give the transition list a positioning context and make the leaving card fill that list. Once the list is the containing block, subtracting the outer component padding would incorrectly shrink the card.

```css
.conflict-list {
  position: relative;
  display: grid;
  gap: 14px;
}
.conflict-card-leave-active {
  position: absolute;
  width: 100%;
}
```

- [x] **Step 4: Run the focused UI contract and behavior suite**

Run: `npm test -- src/modules/sync/components/SyncConflictCenterReadability.test.ts src/modules/sync/components/SyncConflictCenter.test.ts src/modules/sync/components/SyncConflictBulkDialog.test.ts`

Expected: all three files PASS; the CSS contract is green and all existing resolution, retry, confirmation, deletion, locking, and focus behavior remains intact.

### Task 3: Verify the regression boundary and review the batch

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-sync-conflict-readability.md`

**Interfaces:**
- Consumes: the completed component and tests from Tasks 1–2.
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

Inspect `git diff -- src/modules/sync/components/SyncConflictCenter.vue src/modules/sync/components/SyncConflictCenterReadability.test.ts docs/superpowers/plans/2026-08-08-sync-conflict-readability.md`, run `git diff --check` on the two source files, confirm no sync semantics or excluded functionality changed, then replace every plan checkbox with `[x]` and append the exact verification counts.

## Verification Record

- Red phase: the new contract failed all 3 expected checks and reported 7 explicit font declarations below 12 px.
- Focused green phase: 3 test files, 14 tests passed, covering the source contract plus existing conflict resolution and bulk confirmation behavior.
- Review correction: local review found that subtracting outer padding after establishing `.conflict-list` as the containing block would shrink leaving cards. The implementation and contract were corrected to `width: 100%` and every gate was rerun.
- `npm run lint`: passed with zero warnings after the review correction.
- `npm run typecheck`: passed with zero errors after the review correction.
- `npm run build`: passed after the review correction; Vite transformed 2054 modules.
- `npm test`: 139 test files, 775 tests passed after the review correction.
- Final local code review: no remaining Critical or Important findings; `git diff --check` and the plan placeholder scan passed. No sync semantics or excluded launch-only implementation was edited in this batch.
