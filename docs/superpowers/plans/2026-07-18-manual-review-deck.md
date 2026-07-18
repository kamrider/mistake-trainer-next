# Manual Review Deck Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a learner select an ordered set of active problems in the library, start one persistent manual review session, and resume that exact deck after navigation or restart.

**Architecture:** Rust remains authoritative for validating the selected IDs and creating/resuming the single active profile session. The library starts a manual session through a typed command before navigating; the review page then resumes the persisted session without putting problem IDs in the URL. Vue owns only transient selection and animation state.

**Tech Stack:** Rust, rusqlite/SQLCipher, serde, specta/tauri-specta, Vue 3, Vue Router, TypeScript strict, Vitest, Testing Library, Lucide, CSS transitions.

## Global Constraints

- Windows is the only v1 release platform.
- Manual decks contain 1 through 100 unique active problems from the current account and profile.
- Selected problem order is preserved exactly; foreign, archived, trashed, missing, duplicate, or oversized selections fail atomically.
- Invalid manual selection must not cancel or modify an existing active session.
- Opening the review page without a new selection resumes any active due or manual session; only when none exists may it create a new due session.
- Problem IDs never enter the route query, browser history, logs, or arbitrary Vue global state.
- Leaving the review room keeps unfinished progress resumable.
- Motion uses transform and opacity, follows existing 120/180/240 ms tokens, and honors `prefers-reduced-motion`.
- No double-click interaction and no GitHub push are authorized by this plan.

---

### Task 1: Strict persistent manual-session use case

**Files:**
- Modify: `src-tauri/src/modules/review.rs`
- Modify: `src-tauri/tests/review_store.rs`

**Interfaces:**
- Produces: `StartManualReview { account_id, profile_id, problem_ids, now_utc_ms }`.
- Produces: `start_manual_review_queue(&mut Connection, StartManualReview) -> Result<ReviewQueueState, ReviewUseCaseError>`.
- Changes: `list_review_queue` resumes the current active session regardless of its mode before creating a due queue.

- [ ] **Step 1: Write failing integration tests for ordered manual decks**

Create three active problems, one archived problem, and one foreign-profile problem. Assert:

```rust
let started = start_manual_review_queue(
    &mut connection,
    StartManualReview {
        account_id: "account-1".into(),
        profile_id: profile.id.clone(),
        problem_ids: vec![third.id.clone(), first.id.clone(), second.id.clone()],
        now_utc_ms: 100,
    },
)?;
assert_eq!(ids(&started), vec![third.id, first.id, second.id]);
assert_eq!(started.mode, "manual");
assert!(!started.resumed);
```

Reopen through `list_review_queue` with no manual input and assert the same session ID, remaining order, `resumed == true`, original total, and stored completed count.

- [ ] **Step 2: Write failing atomic-rejection tests**

Start a valid due session, snapshot its row, then attempt empty, duplicate, 101-item, archived, missing, and foreign selections. Each call must return `InvalidManualSelection`, and the original session row must be byte-for-byte unchanged.

- [ ] **Step 3: Implement validation before session mutation**

Add:

```rust
pub struct StartManualReview {
    pub account_id: String,
    pub profile_id: String,
    pub problem_ids: Vec<String>,
    pub now_utc_ms: i64,
}
```

Reject lengths outside `1..=100` and duplicates before opening the mutation transaction. Inside one transaction, query each ID with account/profile/status=`active`, preserve caller order, and return `InvalidManualSelection` if the validated count differs. Only after validation succeeds, cancel the prior active session and insert the new `manual` session.

- [ ] **Step 4: Make ordinary queue opening resume either mode**

Remove request-matching cancellation from `list_review_queue`. If an active session exists, clean unavailable remaining items with `queue_entries_for_ids`, update the stored JSON/current index/status transactionally, and return its persisted mode. If no active session remains, create the normal due queue.

- [ ] **Step 5: Run the Rust integration test**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store
```

Expected: all review store tests pass, including ordered resume and atomic rejection.

### Task 2: Typed start command and public error contract

**Files:**
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts` (generated)
- Modify: `src/shared/api/bindings.test.ts`

**Interfaces:**
- Produces: `ReviewManualStartInput { problem_ids: Vec<String> }`.
- Produces: `review_manual_start(input) -> AppResult<ReviewQueueOverview>`.
- Preserves: `review_queue()` with no page-supplied entity IDs.

- [ ] **Step 1: Add a failing command-contract test**

Assert generated TypeScript contains both:

```ts
reviewManualStart: (input: ReviewManualStartInput) =>
  __TAURI_INVOKE<AppResult<ReviewQueueOverview>>("review_manual_start", { input })
reviewQueue: () => __TAURI_INVOKE<AppResult<ReviewQueueOverview>>("review_queue")
```

- [ ] **Step 2: Implement command mapping without leaking IDs**

Map `InvalidManualSelection` to:

```rust
AppResult::failure(
    "review_manual_selection_invalid",
    "所选题目已经变化，请回到题库重新选择后再试。",
    false,
    Uuid::now_v7().to_string(),
)
```

Database, scheduler, and lock failures remain retryable generic review errors. Public messages and serialized errors must not include selected IDs or internal SQL details.

- [ ] **Step 3: Register and regenerate bindings twice**

Register `commands::review::review_manual_start`, run `pnpm bindings:generate` twice, and compare SHA-256 of `src/shared/api/bindings.ts`.

- [ ] **Step 4: Run command and binding tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test review_store
pnpm vitest run src/shared/api/bindings.test.ts
```

Expected: all tests pass and the generated client is stable.

### Task 3: Library selection dock and start transition

**Files:**
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`
- Modify: `src/app/views/LibraryView.vue`
- Create: `src/app/views/LibraryView.test.ts`

**Interfaces:**
- Adds events: `trainSelection: []`, `selectAll: []`, and `clearSelection: []`.
- Adds props: `startingReview: boolean`.
- Consumes: `commands.reviewManualStart({ problemIds })`.

- [ ] **Step 1: Write selection-dock component tests**

For active problems, select two cards and assert the dock exposes `开始训练 2 道题`, `全选当前结果`, and `清空选择`. Archived and trashed tabs must not show the training action. A single click emits each event; no double-click event exists.

- [ ] **Step 2: Write the view orchestration tests**

Mock a successful `reviewManualStart` and assert it receives selected IDs in visible selection order, then routes to `{ name: 'review' }`. On failure, remain in the library, show the returned message, keep all selected checkboxes, and re-enable the start action.

- [ ] **Step 3: Implement the sticky selection dock**

Use a `Transition` around a bottom/sticky paper dock. Make training the visually primary action with `Play`, retain archive/trash as secondary actions, and add select-all/clear controls. Disable all mutating dock actions while `startingReview` is true; the primary label becomes `正在整理训练卡组…`.

- [ ] **Step 4: Add selected-card feedback**

Apply a selected class and `aria-selected` to each card. Animate only background, border, box-shadow, opacity, and `translateY(-2px)` over the existing motion tokens. Reduced-motion mode removes translation and dock entrance motion.

- [ ] **Step 5: Implement start-before-navigation orchestration**

In `LibraryView`, call `reviewManualStart` before routing. Do not clear selection until the command succeeds. If routing fails after session creation, report that the deck is safely saved and can be resumed from the training room.

- [ ] **Step 6: Run library tests and type checking**

Run:

```powershell
pnpm vitest run src/modules/library/components/LibraryWorkspace.test.ts src/app/views/LibraryView.test.ts
pnpm typecheck
```

Expected: component and orchestration tests pass.

### Task 4: Mode-aware review resume and completion experience

**Files:**
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReviewView.test.ts`
- Modify: `src/modules/review/components/ReviewRoom.vue`
- Modify: `src/modules/review/components/ReviewRoom.test.ts`
- Modify: `docs/architecture.md`
- Modify: `docs/plans/review.md`

**Interfaces:**
- `ReviewRoom` consumes `mode: 'due' | 'manual' | string`.
- `ReviewView` calls `commands.reviewQueue()` and renders the persisted mode.

- [ ] **Step 1: Write resume and mode-copy tests**

Assert a manual overview renders `自选卡组`, retains persisted progress, and uses manual completion copy. Assert a fresh due overview still renders the existing due behavior. The route contains no `problemId` or deck IDs.

- [ ] **Step 2: Remove entity IDs from review routing**

Delete `problemId` query parsing and call `commands.reviewQueue()` with no arguments. Update existing tests and generated bindings expectations accordingly.

- [ ] **Step 3: Add mode-aware room presentation**

Show `自选训练` for manual sessions and `到期复习` for due sessions. Keep the existing card turn, progress fill, rating transitions, keyboard shortcuts, and answer lightbox. Add no extra modal before training.

- [ ] **Step 4: Improve completion feedback**

For manual mode, say `这组自选卡已经练完。`; for due mode retain the daily-review message. Animate the completion seal and copy with opacity/transform only, and honor reduced motion.

- [ ] **Step 5: Document the session boundary**

Record that manual selection is validated and persisted by Rust before navigation, only one active session exists per profile, ordinary review entry resumes either mode, and leaving the room intentionally preserves unfinished progress.

- [ ] **Step 6: Run focused frontend tests**

Run:

```powershell
pnpm vitest run src/app/views/ReviewView.test.ts src/modules/review/components/ReviewRoom.test.ts
pnpm lint
pnpm typecheck
```

Expected: all tests and checks pass.

### Task 5: Full verification, visual QA, and local baseline

**Files:**
- Consumes all files from Tasks 1 through 4.
- Produces one clean local Git commit; it does not push.

- [ ] **Step 1: Run all quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Run targeted `rustfmt --check` on modified Rust files and `git diff --check`.

- [ ] **Step 2: Perform browser visual QA**

Use explicit development-only fixtures to inspect active-card selection, dock entrance, select-all/clear, busy state, manual-mode badge, answer turn, resume chip, completion state, keyboard order, responsive CSS, and reduced-motion rules. Production builds must not contain fake runtime data paths.

- [ ] **Step 3: Review the full diff**

Confirm no selected IDs enter route strings, logs, diagnostics, or generated filenames; invalid manual input cannot mutate the old session; and no capture/export code changed.

- [ ] **Step 4: Commit the local baseline**

```powershell
git commit -m "feat: add persistent manual review decks"
```

Report the exact local SHA and keep the worktree clean. Do not push without a new exact-SHA authorization.
