# Crash-safe Exam Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real, restart-safe exam workflow in which a learner answers an entire selected deck without seeing answers, then enters a separate grading pass that records FSRS events and an accurate result summary.

**Architecture:** Keep `review_sessions.mode` as the queue source (`due | manual`) and add an orthogonal persisted `experience` (`review | exam`) plus exam phase, navigation position, and score counters. Rust owns every state transition and rejects grading before the exam enters its grading phase; Vue only renders the returned state and sends opaque positions or ratings.

**Tech Stack:** SQLite/SQLCipher migrations, Rust + rusqlite + Specta, Tauri 2, Vue 3 + TypeScript, Vitest/Testing Library, CSS transitions using existing motion tokens.

## Global Constraints

- Windows-first and fully offline; exam start, navigation, grading, and resume must not depend on Supabase.
- Vue receives no database handles, filesystem paths, or problem IDs in routes.
- A profile still has at most one active review session; starting an exam atomically cancels the previous active session.
- One exam accepts 1 through 100 unique active problems in caller order.
- Question phase never exposes answer assets. Rating is rejected in Rust until grading begins.
- Grading uses the existing transactional `ReviewEvent + ScheduleState + outbox + session advance` path.
- Motion uses `transform` and `opacity`, obeys `prefers-reduced-motion`, and preserves keyboard/focus access.

---

### Task 1: Persist the exam state machine

**Files:**
- Create: `src-tauri/migrations/0007_review_exam.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Test: `src-tauri/tests/database_schema.rs`

**Interfaces:**
- Produces these additive `review_sessions` columns: `experience`, `exam_phase`, `exam_question_index`, `exam_correct_count`, `exam_wrong_count`.
- Existing sessions migrate to `experience='review'`, with nullable `exam_phase` and zeroed counters.

- [x] **Step 1: Write a failing v6-to-v7 migration test**

Create a v6 session, run migrations, and assert its source mode and progress survive while the five new columns receive safe defaults. Also insert an `experience='exam'` row and prove the phase/position constraints reject invalid values.

- [x] **Step 2: Run the schema test and verify it fails**

Run: `.\\scripts\\cargo-msvc.cmd test --manifest-path src-tauri\\Cargo.toml --test database_schema`

Expected: FAIL because schema version 7 and the exam columns do not exist.

- [x] **Step 3: Add the additive migration and wire every upgrade path**

```sql
ALTER TABLE review_sessions
ADD COLUMN experience TEXT NOT NULL DEFAULT 'review'
CHECK(experience IN ('review', 'exam'));
ALTER TABLE review_sessions
ADD COLUMN exam_phase TEXT
CHECK(exam_phase IS NULL OR exam_phase IN ('answering', 'grading'));
ALTER TABLE review_sessions
ADD COLUMN exam_question_index INTEGER NOT NULL DEFAULT 0
CHECK(exam_question_index >= 0);
ALTER TABLE review_sessions
ADD COLUMN exam_correct_count INTEGER NOT NULL DEFAULT 0
CHECK(exam_correct_count >= 0);
ALTER TABLE review_sessions
ADD COLUMN exam_wrong_count INTEGER NOT NULL DEFAULT 0
CHECK(exam_wrong_count >= 0);
```

Set `user_version=7` from versions 0 through 6. Raise backup `CURRENT_SCHEMA_VERSION` to 7; no backup table-set rule changes because the migration is additive.

- [x] **Step 4: Run the schema and backup tests**

Run both `database_schema` and `backup_store`; expect all tests to pass.

### Task 2: Implement Rust exam transitions and contracts

**Files:**
- Modify: `src-tauri/src/modules/review.rs`
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/review_store.rs`
- Test: `src-tauri/tests/command_contract.rs`

**Interfaces:**
- Produces `start_exam_review_queue(connection, StartExamReview) -> ReviewQueueState`.
- Produces `navigate_exam(connection, NavigateExam) -> ReviewQueueState`.
- Produces `begin_exam_grading(connection, BeginExamGrading) -> ReviewQueueState`.
- Adds typed commands `review_exam_start({ problemIds })`, `review_exam_navigate({ position })`, and `review_exam_begin_grading()`.
- Extends `ReviewQueueOverview` with `examPhase`, `examQuestionIndex`, `examCorrectCount`, and `examWrongCount`.

- [x] **Step 1: Write failing domain-store tests**

Cover ordered exam creation, replacement of a due/manual session, navigation persistence, restart resume, out-of-range navigation rejection, rating rejection during `answering`, transition to `grading`, transactional score increments, and completion.

- [x] **Step 2: Run the focused review store test and verify it fails**

Run: `.\\scripts\\cargo-msvc.cmd test --manifest-path src-tauri\\Cargo.toml --test review_store`

Expected: FAIL because the exam functions and state fields are absent.

- [x] **Step 3: Implement a shared validated selected-deck path**

Reuse the current 1–100, uniqueness, ownership, profile, status, and caller-order validation for manual and exam starts. Insert exam sessions with source `mode='manual'`, `experience='exam'`, `exam_phase='answering'`.

- [x] **Step 4: Implement fail-closed exam transitions**

Navigation may only mutate the active answering exam and must validate `0 <= position < problem_count`. Beginning grading atomically changes `exam_phase` and resets both indices. `submit_review` must include a phase guard; `Again/Hard` increment wrong and `Good/Easy` increment correct in the same transaction as the event, schedule, outbox, and session advance.

- [x] **Step 5: Expose typed commands and regenerate bindings**

Register the three commands in Specta, return stable invalid-state errors, run `pnpm bindings:generate`, and confirm Vue receives only opaque IDs already contained in the queue plus an integer position.

- [x] **Step 6: Run Rust review/command/binding tests**

Expect focused tests and `pnpm bindings:check` to pass.

### Task 3: Add the exam entry to the library deck dock

**Files:**
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Test: `src/app/views/LibraryView.test.ts`
- Test: `src/modules/library/components/LibraryWorkspace.test.ts`

**Interfaces:**
- Consumes `commands.reviewExamStart({ problemIds })`.
- Produces the `start-exam` workspace event and a distinct busy experience so only the selected action shows its spinner.

- [x] **Step 1: Write failing UI tests**

Select multiple active problems, click “模拟考试”, assert ordered IDs reach `reviewExamStart`, navigation occurs only after persistence succeeds, and a failed start leaves the selection intact with the Rust message visible.

- [x] **Step 2: Implement the second deck action**

Place “开始训练” and “模拟考试” together as the primary deck choices. Use `Play` for review and `ClipboardCheck` for exam; keep archive/trash visually secondary. Preserve the 100-item limit and do not add modal configuration.

- [x] **Step 3: Add polished dock feedback**

Animate the chosen action with the existing feedback/page tokens, disable conflicting actions while persisting, and remove all motion under reduced-motion preference.

- [x] **Step 4: Run the library tests**

Run the two focused Vitest files and expect them to pass.

### Task 4: Build the two-phase exam room

**Files:**
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/modules/review/components/ReviewRoom.vue`
- Test: `src/app/views/ReviewView.test.ts`
- Test: `src/modules/review/components/ReviewRoom.test.ts`

**Interfaces:**
- Consumes the four exam overview fields and three exam commands.
- `ReviewRoom` accepts `examPhase` and emits `previous`, `next`, and `beginGrading` in addition to existing events.

- [x] **Step 1: Write failing component tests for answer secrecy**

In `answering`, assert answer media and rating controls are absent, previous/next buttons emit correctly, keyboard Left/Right works, and the last question exposes “开始核对答案”. In `grading`, assert the answer is visible immediately and only simple “答错/答对” controls are shown.

- [x] **Step 2: Write failing view integration tests**

Assert a resumed exam restores its persisted question position, navigation persists before loading another problem, entering grading resets to the first problem, rating calls the existing command, and final copy reports correct count, wrong count, and accuracy.

- [x] **Step 3: Implement exam orchestration in `ReviewView`**

Derive the visible item from persisted exam phase/position. Never place IDs in the route. On any failed navigation or phase change, keep the current card and show a retryable floating error. Stop/restart the clock around committed transitions.

- [x] **Step 4: Implement card-deck motion in `ReviewRoom`**

Use directional transform/opacity transitions for previous/next question cards, a paper-turn transition when entering grading, and a restrained score-seal entrance at completion. Keep focus-visible outlines, keyboard equivalents, 44 px targets, and reduced-motion behavior.

- [x] **Step 5: Run focused review UI tests**

Run both review Vitest files and expect them to pass.

### Task 5: Document and verify the vertical slice

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/review.md`
- Create: `docs/windows-exam-acceptance.md`

**Interfaces:**
- Documents the orthogonal session source/experience model and manual Windows acceptance flow.

- [x] **Step 1: Document invariants and acceptance steps**

Include start from selected library cards, answer-hidden navigation, restart during both phases, grading, score accuracy, event/outbox effects, keyboard controls, and reduced-motion verification.

- [x] **Step 2: Run complete quality gates**

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `cargo test --all-targets`, `pnpm bindings:check`, and `pnpm tauri build`. Expect all commands to pass; record any pre-existing nonfatal Windows linker or SQLCipher lock warnings separately.

- [x] **Step 3: Perform visual QA**

Open development previews for library selection, exam answering, exam grading, and completion. Verify no horizontal overflow at 1280 px and 760 px, readable full-size question media, clear phase hierarchy, correct focus flow, and reduced-motion behavior.

- [x] **Step 4: Review the diff and create one local checkpoint commit**

Ensure only exam-mode files are staged, bindings are in sync, and commit with `feat: add crash-safe exam mode`. Do not push without explicit authorization for that exact commit.
