# Settings Sync and Account Panels Implementation Plan

> **For Codex:** Execute this plan task by task with test-first checkpoints. Keep Tauri command orchestration in `SettingsView.vue`; the extracted components are presentation boundaries only.

**Goal:** Reduce the commercial maintenance risk of the settings page by extracting the sync-backend and cloud-account interfaces without changing their behavior, local-first guarantees, or security confirmation flow.

**Architecture:** Add two typed Vue components under `src/app/components`. They receive immutable state and emit user intentions. `SettingsView.vue` remains the application coordinator for backend changes, authentication, manual sync, lock confirmation, overview refresh, and focus restoration.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Vitest, Testing Library, Tauri typed bindings.

---

## Task 1: Specify the presentation contracts

**Files:**
- Create: `src/app/components/SettingsSyncBackendPanel.test.ts`
- Create: `src/app/components/SettingsCloudAuthPanel.test.ts`

- [x] Cover successful, unavailable, busy, and failed backend states.
- [x] Cover signed-out, connected, unconfigured, and busy cloud-account states.
- [x] Verify every action is emitted as an intention and the password remains a password field.
- [x] Run the focused tests and confirm they fail because the components do not exist.

## Task 2: Implement the components

**Files:**
- Create: `src/app/components/SettingsSyncBackendPanel.vue`
- Create: `src/app/components/SettingsCloudAuthPanel.vue`

- [x] Implement typed props and emits with no direct command imports.
- [x] Preserve IDs, accessible names, live regions, disabled states, local-first explanations, and the China-network warning.
- [x] Give each scoped component its own complete desktop, narrow-screen, focus, and reduced-motion styles.
- [x] Expose the cloud sign-out trigger focus method for dialog cancellation.
- [x] Run the focused component tests to green.

## Task 3: Reconnect the settings coordinator

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] Replace the two inline sections with component instances.
- [x] Keep authentication, backend, sync, conflict refresh, and lock commands in the parent.
- [x] Restore focus to the extracted sign-out trigger after cancelling the security dialog.
- [x] Remove only styles now owned by the two child components.
- [x] Run the parent integration tests.

## Task 4: Commercial-quality verification

- [x] Run TypeScript checking and ESLint.
- [x] Run the complete frontend test suite.
- [x] Run the production build and Rust boundary contract.
- [x] Review the diff for leaked commands, password exposure, mutable state, regressions, and unrelated edits.
- [x] Exercise desktop and 375 px layouts with real browser interactions and inspect runtime logs.
