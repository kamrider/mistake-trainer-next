# Settings Learning Preference Panels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract subject and review-rhythm presentation from `SettingsView.vue` while preserving profile-scoped persistence, validation, accessible names, and unsaved selections after failures.

**Architecture:** `SettingsView` remains the orchestration boundary for Tauri commands, validation messages, and loaded/saved preferences. Two presentational components render immutable props and emit explicit user intentions; they never invoke commands or mutate preference objects received from the parent.

**Tech Stack:** Vue 3, TypeScript with `exactOptionalPropertyTypes`, Vitest, Testing Library, Lucide Vue.

## Global Constraints

- Preserve unrelated dirty-worktree changes; do not stage or commit.
- Do not edit OCR, recognition, migration persistence, installer, release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Preserve Chinese copy, section IDs, accessible names, settings-directory targets, and current save payloads.
- Keep all Tauri commands and `normalizeAppResult` calls in `SettingsView`.
- Child components must emit immutable updates instead of mutating props.
- Preserve the 760 px responsive layout, 44 px narrow-screen targets, focus-visible styling, and reduced-motion behavior.

---

### Task 1: Extract the subject preference panel

**Files:**

- Create: `src/app/components/SettingsSubjectPanel.vue`
- Create: `src/app/components/SettingsSubjectPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

```ts
{
  preferences: SubjectPreferences
  builtinSubjects: readonly string[]
  customSubject: string
  saving: boolean
  message: string
}
```

The component emits:

```ts
{
  toggleSubject: [subject: string, enabled: boolean]
  updateCustomSubject: [value: string]
  addCustomSubject: []
  removeCustomSubject: [subject: string]
  updateCaptureSound: [enabled: boolean]
  save: []
}
```

- [x] Add component tests for checked built-ins, the last-subject guard, custom-subject input/add/remove intentions, sound updates, busy save state, and status copy.
- [x] Run the focused test and confirm the component module is missing.
- [x] Implement the component with `settings-subjects`, `常用科目`, explicit event handlers, and no prop mutation.
- [x] Add parent handlers that replace `SubjectPreferences` with copied arrays/objects before calling the existing add/remove/save logic.
- [x] Replace the inline subject section and move only its scoped styles, mobile targets, and reduced-motion behavior.
- [x] Run component and parent integration tests.

### Task 2: Extract the review-rhythm panel

**Files:**

- Create: `src/app/components/SettingsReviewPanel.vue`
- Create: `src/app/components/SettingsReviewPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`

**Interfaces:**

```ts
export interface SettingsReviewOption {
  value: ReviewFocusPolicy
  title: string
  hint: string
}
```

The component requires `preferences`, `options`, `saving`, and `message`; it emits `updateFocusPolicy` and `save`.

- [x] Add component tests for option names, selected state, immutable policy intent, save intent, failure-message visibility, and busy state.
- [x] Run the focused test and confirm the component module is missing.
- [x] Implement the component with `settings-review`, `训练间的专注插曲`, and explicit event handlers.
- [x] Add a parent policy-update handler that replaces the preference object and keep the command payload unchanged.
- [x] Replace the inline review section and move only its styles, mobile layout, and reduced-motion behavior.
- [x] Run component and parent integration tests.

### Task 3: Verify behavior and layout

- [x] Run `npm run typecheck`.
- [x] Run `npm run lint`.
- [x] Run `npm test -- --run`.
- [x] Run `git diff --check`.
- [x] Verify desktop and 375 px layouts, directory anchors, 44 px narrow-screen controls, and empty browser warning/error logs.
- [x] Review the scoped diff for command leakage, prop mutation, copy drift, or changes to excluded pre-launch work.
