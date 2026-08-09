# Capture Layout Template Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. This session cannot delegate because the active workspace policy forbids sub-agents unless the user explicitly requests them.

**Goal:** Extract the layout-template controls and destructive regroup confirmation from `CaptureWorkspace.vue` into a focused component with its own interaction, accessibility, and responsive-style boundary.

**Architecture:** `CaptureWorkspace` remains the feature orchestrator and supplies batch-derived counts. A new private presentation/controller component owns layout inputs, split-index defaults, confirmation lifecycle, document boundary acquisition, focus trapping and restoration, and emits the existing `applyLayout` payload unchanged. No API, store, database, or Tauri contract changes.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Testing Library, Vitest, scoped CSS.

## Global constraints

- Preserve the existing `applyLayout(mode, questions, answers, splitIndex)` contract and all Chinese interaction copy.
- Preserve direct apply when no drafts exist and impact confirmation when drafts exist.
- Preserve Escape, Tab/Shift+Tab trapping, launcher focus return, background inert state, scroll lock, nested boundary safety, busy guards, and source-image retention copy.
- Keep explicit controls and actions at least 44 px high and visible fonts at least 12 px.
- Do not modify launch-only concerns, backup/storage migration flows, or `recognition_visual_split.rs`.
- Do not add dependencies, stage, commit, or overwrite unrelated dirty-worktree changes.

## Task 1: Lock the child interaction contract with failing tests

**Files:**

- Create: `src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts`

**Interfaces:**

```ts
props: {
  itemCount: number
  draftCount: number
  affectedNoteCount: number
  busy: boolean
}

emit('apply', mode, questionImages, answerImages, splitIndex)
```

- [x] Verify a batch without drafts applies immediately with the unchanged default payload.
- [x] Verify an existing batch shows exact impact counts and emits only after confirmation.
- [x] Verify split mode resets its midpoint when `itemCount` changes and emits the split index.
- [x] Verify Escape, focus containment/return, background inert state, scroll lock, busy-state protection, and unmount cleanup.
- [x] Run the targeted test and confirm it fails because the component does not exist.

## Task 2: Implement the focused layout-template component

**Files:**

- Create: `src/modules/capture/components/CaptureLayoutTemplatePanel.vue`

- [x] Move layout mode, question/answer counts, split index, confirmation state, and lifecycle into the component.
- [x] Reuse `acquireDialogDocumentBoundary` and `trapDialogFocus`; release the boundary idempotently on every close/unmount path.
- [x] Move the layout panel, impact dialog markup, scoped layout CSS, and responsive rules into the component.
- [x] Run the child test and confirm it passes.

## Task 3: Replace workspace-owned layout state with the child boundary

**Files:**

- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspaceReadability.test.ts`

- [x] Import and render `CaptureLayoutTemplatePanel`, forwarding the emitted payload as the existing `applyLayout` event.
- [x] Remove obsolete workspace refs, functions, imports, unmount cleanup, markup, and layout-only CSS.
- [x] Keep the workspace integration regressions for impact copy, event forwarding, and document boundary behavior.
- [x] Extend the readability contract to scan both workspace and extracted component styles.
- [x] Run the affected component and readability tests.

## Task 4: Commercial-quality validation and local review

**Files:**

- Verify all modified files.

- [x] Run ESLint and TypeScript typecheck.
- [x] Run the production build.
- [x] Run the complete Vitest suite.
- [x] Review the diff for public-contract drift, missing boundary release paths, scoped-style regressions, and unrelated edits.
- [x] Record actual verification results in this plan.

## Verification record

- Red phase: `npm test -- src/modules/capture/components/CaptureLayoutTemplatePanel.test.ts` failed only because `CaptureLayoutTemplatePanel.vue` did not exist.
- Affected regressions: 3 files, 39 tests passed.
- ESLint: passed with zero warnings.
- Vue/TypeScript typecheck: passed.
- Production build: passed; Vite transformed 2054 modules.
- Complete Vitest: 137 files, 764 tests passed.
- One parallel build/test run produced an unrelated `App.test.ts` route timeout. The file passed 6/6 in isolation, then the serial full suite passed 764/764.
- Local review found no Critical or Important issues. Added direct regressions for background `inert` and unmount-time document-boundary release.
- `CaptureWorkspace.vue` decreased from 1374 to 1244 lines. No public event, API, schema, dependency, excluded launch concern, backup/storage migration flow, or recognition algorithm changed.
