# Subject Presets and Card Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist a profile-level subject palette, apply one subject to a whole capture batch from the top of the organizer, and make question/answer card drops visually distinct, smooth, accessible, and optionally audible.

**Architecture:** A new schema-v5 `profile_preferences` row stores enabled built-in subjects, custom subjects, and capture sound preference for the active account/profile. Typed Rust commands load and atomically replace the validated preferences. The capture view loads those preferences once, renders subject chips near the organizer header, and reuses the existing revision-checked batch/draft commands. Pointer drag state exposes the hovered target and role so Vue can render transform/opacity-only lift, magnet, and settle feedback without changing persistence semantics.

**Tech Stack:** Vue 3, TypeScript strict, Vitest/Testing Library, Tauri 2, Rust, rusqlite/SQLCipher, tauri-specta, Web Audio API.

## Global Constraints

- Built-in subjects are exactly: `语文`, `数学`, `英语`, `政治`, `历史`, `地理`, `物理`, `化学`, `生物`.
- Preferences are isolated by `account_id` and `profile_id` and live in the encrypted database.
- Custom subjects are trimmed, unique, at most 20 entries, and at most 40 Unicode scalar values each.
- At least one subject must remain enabled.
- Batch apply changes the batch subject and clears per-draft subject overrides so every draft receives the chosen subject deterministically.
- Drag animation uses only `transform`, `opacity`, border, and background; it must be disabled by `prefers-reduced-motion`.
- Drop sound is a short synthesized local tone, defaults on, makes no network request, and can be disabled in Settings.
- No double-click interaction is introduced.

---

### Task 1: Persist profile capture preferences

**Files:**
- Create: `src-tauri/migrations/0005_profile_preferences.sql`
- Create: `src-tauri/src/modules/preferences.rs`
- Create: `src-tauri/src/commands/preferences.rs`
- Create: `src-tauri/tests/preferences_store.rs`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/tests/database_schema.rs`
- Modify: `src-tauri/src/modules/backup.rs`

**Interfaces:**
- Produces: `SubjectPreferences { enabled_subjects: Vec<String>, custom_subjects: Vec<String>, capture_sound_enabled: bool }`.
- Produces: `subject_preferences_get() -> AppResult<SubjectPreferences>` and `subject_preferences_save(input: SubjectPreferencesInput) -> AppResult<SubjectPreferences>`.

- [ ] **Step 1: Write failing migration and preference-store tests**

Test schema version 5, default nine subjects, profile isolation, normalization/deduplication, rejection of empty enabled selections, custom subject count/length bounds, and sound persistence.

- [ ] **Step 2: Run the failing Rust tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema --test preferences_store`

Expected: FAIL because schema v5 and preference commands do not exist.

- [ ] **Step 3: Implement migration, validation, commands, and registration**

Create one strict table keyed by `(account_id, profile_id)` with JSON validity checks and a learner-profile foreign key. Return the nine-subject default without writing when no row exists; save through an upsert after trimming, stable deduplication, built-in/custom membership validation, and bounds checks.

- [ ] **Step 4: Update backup schema validation**

Require `profile_preferences` for schema version 5 while preserving validation of schema versions 1–4.

- [ ] **Step 5: Run focused Rust tests and generate bindings**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema --test preferences_store`

Run: `corepack pnpm bindings:generate`

Expected: PASS and generated `src/shared/api/bindings.ts` includes both preference commands and types.

### Task 2: Settings subject palette and capture batch assignment

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/commands/capture_inbox.rs`
- Modify: `src-tauri/tests/capture_inbox_store.rs`

**Interfaces:**
- Consumes: `subjectPreferencesGet`, `subjectPreferencesSave`, and `SubjectPreferences` from Task 1.
- Produces: `capture_batch_assign_subject(batch_id, expected_revision, subject) -> AppResult<CaptureBatchDetail>`.

- [ ] **Step 1: Write failing Rust and Vue tests**

Assert that bulk assignment updates the batch, clears every draft override, increments one revision, and makes otherwise complete cards ready. Assert Settings can enable built-ins, add/remove custom subjects, toggle sound, and save. Assert the capture organizer shows enabled subjects near the top and one click applies the chosen value to the entire batch.

- [ ] **Step 2: Run focused tests to verify failure**

Run: `corepack pnpm exec vitest run src/app/views/SettingsView.test.ts src/app/views/CaptureView.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`

Expected: FAIL on the new behavior.

- [ ] **Step 3: Implement atomic batch subject assignment**

Inside one transaction, verify ownership, organizing state, and revision; normalize the selected subject; update `capture_batches.subject`; set every batch draft `subject_override = NULL`; touch the batch once; commit; return fresh detail.

- [ ] **Step 4: Implement Settings and organizer UI**

Add a subject-preference panel ahead of backup controls. Use checkbox chips for the nine built-ins, removable chips plus an input for custom values, and a sound switch. In the organizer, put the enabled subject chips below the workbench header with a clear `整批科目` label; clicking a chip calls the atomic command and updates all cards. Replace the bottom card subject text input with an in-card compact subject select while keeping tags and note in the lower inspector.

- [ ] **Step 5: Run focused tests**

Run: `corepack pnpm exec vitest run src/app/views/SettingsView.test.ts src/app/views/CaptureView.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`

Expected: PASS.

### Task 3: Role-aware drag, settle animation, and optional sound

**Files:**
- Create: `src/modules/capture/composables/useCaptureFeedback.ts`
- Create: `src/modules/capture/composables/useCaptureFeedback.test.ts`
- Modify: `src/modules/capture/composables/useCapturePointerDrag.ts`
- Modify: `src/modules/capture/composables/useCapturePointerDrag.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.test.ts`

**Interfaces:**
- Produces: pointer state `hoveredTarget: CaptureDropTarget | null` and `dropSequence: number`.
- Produces: `useCaptureFeedback(soundEnabled)` with `playDrop(role)` and `reducedMotion`.

- [ ] **Step 1: Write failing interaction tests**

Assert hover target tracking, cleanup on Escape/pointer cancellation, question/answer ghost classes, matching card-face highlight, one settle pulse after successful mutation, no audio when disabled, and no animation/audio when reduced motion is active.

- [ ] **Step 2: Run focused tests to verify failure**

Run: `corepack pnpm exec vitest run src/modules/capture/composables/useCapturePointerDrag.test.ts src/modules/capture/composables/useCaptureFeedback.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: FAIL on new state and feedback.

- [ ] **Step 3: Implement transform-only card feedback**

The loose card scales to `0.94` on lift, follows at 60 Hz through CSS transform, scales to `1.04` over a valid target, and fades to `0.82` opacity. Question uses ink-green; answer uses cinnabar. The target face scales to `1.012` with the matching translucent fill. After the persisted mutation succeeds, the target card runs one 240 ms settle keyframe and the moved item enters through the existing transition group.

- [ ] **Step 4: Implement local drop tone**

Lazily create `AudioContext` on the first successful user-driven drop. Play a 55 ms sine oscillator at 440 Hz for question and 554 Hz for answer through a low gain envelope. Do nothing when sound is disabled, reduced motion is requested, AudioContext is unavailable, or the mutation fails.

- [ ] **Step 5: Run focused tests**

Run: `corepack pnpm exec vitest run src/modules/capture/composables/useCapturePointerDrag.test.ts src/modules/capture/composables/useCaptureFeedback.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: PASS.

### Task 4: Quality gate and Windows acceptance

**Files:**
- Modify: `docs/windows-capture-acceptance.md`
- Modify: `src/shared/api/bindings.test.ts`

**Interfaces:**
- Consumes all tasks above.
- Produces a reproducible acceptance checklist for subject configuration, whole-batch assignment, reduced motion, and sound opt-out.

- [ ] **Step 1: Update binding and acceptance assertions**

Document default nine subjects, custom subject persistence, cross-profile isolation, whole-batch apply, per-card override, role colors, sound setting, and reduced-motion behavior.

- [ ] **Step 2: Run all quality gates**

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Run: `corepack pnpm test`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Run: `corepack pnpm bindings:check`

Run: `corepack pnpm tauri build`

Expected: all commands exit 0; known OpenSSL static-PDB linker warnings may remain non-fatal.

- [ ] **Step 3: Review and commit**

Run: `git diff --check`

Commit message: `feat: add subject presets and card drop feedback`

