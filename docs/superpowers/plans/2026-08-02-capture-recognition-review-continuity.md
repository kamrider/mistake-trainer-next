# Capture Recognition Review Continuity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep recognition review on the user's current category and suggestion while same-job optimistic/server updates arrive, while still resetting cleanly for a genuinely new recognition job.

**Architecture:** Extract category, cursor, and local decision reconciliation from the 799-line review component into `useCaptureRecognitionReviewSession`. The controller distinguishes same-job state synchronization from new-job initialization: same-job replacements preserve the current suggestion by ID and reconcile authoritative decisions, while a new job chooses the first non-empty category and starts at its first suggestion.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Testing Library Vue.

## Global Constraints

- Preserve the active filter and current suggestion across replacements of the same `CaptureRecognitionJob.id`.
- Preserve the current suggestion by ID rather than raw array index when suggestions reorder.
- Reconcile accepted/rejected local state from every authoritative or optimistically projected job replacement so failed saves can roll back truthfully.
- Reset filter, cursor, and decisions when the job ID changes.
- Initial category priority is `review`, then `high`, then `low`, then `stale`; an all-stale job must open the stale category instead of an empty low category.
- Preserve preview requests, keyboard shortcuts, bulk high-confidence acceptance, impact confirmation, focus handling, low-confidence safety, stale safety, and parent event payloads.
- Do not alter recognition algorithms, native/Rust recognition transactions, crop geometry, storage/device migration, updater recovery, account deletion, licensing, privacy, support, or launch gates.
- Preserve unrelated dirty-worktree changes, especially `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not stage or commit this dirty-worktree batch.

---

### Task 1: Characterize same-job review continuity

**Files:**

- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**

- Consumes: `CaptureRecognitionReview` props `job`, `previews`; emitted `review` event.
- Produces: regression evidence for same-ID job replacement and stale-only initialization.

- [x] **Step 1: Add the same-job replacement regression**

Create a job with two `review` suggestions, accept the first, assert the component advances to `2 / 2`, then rerender with the same job ID and the first suggestion authoritatively accepted:

```ts
const fixture = job()
fixture.suggestions = [suggestion('review-1', 'review'), suggestion('review-2', 'review')]
const view = render(CaptureRecognitionReview, { props: { job: fixture, previews: {} } })
await user.click(screen.getByRole('button', { name: '接受建议' }))
expect(screen.getByText('2 / 2')).toBeVisible()

const saved = structuredClone(fixture)
saved.suggestions[0]!.state = 'accepted'
await view.rerender({ job: saved })
expect(screen.getByText('2 / 2')).toBeVisible()
```

- [x] **Step 2: Add the stale-only initialization regression**

Render a new job containing only one stale suggestion and assert `已过期 1` is active with `aria-pressed="true"` and the stale action is visible.

- [x] **Step 3: Run the component suite red**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts
```

Expected: same-job rerender returns to `1 / 2`, and an all-stale job opens the empty low category.

### Task 2: Extract the review-session state boundary

**Files:**

- Create: `src/modules/ocr/composables/useCaptureRecognitionReviewSession.ts`
- Create: `src/modules/ocr/composables/useCaptureRecognitionReviewSession.test.ts`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`

**Interfaces:**

- Consumes: `job: () => CaptureRecognitionJob`.
- Produces: `filter`, `counts`, `filtered`, `current`, `currentIndex`, `acceptedIds`, `decisionState`, `recordDecision`, `recordAcceptedMany`, `move`, and `selectFilter`.

- [x] **Step 1: Test controller initialization and job identity**

Use a `ref<CaptureRecognitionJob>` to assert:

```ts
expect(session.filter.value).toBe('stale')
jobRef.value = sameJobWithReorderedSuggestions
expect(session.current.value?.id).toBe(previousCurrentId)
jobRef.value = differentJob
expect(session.currentIndex.value).toBe(0)
```

Also record an optimistic acceptance, replace the same job with an authoritative proposed state, and assert `decisionState` rolls back to `proposed`.

- [x] **Step 2: Implement pure category selection and filtering**

Define:

```ts
export type CaptureRecognitionReviewFilter = 'review' | 'high' | 'low' | 'stale'

function suggestionsFor(job: CaptureRecognitionJob, filter: CaptureRecognitionReviewFilter) {
  if (filter === 'stale') return job.suggestions.filter(item => item.state === 'stale')
  return job.suggestions.filter(item => item.state !== 'stale' && item.reviewBand === filter)
}
```

Choose the initial filter in the exact priority `review → high → low → stale`, falling back to `review` only when every category is empty.

- [x] **Step 3: Implement same-job synchronization**

On every job replacement, rebuild local accepted/rejected sets from suggestion states. For the same job ID, derive the currently displayed suggestion ID from the previous job/filter/index and find that ID in the new filtered list; otherwise clamp the prior index. For a different ID, choose the initial filter and reset index to zero.

- [x] **Step 4: Move session mutations out of the component**

Replace component-owned `filter`, `currentIndex`, `locallyAccepted`, `locallyRejected`, `counts`, `filtered`, `current`, `acceptedIds`, `resetReview`, `decisionState`, and raw set mutation with the controller API. Keep announcements and emitted payloads in the component because they are interaction/presentation responsibilities.

- [x] **Step 5: Run controller and component suites green**

Run:

```powershell
npm test -- --run src/modules/ocr/composables/useCaptureRecognitionReviewSession.test.ts src/modules/ocr/components/CaptureRecognitionReview.test.ts
npm run typecheck
```

Expected: both suites and TypeScript pass.

### Task 3: Verify parent workflow integration and commercial gates

**Files:**

- Modify: `docs/superpowers/plans/2026-08-02-capture-recognition-review-continuity.md`

**Interfaces:**

- Consumes: parent `useCaptureRecognitionWorkflow` optimistic projection and server rollback behavior.
- Produces: verification record and boundary review.

- [x] **Step 1: Run adjacent recognition suites**

Run:

```powershell
npm test -- --run src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/ocr/composables/useCaptureRecognitionReviewSession.test.ts src/modules/ocr/composables/useCaptureRecognitionWorkflow.test.ts src/app/views/CaptureView.test.ts
```

Expected: review UI, job queue, and parent view suites pass together.

- [x] **Step 2: Run final gates**

Run `npm run lint`, `npm run typecheck`, `npm run build`, then `npm test -- --run --maxWorkers=1`.

- [x] **Step 3: Review state ownership**

Confirm the component no longer deep-resets session state, new-job and same-job semantics are distinct, decision rollback follows authoritative props, current suggestion identity survives reordering, and no recognition algorithm or native boundary changed.

- [x] **Step 4: Check workspace hygiene**

Run targeted whitespace checks, verify the index is empty, verify `dist` remains ignored, and confirm the existing `recognition_visual_split.rs` modification remains untouched. Record baseline, red, green, final gates, local review, and hygiene below.

## Verification record

- Audit: `CaptureRecognitionReview` deep-watches the entire job and calls `resetReview`; `useCaptureRecognitionWorkflow.review()` synchronously projects a replacement job before persisting, proving same-job prop replacements occur on every decision.
- Baseline: review component, recognition workflow, and Capture view passed, 3 files / 52 tests.
- Red: the component suite failed in both intended assertions—same-job save returned from `2 / 2` to `1 / 2`, and a stale-only job left the stale category inactive. The new controller suite also failed before implementation because its module did not exist.
- Focused green: controller and component passed, 2 files / 15 tests; controller, component, parent recognition workflow, and Capture view passed together, 4 files / 57 tests; `npm run typecheck` passed.
- Final gates: `npm run lint`, `npm run typecheck`, production `npm run build` (2,048 modules), and single-worker full Vitest (133 files / 747 tests) passed.
- Local review: the component no longer watches or resets the whole job; same-job replacements preserve suggestion identity through reordering, authoritative replacements reconcile optimistic decisions, and only new job IDs reset the category/cursor. Recognition algorithms, crop geometry, native commands, and Rust boundaries were unchanged.
- Hygiene: targeted tracked and untracked whitespace checks found no errors; index is empty; `dist` is absent from status; the pre-existing `recognition_visual_split.rs` modification remains present and was not edited in this batch.
