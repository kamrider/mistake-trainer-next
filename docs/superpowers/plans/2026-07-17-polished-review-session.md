# Polished Review Session Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing FSRS-backed review page into an accurate, resumable and polished card-training experience with timing, keyboard control, image inspection and motion accessibility.

**Architecture:** Rust remains authoritative for session identity, queue position and review persistence. Vue receives a typed queue overview, owns only transient presentation state, and records the elapsed recall time that is submitted with the rating. Media inspection and timing are split into focused components/composables so the review room stays testable.

**Tech Stack:** Rust, rusqlite/SQLCipher, tauri-specta, Vue 3, TypeScript strict, Vitest, Testing Library, Lucide, CSS transitions.

## Global Constraints

- Preserve the existing FSRS 6.6.1 event and outbox transaction contract.
- Vue must not access SQLite, arbitrary filesystem paths or asset keys.
- Ordinary motion animates only `transform` and `opacity`; honor `prefers-reduced-motion`.
- Keyboard controls must not hijack editable elements or modified shortcuts.
- Existing manual single-problem review through `?problemId=` must remain supported.
- No new runtime dependency is required for this slice.

---

### Task 1: Accurate resumable queue overview

**Files:**
- Modify: `src-tauri/src/modules/review.rs`
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/tests/review_store.rs`
- Modify: `src/shared/api/bindings.ts` (generated)

**Interfaces:**
- Produces: `ReviewQueueOverview { session_id, mode, resumed, completed_count, total_count, items }` in Rust and the generated camelCase TypeScript equivalent.
- Preserves: `review_queue(problem_id)` command name and `ReviewQueueItem` fields.

- [ ] **Step 1: Add failing store tests for new and resumed session metadata**

Assert that a fresh due session reports `resumed == false`, `completed_count == 0`, and the original total. Submit one rating, reopen the queue, and assert `resumed == true`, `completed_count == 1`, and the unchanged total.

- [ ] **Step 2: Run the focused Rust test and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store`

Expected: compilation fails because `ReviewQueueOverview` and its metadata do not exist.

- [ ] **Step 3: Return a queue overview from the use case**

Introduce a store-level overview that carries the session ID, mode, resume flag, persisted current index, persisted total and remaining queue entries. New sessions report `resumed: false`; reused active sessions report `resumed: true`. If cleanup removes archived/trashed problems, derive `completed_count` from the cleaned session position and keep `total_count >= completed_count + items.len()`.

- [ ] **Step 4: Map the overview through the typed Tauri command**

Change `review_queue_for` and `review_queue` to return `AppResult<ReviewQueueOverview>`, using JavaScript-safe number types and public camelCase fields. Keep errors stable and do not expose account/profile identifiers.

- [ ] **Step 5: Regenerate bindings and run focused Rust tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store`

Run: `pnpm bindings:generate`

Run: `pnpm vitest run src/shared/api/bindings.test.ts`

Expected: all pass and `src/shared/api/bindings.ts` contains `ReviewQueueOverview`.

### Task 2: Deterministic recall clock

**Files:**
- Create: `src/modules/review/composables/useReviewClock.ts`
- Create: `src/modules/review/composables/useReviewClock.test.ts`

**Interfaces:**
- Produces: `useReviewClock(options)` with `elapsedMs`, `displayText`, `expired`, `running`, `start()`, `stop()`, and `reset(limitSeconds?)`.
- Consumes: optional positive integer `limitSeconds`; clock uses `performance.now()` and a 250 ms display update.

- [ ] **Step 1: Write failing fake-timer tests**

Cover elapsed display, countdown display, automatic expiration, stop freezing the submitted duration, and reset between cards.

- [ ] **Step 2: Run the focused frontend test and verify failure**

Run: `pnpm vitest run src/modules/review/composables/useReviewClock.test.ts`

Expected: module-not-found failure.

- [ ] **Step 3: Implement the composable**

Use monotonic `performance.now()` for duration and an interval only to refresh the display. Clamp duration to `[0, 86_400_000]`; expiration is presentation feedback only and must never auto-submit a rating.

- [ ] **Step 4: Run the focused test**

Run: `pnpm vitest run src/modules/review/composables/useReviewClock.test.ts`

Expected: all clock tests pass.

### Task 3: Accessible image inspection

**Files:**
- Create: `src/modules/review/components/ReviewMediaLightbox.vue`
- Create: `src/modules/review/components/ReviewMediaLightbox.test.ts`

**Interfaces:**
- Props: `images: string[]`, `initialIndex: number`, `label: string`.
- Emits: `close`.
- Keyboard: `Escape` closes; left/right arrows navigate; focus is trapped inside the modal and returned by the parent trigger after close.

- [ ] **Step 1: Write failing lightbox interaction tests**

Cover dialog semantics, image counter, previous/next buttons, arrow keys, Escape and single-image disabled navigation.

- [ ] **Step 2: Run the focused test and verify failure**

Run: `pnpm vitest run src/modules/review/components/ReviewMediaLightbox.test.ts`

Expected: component-not-found failure.

- [ ] **Step 3: Implement the lightbox**

Render the original decrypted data URL with `object-fit: contain`, a paper-ink backdrop, explicit close/previous/next controls, and opacity/scale transitions. Lock document scroll while open and restore it on unmount.

- [ ] **Step 4: Run the focused test**

Run: `pnpm vitest run src/modules/review/components/ReviewMediaLightbox.test.ts`

Expected: all lightbox tests pass.

### Task 4: Polished review card interaction

**Files:**
- Modify: `src/modules/review/components/ReviewRoom.vue`
- Modify: `src/modules/review/components/ReviewRoom.test.ts`
- Modify: `src/app/views/ReviewView.vue`
- Create: `src/app/views/ReviewView.test.ts`
- Modify: `src-tauri/src/modules/problems.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/tests/problem_detail.rs`
- Modify: `src-tauri/tests/problem_lifecycle.rs`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/app/views/LibraryView.vue`

**Interfaces:**
- `ReviewRoom` gains `elapsedText`, `expired`, `resumed`, and `timeLimitSeconds` presentation props plus `reveal` and `inspect` behavior kept internal.
- `ReviewView` consumes the generated `ReviewQueueOverview`, calculates `current = completedCount + localIndex + 1`, and submits the stopped monotonic duration.

- [ ] **Step 1: Add failing component tests**

Cover Space/Enter reveal, 1/2 and A/S/D/F ratings after reveal, editable-target shortcut guards, timer state, resume notice, image-lightbox opening, and reduced-motion-safe transition classes.

- [ ] **Step 2: Add failing view orchestration tests**

Mock `reviewQueue`, `problemDetail`, and `reviewSubmit`. Verify resumed progress uses persisted counts, retry reloads a failed queue, a rating advances exactly once, and the submitted duration is bounded.

- [ ] **Step 3: Run the focused frontend tests and verify failure**

Run: `pnpm vitest run src/modules/review/components/ReviewRoom.test.ts src/app/views/ReviewView.test.ts`

Expected: failures for the new queue envelope and interactions.

- [ ] **Step 4: Redesign the review room**

Use a keyed card-stage transition: question remains the stable front, answer enters as a secondary sheet with a short paper-turn transform/opacity motion. Add a visible clock chip, resume chip, keyboard legend, focused button states and large-image affordances. Keep multiple question/answer images in order. Do not use continuous animation, glow or floating decoration.

- [ ] **Step 5: Persist the existing per-problem time limit**

Expose `time_limit_seconds` as `timeLimitSeconds: number | null` through `ProblemDetail` and `ProblemUpdateInput`. Validate the range `1..=86400`, include it in the same problem/outbox transaction, and add the optional numeric field to the library detail editor. Empty input means no limit; invalid input blocks save with an inline error.

- [ ] **Step 6: Integrate queue metadata and clock in the view**

Replace the raw array response with the overview. Reset/start the clock when a problem loads, stop on reveal, submit the frozen duration, expose an inline retry action, and preserve the stored progress denominator across restart. Clear stale errors after a successful retry or item load.

- [ ] **Step 7: Run focused frontend tests**

Run: `pnpm vitest run src/modules/review src/app/views/ReviewView.test.ts`

Expected: all pass.

### Task 5: Verification, visual QA and baseline

**Files:**
- Modify only if verification reveals a defect: review files from Tasks 1-4.

**Interfaces:**
- No new public interface beyond Tasks 1-4.

- [ ] **Step 1: Run all static and unit gates**

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm test`

Run: `pnpm build`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Expected: all commands exit 0.

- [ ] **Step 2: Run binding drift and formatting checks**

Run: `pnpm bindings:generate` twice and compare `Get-FileHash src/shared/api/bindings.ts` before and after the second generation.

Run: `pnpm vitest run src/shared/api/bindings.test.ts`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check`

Run: `git diff --check`

Expected: all exit 0; line-ending notices are informational only.

- [ ] **Step 3: Perform browser visual QA**

Open the local Vue preview at 1280x720 and a narrow viewport. Inspect question, answer, advanced rating and lightbox states. Verify no clipped controls, console errors or horizontal overflow, and confirm reduced-motion CSS removes transforms/transitions.

- [ ] **Step 4: Review the diff for security and regressions**

Confirm no account/profile IDs, asset keys, file paths or arbitrary HTML enter the new UI. Confirm only a successful rating advances the queue and that a failed submission leaves the same card visible.

- [ ] **Step 5: Create one local Git baseline**

Run: `git add -A`

Run: `git commit -m "feat: polish resumable review sessions"`

Expected: a local commit on `feature/capture-library`; do not push without exact authorization for the new SHA.
