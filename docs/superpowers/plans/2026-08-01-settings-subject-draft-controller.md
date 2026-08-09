# Settings Subject Draft Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent invalid or unexplained subject-draft interactions and give custom-subject input a clear, accessible commercial-quality contract.

**Architecture:** Add a focused Vue composable that operates on the existing `SubjectPreferences` ref and exclusively owns custom-subject input, local validation copy, immutable draft mutations, and the “at least one enabled subject” invariant. Keep persistence, queued-save revisions, and server-result application in the existing save controller. The presentation component remains intention-only but exposes a stable input label, disabled empty submission, and visible invariant guidance.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue.

## Global Constraints

- Preserve `SubjectPreferences` and `SubjectPreferencesInput` generated contracts and the existing queued persistence semantics.
- Every successful draft mutation must call `markSubjectDraftChanged()` exactly once; rejected or no-op intentions must not dirty the draft.
- Never allow removal or disabling to leave `enabledSubjects` empty.
- Reject custom names that are blank, longer than 40 characters, duplicate an existing custom name, or duplicate a built-in subject; compare duplicates after trimming and case-folding.
- Keep the maximum of 20 custom subjects and retain the exact existing maximum-count copy.
- Keep editing available during an in-flight save so the queued-save controller can persist the newest draft.
- Do not change review preferences, persistence commands, synchronization, authentication, storage/device migration, updater recovery, Rust, bindings, or launch-gate work.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize draft invariants and accessible feedback

**Files:**

- Create: `src/app/composables/useSubjectPreferenceDraft.test.ts`
- Modify: `src/app/components/SettingsSubjectPanel.test.ts`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: Add controller mutation tests**

Cover successful trimmed custom addition, immutable toggle/sound updates, custom removal, exactly-one `onChanged` call per mutation, and no-op stability.

- [x] **Step 2: Add rejection tests**

Assert blank, over-40, built-in duplicate, case-insensitive custom duplicate, 21st custom subject, disabling the last enabled subject, and deleting the only enabled custom subject are rejected with precise copy and without changing preferences or calling `onChanged`.

- [x] **Step 3: Add panel accessibility tests**

Require the custom input to be discoverable as `自定义科目名称`, empty submission to be disabled, and visible copy to explain why the final enabled subject cannot be removed.

- [x] **Step 4: Add page-level duplicate and deletion regressions**

Submit `数学` as a custom subject and assert the duplicate message appears without dirty persistence; load a draft whose only enabled subject is custom, click delete, and assert the chip remains with actionable guidance.

- [x] **Step 5: Run the new suites red**

Run controller, subject panel, and Settings view tests. Expect controller import resolution to fail, the input label/invariant copy assertions to fail, and page actions to remain silent or invalid.

### Task 2: Implement the subject draft controller

**Files:**

- Create: `src/app/composables/useSubjectPreferenceDraft.ts`
- Test: `src/app/composables/useSubjectPreferenceDraft.test.ts`

- [x] **Step 1: Centralize input and local message state**

Expose readonly `customSubject` and `message`, `updateCustomSubject(value)`, and `clearMessage()`; input edits clear stale local validation without marking the persisted draft dirty.

- [x] **Step 2: Enforce custom-subject validation**

Trim accepted values, apply 40-character and 20-item limits, reject built-in/custom duplicates using trimmed lowercase comparison, append accepted subjects immutably, enable them if needed, clear the input, and call `onChanged` once.

- [x] **Step 3: Enforce enabled-subject invariants**

Reject disabling or deleting the sole enabled subject with actionable copy. Apply successful toggles, removals, and sound changes immutably; suppress exact no-ops.

- [x] **Step 4: Run controller tests green and typecheck**

Run the new controller suite and `npm run typecheck`.

### Task 3: Integrate the panel and Settings view

**Files:**

- Modify: `src/app/components/SettingsSubjectPanel.vue`
- Modify: `src/app/components/SettingsSubjectPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: Improve the panel contract**

Add `aria-label="自定义科目名称"`, disable empty trimmed submission, bind validation/status copy through `aria-describedby`, and render `至少保留一个常用科目；最后一个已选科目会保持启用。` adjacent to subject choices.

- [x] **Step 2: Replace page-local draft mutations**

Instantiate `useSubjectPreferenceDraft` with the existing preferences ref, built-in list, and queued-save `markChanged`; remove page-local add/remove/toggle/sound functions and local message ref.

- [x] **Step 3: Preserve queued-save composition**

Keep `subjectMessage` priority as local validation before save status, clear local validation when save begins, and retain the existing refresh/navigation dirty guards.

- [x] **Step 4: Run focused and adjacent tests**

Run draft controller, subject panel, Settings view, queued preference save, unsaved guard, and page loader tests.

### Task 4: Commercial-quality gates and local review

- [x] **Step 1: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 2: Review invariant ownership and copy**

Confirm no page-local subject mutation remains, every successful mutation dirties once, every rejection leaves state clean, duplicates include built-ins, final enabled subject cannot disappear, and queued edits during save remain supported.

- [x] **Step 3: Check workspace hygiene and record evidence**

Run target whitespace checks, confirm the index is empty, confirm build output remains ignored, and verify the pre-existing recognition modification was untouched. Record baseline, red, green, final gates, review, and hygiene below.

## Verification record

- Baseline: subject panel, Settings view, and queued preference save passed, 3 files / 66 tests.
- Red: the controller suite failed at import resolution; the panel lacked a named custom-subject textbox, empty submission remained enabled, and no final-subject explanation existed; the Settings regression confirmed the only enabled custom subject was actually removed and duplicate built-ins were silent.
- Core green: draft controller, subject panel, and Settings view passed, 3 files / 68 tests. The first green run exposed and corrected a unit-fixture issue where the controlled input prop was not rerendered after its emit.
- Focused integration: draft controller, subject panel, Settings view, queued preference save, unsaved guard, and page loader passed, 6 files / 85 tests; TypeScript passed.
- Final gates: ESLint passed with zero warnings; `vue-tsc --noEmit` passed directly and through the production build; mobile vendor verification and Vite production build passed; serial full Vitest passed, 132 files / 738 tests.
- Review: the 127-line controller is the only owner of subject draft mutations and local validation. Every accepted add/remove/toggle/sound change flows through one `commit` call and therefore one queued-save revision increment; blank, overlong, duplicate, maximum-count, last-disable, missing-target, and exact no-op paths return without dirtying. Duplicate comparison trims, NFKC-normalizes, and case-folds built-in and custom names.
- Integration: `SettingsView.vue` is 942 lines and retains only server/page-loader writes to `subjectPreferences`; all interactive draft mutations are delegated to the controller. The subject panel exposes `自定义科目名称`, disables blank submission, associates status copy with the textbox, and displays the final-subject invariant while leaving editing available during queued saves.
- Hygiene: target whitespace checks passed; nothing was staged or committed; production output remained ignored; `recognition_visual_split.rs` remains an untouched pre-existing modification; excluded launch-gate work was not changed.
