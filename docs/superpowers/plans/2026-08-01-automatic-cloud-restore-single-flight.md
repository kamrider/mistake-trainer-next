# Automatic Cloud Restore Single Flight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Coalesce overlapping startup, online, and visibility-triggered cloud session restoration into one authoritative restore-and-sync transaction.

**Architecture:** Keep the existing `syncController` responsible for native `syncNow` serialization. Add a separate root-level Promise latch around the complete `authRestore → phase decision → syncController.run` chain so authentication results cannot race, and release that latch in `finally` so later automatic triggers still work.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue

## Global Constraints

- Do not modify Rust files or the existing `src-tauri/src/infrastructure/recognition_visual_split.rs` change.
- Do not implement launch-gate work for licensing, privacy, support operations, account deletion, device migration, update recovery, or SLA.
- Startup, online, and visibility triggers that overlap must share one `authRestore → sync` transaction.
- A coalesced trigger must not invoke `authRestore` or `syncNow` again while the older automatic transaction is pending.
- The transaction latch must clear after success, handled failure, or thrown failure so a later trigger can retry.
- Existing visibility cooldown, offline behavior, authentication status mapping, sync result handling, and post-sync profile refresh must remain unchanged.
- Do not stage or commit the existing dirty worktree.

---

### Task 1: Automatic Restore Transaction

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Consumes: existing `restoreCloudAndSync(reason: SyncTrigger)` trigger sites and `syncController.run(reason)`.
- Produces: `restoreCloudAndSync(reason): Promise<void>` that returns the same pending Promise to overlapping callers and starts a fresh transaction after the pending one settles.

- [x] **Step 1: Strengthen the existing in-flight startup-sync test**

Change the current network-recovery test so an `online` event during pending startup sync expects one `authRestore` call and one `syncNow` call. After resolving startup sync, dispatch another `online` event and expect a second authentication restoration and a second sync, proving the latch releases.

- [x] **Step 2: Run the App profile test and verify failure**

Run: `npm test -- --run src/app/App.profile.test.ts`

Expected: FAIL because the current implementation calls `authRestore` twice while startup sync is still pending.

- [x] **Step 3: Implement the complete-chain Promise latch**

Rename the current implementation body to `runCloudRestoreAndSync(reason)`. Add `let cloudRestoreTask: Promise<void> | undefined`; have `restoreCloudAndSync(reason)` return the existing task when present, otherwise assign `runCloudRestoreAndSync(reason).finally(...)` before returning it. Clear only the matching task in `finally`.

- [x] **Step 4: Run application synchronization tests**

Run: `npm test -- --run src/app/App.profile.test.ts src/app/sync-controller.test.ts src/app/App.test.ts`

Expected: every focused test passes.

### Task 2: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-automatic-cloud-restore-single-flight.md`

**Interfaces:**
- Consumes: completed automatic restore transaction.
- Produces: checked task boxes and exact verification evidence.

- [x] **Step 1: Run static checks**

Run: `npm run typecheck`

Run: `npm run lint`

Expected: both commands exit 0.

- [x] **Step 2: Run the complete frontend suite**

Run: `npm test -- --run`

Expected: every test file and test passes.

- [x] **Step 3: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust file was edited during this task.

- [x] **Step 4: Record verification without committing**

Check every completed task box and append the exact red/green test, typecheck, lint, complete-suite, hygiene, index, and scope results. Do not run `git add` or `git commit`.

## Verification Record

- Red phase: the strengthened startup-sync test observed 2 `authRestore` calls while the first automatic transaction was pending.
- Focused green phase: 3 test files passed, 31 tests passed; the pending trigger was coalesced and a post-settlement trigger started a fresh restore and sync.
- Typecheck: `npm run typecheck` exited 0.
- Lint: `npm run lint` exited 0 with zero warnings after simplifying the prior initialization task cleanup to its single-owner invariant.
- Complete frontend suite: 100 test files passed, 624 tests passed.
- Patch hygiene: `git diff --check` exited 0; existing LF-to-CRLF notices were warnings only, and the scoped trailing-whitespace scan returned no matches.
- Git index: `git diff --cached --name-only` returned no files.
- Scope: this task changed `App.vue`, its profile integration test, this plan, and simplified the prior lifecycle controller's single-owner task cleanup to satisfy the full lint gate; no Rust file was edited, and the pre-existing `recognition_visual_split.rs` modification was left untouched.
