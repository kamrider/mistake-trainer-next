# Capture Card Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make newly created capture cards reversible, let assigned images switch between question and answer in place, inherit the current subject, and explain every incomplete state.

**Architecture:** Keep all destructive-looking card operations reversible: deleting a draft removes only the draft/link rows and returns its `capture_items` to the unassigned pool. Reuse the existing revision-checked `capture_item_move` transaction for in-card role changes. Extend card creation with an explicit inherited subject and keep readiness derived from persisted subject/question/answer state.

**Tech Stack:** Vue 3, TypeScript strict, Vitest/Testing Library, Tauri 2 typed commands, Rust, rusqlite/SQLCipher.

## Global Constraints

- Do not add double-click behavior.
- Deleting a capture card must not delete its images or encrypted assets.
- Every mutation must remain revision-checked and atomic.
- A ready card requires a non-empty subject, at least one question image, and at least one answer image.
- Existing v4 databases require no schema migration.

---

### Task 1: Reversible draft deletion and subject inheritance

**Files:**
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/commands/capture_inbox.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/capture_inbox_store.rs`
- Test: `src/shared/api/bindings.test.ts`
- Regenerate: `src/shared/api/bindings.ts`

**Interfaces:**
- Consumes: `MergeCaptureCard`, `CaptureBatchDetail`, optimistic `expected_revision`.
- Produces: `MergeCaptureCard.new_draft_subject: Option<String>` and `delete_capture_draft(connection, account_id, profile_id, batch_id, draft_id, expected_revision, now_utc_ms)` exposed as `capture_draft_delete`.

- [ ] **Step 1: Write failing Rust tests**

Add tests proving that a newly merged card inherits the supplied subject and becomes ready after one question plus one answer, and that deleting a card returns all linked items to `unassigned_item_ids` without deleting `capture_items` or `assets`.

- [ ] **Step 2: Run the focused Rust test and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`

Expected: FAIL because `new_draft_subject` and `delete_capture_draft` are not implemented.

- [ ] **Step 3: Implement the minimal revision-checked transactions**

When `target_draft_id` is `None`, normalize `new_draft_subject`; store it as `subject_override` only when it is non-empty and differs from the batch subject. For delete, verify ownership/state/revision, delete only the draft row, compact later draft positions, touch the batch once, commit, and return fresh detail.

- [ ] **Step 4: Expose and regenerate the typed command contract**

Add `capture_draft_delete` to the Tauri/Specta command list and regenerate `src/shared/api/bindings.ts`. Assert that the generated client contains the command and `newDraftSubject`.

- [ ] **Step 5: Run focused backend and binding tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`

Run: `corepack pnpm test -- src/shared/api/bindings.test.ts`

Expected: PASS.

### Task 2: In-card correction and clear incomplete reasons

**Files:**
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/app/views/CaptureView.vue`
- Test: `src/modules/capture/components/CaptureDraftCard.test.ts`
- Test: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Test: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Consumes: `commands.captureItemMove`, `commands.captureDraftDelete`, `CaptureDraftSummary`.
- Produces: `changeItemRole(itemId, draftId, targetRole, targetPosition)` and `deleteDraft(draftId)` UI events.

- [ ] **Step 1: Write failing component tests**

Assert that a card with two question images displays `缺答案`, a blank subject displays `缺科目`, the active question image offers `转为答案`, and `撤销这张卡` emits the delete event only after confirmation in the workspace.

- [ ] **Step 2: Run focused frontend tests and verify failure**

Run: `corepack pnpm test -- src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Expected: FAIL because the correction controls and command handler do not exist.

- [ ] **Step 3: Implement in-card role switching**

Add a visible `转为答案`/`转为题面` action beside `移回`. Emit `moveItem` with the same draft, opposite role, and append position. Keep click and drag behavior separate; do not introduce double-click.

- [ ] **Step 4: Implement reversible card deletion**

Add `撤销这张卡` to the card header. The workspace confirmation must say that all images return to the left material pool. The view calls `captureDraftDelete`, installs returned detail, and reports auto-save state.

- [ ] **Step 5: Inherit subject and explain readiness**

On a new-card drop pass `selectedDraft.subject || batch.subject` as `newDraftSubject`. Replace the generic `未完成` chip with deterministic missing labels built from subject/question/answer state, for example `缺答案 · 缺科目`.

- [ ] **Step 6: Run focused frontend tests**

Run: `corepack pnpm test -- src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Expected: PASS.

### Task 3: Regression gate and delivery

**Files:**
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Consumes: completed Tasks 1-2.
- Produces: repeatable Windows acceptance steps for undo, role correction, subject inheritance, and ready-state feedback.

- [ ] **Step 1: Update manual acceptance steps**

Document: create a card, switch one assigned image to answer, verify ready, undo the card, verify both images return left, then recreate and confirm the previous subject is inherited.

- [ ] **Step 2: Run the complete quality gate**

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Run: `corepack pnpm test`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Run: `corepack pnpm build`

Expected: all commands PASS.

- [ ] **Step 3: Review the final diff**

Run: `git diff --check`

Run: `rg -n "dblclick|double.?click|双击" src src-tauri -g "!target/**"`

Expected: no product double-click behavior and no whitespace errors.

- [ ] **Step 4: Commit and push**

Run: `git add docs src src-tauri`

Run: `git commit -m "fix: make capture cards reversible"`

Run: `git push origin feature/capture-library`

