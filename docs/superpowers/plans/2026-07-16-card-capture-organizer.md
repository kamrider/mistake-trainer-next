# Capture Card Fusion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the two-column capture organizer into a persistent card-fusion workflow where one click marks a loose image as question or answer, dragging to the right creates or merges a study card, and clicking a card flips between readable question and answer faces.

**Architecture:** Persist the loose-image role on `capture_items`, and add one atomic Rust use case that either creates a draft or merges items into an existing draft in one revision. The Vue layer uses Pointer Events for internal dragging so WebView2 never depends on native HTML5 drag behavior; external file drop remains native. Every successful organizer mutation replaces the current batch detail, providing auto-save and an explicit saved state.

**Tech Stack:** Vue 3, TypeScript strict, Pointer Events, Vitest/Testing Library, Tauri 2, Rust, rusqlite/SQLCipher.

## Global Constraints

- Do not add or retain any double-click gesture.
- One left-image click toggles `question | answer`; question uses ink green and answer uses cinnabar.
- Dropping on the empty right lane creates a draft and assigns the dragged image atomically.
- Dropping on an existing card merges the image into that card using its persisted staged role.
- A card supports multiple ordered images on both faces and can return an image to the left without deleting it.
- Card flip lasts 240 ms, animates only `transform` and `opacity`, and becomes instant under `prefers-reduced-motion`.
- Internal dragging must use Pointer Events; native HTML5 drag remains only for files entering the application.
- Every mutation is persisted immediately; the bottom action only commits complete cards to the formal library.
- Preserve LAN upload, encrypted assets, mobile filename containment, 960 px derived previews, and atomic ready-draft commit.

---

### Task 1: Persist loose-image roles and atomically create or merge cards

**Files:**
- Create: `src-tauri/migrations/0004_capture_staged_roles.sql`
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/commands/capture_inbox.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/tests/capture_inbox_store.rs`
- Modify: `src-tauri/tests/backup_store.rs`

**Interfaces:**
- Produce `CaptureItemSummary.staged_role: "question" | "answer"`.
- Produce `capture_item_stage_role(batch_id, expected_revision, item_id, staged_role) -> AppResult<CaptureBatchDetail>`.
- Produce `capture_card_merge(input: CaptureCardMergeInput) -> AppResult<CaptureBatchDetail>`, where `target_draft_id = null` creates one draft and all item IDs are moved in one transaction.

- [ ] Add failing v3-to-v4 migration, persistence, atomic-create, merge, revision-conflict, and backup-restore tests.
- [ ] Run the focused Rust tests and confirm they fail before implementation.
- [ ] Add the migration, DTO fields, transactional use cases, command wrappers, error mapping, and binding registration.
- [ ] Run the focused Rust tests and confirm they pass.

### Task 2: Replace internal native dragging with Pointer Events

**Files:**
- Create: `src/modules/capture/composables/useCapturePointerDrag.ts`
- Create: `src/modules/capture/composables/useCapturePointerDrag.test.ts`
- Modify: `src/modules/capture/components/CaptureThumbnail.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Produce pointer-drag state after a 6 px threshold, a transform-only drag ghost, registered `new-card | card | unassigned` drop zones, Escape cancellation, and pointer-capture cleanup.
- Emit `stageItemRole`, `mergeCard`, and `moveItem` without any `dblclick` handler.

- [ ] Add failing tests for click role toggle, no double-click behavior, blank-lane creation, existing-card merge, unassign drop, cancellation, and no mutation before drop.
- [ ] Run the focused Vue tests and confirm they fail before implementation.
- [ ] Implement the composable and wire thumbnails, workspace drop zones, selected styling, and saved-state feedback.
- [ ] Run the focused Vue tests and confirm they pass.

### Task 3: Deliver the game-like multi-image question card

**Files:**
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`

**Interfaces:**
- A body click selects and flips the card; a dragged image can land on either face/card.
- Each face shows one readable 960 px preview plus an ordered filmstrip for additional images.
- Metadata is edited once in the selected-card inspector, not repeated on every card.

- [ ] Add failing tests for true front/back flip, 240 ms motion token, multiple-image filmstrip, reduced motion, drop target, and selected inspector.
- [ ] Run focused component tests and confirm they fail before implementation.
- [ ] Implement the 3D `rotateY(180deg)` card, simplify per-image controls, and move fields to the selected-card inspector.
- [ ] Run focused component tests and confirm they pass.

### Task 4: Fix formal commit and auto-save acceptance

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Every organizer command returns and installs the latest `CaptureBatchDetail`.
- The UI shows `保存中 / 已自动保存 / 保存失败` independently of formal library commit.
- `capture_commit_ready` reports committed and remaining counts; zero ready cards cannot look successful.

- [ ] Add failing tests for latest-revision chaining, persistence after reload, zero-ready feedback, successful commit, and failure recovery.
- [ ] Run focused tests and confirm they fail before implementation.
- [ ] Implement mutation serialization, save-state reporting, and precise commit feedback.
- [ ] Run focused tests and confirm they pass.

### Task 5: Regression gates and delivery

**Files:**
- Modify generated `src/shared/api/bindings.ts` through the existing binding generator.
- Modify `src/shared/api/bindings.test.ts`.

- [ ] Regenerate TypeScript bindings and run the drift check.
- [ ] Run `corepack pnpm lint`, `corepack pnpm typecheck`, `corepack pnpm test`, and `corepack pnpm build`.
- [ ] Run `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets` and `corepack pnpm tauri build`.
- [ ] Review the diff for unrelated changes, then commit and push `feature/capture-library`.
