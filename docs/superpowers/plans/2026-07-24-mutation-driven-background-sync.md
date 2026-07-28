# Mutation-Driven Background Sync Implementation Plan

> **For agentic workers:** Execute this plan in small reviewable increments. Keep the
> encrypted local library as the source of truth and never make a successful local save
> depend on cloud availability.

**Goal:** After a cloud-visible local mutation commits, quietly schedule a bounded,
debounced sync so new work reaches another device without requiring a page change or a
manual button.

**Architecture:** Extend the app-owned `SyncController` with a mutation scheduler. The
controller coalesces quick edits, waits for any older sync to finish, and runs one more
pass when a mutation arrives during that sync. Feature views only notify the controller
after their typed command reports success. App state decides whether cloud sync is
eligible; local-only, signed-out, and offline sessions remain silent and retain their
outbox for the existing startup/online/manual recovery path.

**Tech Stack:** Vue 3 dependency injection, Vitest/Testing Library, existing Tauri typed
commands and Rust outbox.

## Global invariants

- A local command success remains success even when the later cloud attempt fails.
- No timer, polling loop, or retry loop runs for local-only, signed-out, or offline state.
- Mutation bursts produce one sync after a short quiet window.
- A mutation committed while another sync is in flight always receives a later pass; it
  must not be treated as covered by the older request.
- Manual, startup, online, and visible triggers continue to coalesce through the same
  single-flight command.
- Components notify only after a real mutation succeeds; reads, cancelled dialogs,
  validation failures, and local-only capture draft edits do not schedule sync.
- Timers are disposed when the app unmounts. Reduced-motion behavior is unchanged because
  the scheduler has no visual animation of its own.

---

### Task 1: Make the sync controller mutation-safe

**Files:**
- Modify: `src/app/sync-controller.ts`
- Modify: `src/app/sync-controller.test.ts`
- Modify: `src/app/App.vue`

**Contract:**

```ts
export interface SyncController {
  run(reason: SyncTrigger): Promise<AppResult<SyncNowReport>>
  scheduleMutation(): void
  dispose(): void
}
```

- Add `mutation` to `SyncTrigger`.
- Accept a bounded debounce delay and `canScheduleMutation` predicate when constructing
  the controller.
- Keep a dirty flag separate from the single-flight promise.
- When the debounce expires, wait for an older in-flight request and then run a fresh
  mutation pass. If another mutation arrives during that pass, loop exactly once more
  after the quiet window; never spin on a failed result.
- `dispose()` clears timers and prevents later scheduled work.
- In `App.vue`, allow mutation scheduling only for connected-capable phases
  (`idle`, `syncing`, `synced`, `deferred_capture`, `retry_waiting`). Offline recovery
  remains owned by the `online` event.

**Tests:**

1. Several quick mutations result in one command.
2. A mutation during an in-flight manual/startup request causes a fresh second command.
3. A mutation during the mutation pass causes one later pass.
4. An ineligible predicate and `dispose()` prevent invocation.
5. Existing direct-trigger coalescing remains unchanged.

---

### Task 2: Notify after the primary learning mutations

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReportView.vue`
- Modify the corresponding view tests.

**Success boundaries:**

- Profile create, rename, and delete schedule after the new overview is accepted.
  Profile selection is navigation, not a cloud-visible mutation.
- Capture schedules only after `capture_commit_ready` commits one or more formal
  problems. Draft layout, crop, staging, and LAN upload remain local-only.
- Library update and status change schedule after the returned problem/list state is
  applied.
- Review schedules after a rating transaction succeeds. Multiple answers in one study
  burst debounce naturally.
- Export snapshot create, trash, and restore schedule after success. File generation is
  local artifact work and does not schedule.

Each view injects the controller optionally so isolated component tests and browser design
preview remain command-safe. Add at least one positive and one failure/cancellation
assertion for every feature family.

---

### Task 3: Cover migration and conflict decisions

**Files:**
- Modify: `src/modules/legacy/components/LegacyImportPanel.vue`
- Modify: `src/modules/sync/components/SyncConflictCenter.vue`
- Modify their tests.

- Schedule once after an atomic legacy import and once after a successful rollback.
- Schedule after a conflict field or entity decision succeeds because retaining the local
  side can enqueue a replacement outbox operation.
- Do not schedule after reads, failed commands, or focus restoration.

---

### Task 4: Verification and checkpoint

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
git diff --check
```

Visible acceptance:

1. A connected save briefly changes the shell status to “正在安全同步” and settles at
   “本地与云端已同步”.
2. Several quick ratings do not flicker or issue one request per click.
3. Local-only and offline saves remain calm and usable.
4. Starting a phone capture is never interrupted by a scheduled sync; committing the
   organized batch after capture ends schedules normally.
5. No horizontal overflow or motion regression appears at desktop or compact width.

Commit as one controller increment and one feature-wiring increment, then keep the branch
local unless the user explicitly authorizes a push.
