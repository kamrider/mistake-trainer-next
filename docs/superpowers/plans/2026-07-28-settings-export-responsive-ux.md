# Settings, Export, and Responsive UX Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make settings and export workflows understandable as distinct product areas, while preserving keyboard/screen-reader behavior and readable controls on narrow Windows windows.

**Architecture:** Keep data loading and mutations in the existing route views. Extract only presentational navigation/workflow components, add semantic group metadata to the existing settings directory, and expose stable section anchors in the report/export route. Do not change command contracts or generated bindings.

**Tech Stack:** Vue 3, TypeScript, Vue Router, Testing Library, Vitest, scoped CSS.

## Global Constraints

- Preserve all existing commands, persistence behavior, and route URLs.
- Do not edit generated `src/shared/api/bindings.ts`.
- Do not implement the separately deferred pre-launch items: licensing, privacy/legal pages, support operations, account deletion, device migration, update-failure recovery, or SLA.
- Do not stage or commit the dirty worktree.

---

## Chunk 1: Settings Information Architecture

### Task 1: Specify grouped settings navigation

**Files:**
- Modify: `src/app/components/SettingsSectionNav.test.ts`
- Modify: `src/app/components/SettingsSectionNav.vue`

- [x] Add a failing component test proving navigation items are announced under “账户与同步”, “学习体验”, “数据与安全”, and “应用维护” groups.
- [x] Extend `SettingsSectionLink` with a required `group` field and derive ordered groups without changing click/observer behavior.
- [x] Render each group with a visible label and an accessible group name.
- [x] Add `aria-controls` to each section button and make the horizontal scroller keyboard-focusable.
- [x] Ensure scroll controls and section buttons meet a 44px minimum target.
- [x] Run `pnpm vitest run src/app/components/SettingsSectionNav.test.ts`.

### Task 2: Apply product groups to settings

**Files:**
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/views/SettingsView.vue`

- [x] Add a failing route test for the four product groups and their representative settings.
- [x] Assign every settings section to exactly one group; keep dynamic sections conditional.
- [x] Add missing `aria-labelledby` wiring to the backup panel.
- [x] Keep all section IDs stable so existing deep scrolling remains compatible.
- [x] Run `pnpm vitest run src/app/views/SettingsView.test.ts src/app/components/SettingsSectionNav.test.ts`.

---

## Chunk 2: Export Information Architecture

### Task 3: Expose a clear export workflow

**Files:**
- Create: `src/modules/export/components/ExportWorkflowGuide.vue`
- Create: `src/modules/export/components/ExportWorkflowGuide.test.ts`
- Modify: `src/app/views/ReportView.test.ts`
- Modify: `src/app/views/ReportView.vue`

- [x] Add a failing component test for the ordered workflow: choose questions, save a reusable snapshot, generate a file.
- [x] Implement a compact semantic ordered list that explains which step stores data and which step creates local files.
- [x] Add a route-level section directory linking “学习概览” and “导出中心”.
- [x] Give the report overview and export center stable IDs and labelled section headings.
- [x] Place the workflow guide before export controls without changing export mutations.
- [x] Run `pnpm vitest run src/modules/export/components/ExportWorkflowGuide.test.ts src/app/views/ReportView.test.ts`.

---

## Chunk 3: Narrow-Screen Readability

### Task 4: Raise type and target floors

**Files:**
- Modify: `src/app/components/SettingsSectionNav.vue`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/modules/export/components/ExportCandidatePicker.vue`
- Modify: `src/modules/export/components/ExportSnapshotHistory.vue`

- [x] Replace 9–11px instructional and status copy in the touched workflows with a 12px minimum.
- [x] Use 13–14px for form labels and primary supporting copy.
- [x] At `max-width: 760px`, ensure controls are at least 44px high and groups stack without horizontal page overflow.
- [x] Preserve reduced-motion behavior and horizontal navigation overflow cues.
- [x] Run `pnpm lint`.
- [x] Run `pnpm typecheck`.
- [x] Run `pnpm test:coverage`.

## Self-Review

- [x] Confirm every settings section appears exactly once in the directory.
- [x] Confirm export commands and payloads are unchanged.
- [x] Confirm no deferred pre-launch feature was added.
- [x] Confirm keyboard focus, accessible names, and narrow-window layout are covered by tests.
