# Capture Card Editing Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the capture card editing surface commercially readable and touch-friendly, and add a safe browser design fixture so its desktop and narrow layouts can be verified repeatedly.

**Architecture:** Add a pure development-preview fixture factory and let `CaptureView` load it only for the Vite browser route `/#/inbox?preview=capture-card`; the normal browser warning and all desktop/native flows remain unchanged. Add a source contract covering the directly coupled `CaptureDraftCard` and `CaptureThumbnail`, then change only their CSS typography and action geometry.

**Tech Stack:** Vue 3, TypeScript, Vue Router, Vitest, Testing Library Vue, Vite browser preview, scoped CSS

## Global Constraints

- Do not modify Rust, native commands, capture persistence, drag/drop transactions, crop behavior, or production desktop initialization.
- The preview fixture must run only when `import.meta.env.DEV`, `!isTauri()`, and `route.query.preview === 'capture-card'` are all true.
- The preview fixture must use generated SVG data URLs and must not read files, call native APIs, or persist data.
- Every explicit visible pixel font size in `CaptureDraftCard.vue` and `CaptureThumbnail.vue` must be at least 12px.
- Every real select, button, or link-like capture-card action must expose a 44px target. Non-interactive readiness and drag indicators may remain smaller.
- Preserve existing keyboard shortcuts, drag behavior, card flip behavior, focus changes, responsive breakpoints, and reduced-motion behavior.
- Do not implement launch-gate licensing, privacy/legal policy text, support operations, account deletion, device migration, update recovery, or SLA work.
- Do not stage or commit the dirty worktree.

---

### Task 1: Safe Capture Card Development Preview

**Files:**
- Create: `src/app/views/capture-development-preview.ts`
- Create: `src/app/views/capture-development-preview.test.ts`
- Modify: `src/app/views/CaptureView.vue`

**Interfaces:**
- Produces: `createCaptureDevelopmentPreview(now?: number): { batches: CaptureBatchSummary[]; detail: CaptureBatchDetail; previews: Record<string, string> }`.
- Consumes: `CaptureView` assigns the returned arrays/object to its existing refs and reactive preview cache only in the exact development browser route.

- [x] **Step 1: Add the failing fixture contract test**

Assert the factory returns an organizing batch, one ready draft and one incomplete draft, consistent item/draft references, at least one question and answer preview, and only `data:image/svg+xml` preview values.

- [x] **Step 2: Run the fixture test and verify red**

Run: `npm test -- --run src/app/views/capture-development-preview.test.ts`

Expected: FAIL because the fixture module does not exist.

- [x] **Step 3: Implement the pure fixture and route gate**

Create two realistic drafts and five items from typed binding structures. Add an SVG data URL helper. In `CaptureView.onMounted`, before the generic non-desktop warning, load the fixture only when the exact three-part development predicate is true; assign `batches`, `detail`, and `previews`, then return without calling native setup.

- [x] **Step 4: Run fixture and existing CaptureView tests**

Run: `npm test -- --run src/app/views/capture-development-preview.test.ts src/app/views/CaptureView.test.ts`

Expected: both files pass and existing desktop behavior remains covered.

### Task 2: Card And Thumbnail Readability Contract

**Files:**
- Create: `src/modules/capture/components/CaptureCardEditingReadability.test.ts`
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureThumbnail.vue`

**Interfaces:**
- Consumes: both SFC sources with `node:fs` from repository-root paths.
- Produces: a contract rejecting visible explicit font sizes below 12px and asserting exact 44px action rules.

- [x] **Step 1: Add the failing source contract**

Scan both SFCs for pixel `font-size` values below 12. Assert 44px rules for `.draft-target`, `.card-subject`, `.expand-image,.crop-image`, `.change-role`, `.flip-button`, `.return-image`, `.image-overlay button`, `.remove-button`, `.crop-button`, and `.is-filmstrip .crop-button`.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run src/modules/capture/components/CaptureCardEditingReadability.test.ts`

Expected: FAIL with current 9px, 10px, and 11px text plus 25–42px action targets.

- [x] **Step 3: Raise visible copy and action geometry**

Change every explicit 9px, 10px, or 11px font size in the two target SFCs to 12px. Raise the contracted actions to 44px while preserving the 20–28px non-interactive drag and readiness indicators, thumbnail dimensions, component markup, and behavior.

- [x] **Step 4: Run focused card and thumbnail tests**

Run: `npm test -- --run src/modules/capture/components/CaptureCardEditingReadability.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all focused tests pass.

### Task 3: Desktop And Narrow Browser Verification

**Files:**
- Modify only Task 1 or Task 2 files if visual inspection finds clipping or horizontal overflow.

**Interfaces:**
- Consumes: `/#/inbox?preview=capture-card`.
- Produces: real layout and interaction evidence for the card, filmstrip, actions, flip state, and image overlay.

- [x] **Step 1: Start a temporary Vite server**

Start `npm run dev -- --host 127.0.0.1`, record the created listener, and stop only that process tree after verification.

- [x] **Step 2: Verify desktop editing**

At 1280px width, confirm the preview loads without the generic browser warning, the first card and its filmstrip fit without page/card horizontal overflow, sampled text computes to at least 12px, and contracted actions compute to at least 44px. Flip to the answer and open/close the image overlay.

- [x] **Step 3: Verify narrow editing**

At 390px width, confirm the subject select wraps below the header, card actions remain reachable without horizontal overflow, thumbnails remain horizontally scrollable within their filmstrip rather than widening the page, and flip/open/close controls remain usable.

- [x] **Step 4: Restore browser and process state**

Reset the viewport, finalize only created tabs, stop the exact Vite execution and child listener, and confirm port 1420 has no remaining listener if it was empty before the task.

### Task 4: Quality Gate And Verification Record

**Files:**
- Modify: `docs/superpowers/plans/2026-08-01-capture-card-editing-readability.md`

**Interfaces:**
- Consumes: all completed tests, browser evidence, and Git checks.
- Produces: checked task boxes and exact verification evidence.

- [x] **Step 1: Run static checks**

Run: `npm run typecheck`

Run: `npm run lint`

Expected: both exit 0.

- [x] **Step 2: Run the complete frontend suite**

Run: `npm test -- --run`

Expected: every frontend test passes.

- [x] **Step 3: Verify patch hygiene and scope**

Run: `git diff --check`

Run: `git diff --cached --name-only`

Expected: no whitespace errors and an empty staged-file list. Confirm no Rust file was changed by this task.

- [x] **Step 4: Record verification without committing**

Check all completed boxes and append the exact red/green test totals, desktop/narrow findings, process cleanup result, static/full test results, hygiene, index, and scope evidence. Do not stage or commit.

## Verification Record — 2026-08-01

- Fixture red: `capture-development-preview.test.ts` failed because the preview module did not yet exist.
- Fixture green: `capture-development-preview.test.ts` plus `CaptureView.test.ts` passed, 2 files / 37 tests.
- Readability red: the source contract reported 10 explicit font sizes below 12px and failed first at the undersized `.draft-target` action.
- Focused green: the readability, draft-card, thumbnail, and workspace suites passed, 4 files / 39 tests.
- Desktop browser (1280×900): the development fixture loaded without the generic browser warning; page and first card had no horizontal overflow; sampled visible text computed to 12px; all sampled actions computed to 44px high; flip, answer display, image open, and image close worked. The image dialog measured 1201×836 and its close target measured 44×44.
- Narrow browser (390×844): page and first card had no horizontal overflow; the subject control wrapped with CSS order 3; the card measured 335px wide with a 333px scroll width; sampled actions remained 44px high. The image dialog measured 351×820, stayed inside the viewport, closed cleanly, and its close target remained 44×44.
- Process hygiene: the temporary server listener was PID 30376; after stopping that exact process, port 1420 had zero listeners. The temporary viewport override was reset and the created browser tab was finalized.
- Static checks: `npm run typecheck` and `npm run lint` both exited 0.
- Full regression: `npm test -- --run` passed, 104 files / 631 tests.
- Patch hygiene: `git diff --check` exited 0; only existing line-ending warnings were printed. `git diff --cached --name-only` was empty.
- Scope: this batch changed only the capture browser preview, its tests, capture-card/thumbnail CSS, the readability contract, and this plan. It did not modify Rust or any launch-gate item, and nothing was staged or committed.
- Final local review: no Critical or Important issue found. The source-level CSS contract is intentionally backed by computed-style and interaction checks in the real browser at both target widths.
