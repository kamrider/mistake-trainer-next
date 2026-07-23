# Desktop Crop Workbench Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing non-destructive desktop crop editor precise and comfortable for large worksheet photos by fixing zoom reachability, adding eight-direction resize, canvas panning, ordered region previews, and accessible motion.

**Architecture:** Keep Rust crop recipes and persistence unchanged. Extract normalized crop geometry into a small pure TypeScript module, then let `CaptureCropEditor.vue` own only viewport state, history, focus, and presentation. Zoom changes the image shell's real layout dimensions so scrollbars cover every pixel; crop rectangles remain normalized and therefore independent of screen scaling.

**Tech Stack:** Vue 3 Composition API, TypeScript strict, Vitest, Testing Library, existing Motion CSS tokens and Lucide icons.

## Global Constraints

- Never mutate, replace, or delete the encrypted source asset.
- Keep the existing atomic `CaptureCropRecipe[]` command contract and 1–10 region limit.
- Crop coordinates remain finite normalized `[0, 1]` rectangles with a minimum width and height of `0.015`.
- Zoom and panning must not change emitted crop coordinates.
- Mouse, pen, touch, keyboard, 200% scaling, high contrast, and `prefers-reduced-motion` remain usable.
- Do not add a runtime dependency.

---

### Task 1: Extract deterministic crop geometry

**Files:**
- Create: `src/modules/capture/domain/cropGeometry.ts`
- Create: `src/modules/capture/domain/cropGeometry.test.ts`

**Interfaces:**
- Produces: `CropRegion`, `CropResizeHandle`, `moveCropRegion(region, dx, dy)`, `resizeCropRegion(region, handle, dx, dy, minimumSize?)`, and `fitImageWithin(naturalWidth, naturalHeight, viewportWidth, viewportHeight, maximumWidth?)`.

- [x] **Step 1: Write failing geometry tests**

Cover movement clamping on every edge, all eight resize handles, minimum size, finite output, and landscape/portrait contain-fit dimensions. Assert that west/north resizing changes origin while east/south resizing does not.

- [x] **Step 2: Run the focused test and verify the missing module failure**

Run: `pnpm vitest run src/modules/capture/domain/cropGeometry.test.ts`

Expected: FAIL because `cropGeometry.ts` does not exist.

- [x] **Step 3: Implement pure normalized geometry**

Use left/top/right/bottom edges for resizing. Clamp moving rectangles without changing size; clamp the selected resize edges without allowing them to cross the opposite edge minus `minimumSize`; reject non-finite deltas by returning the unchanged region. `fitImageWithin` must return positive integer CSS dimensions and preserve aspect ratio.

- [x] **Step 4: Run the focused test**

Run: `pnpm vitest run src/modules/capture/domain/cropGeometry.test.ts`

Expected: PASS.

---

### Task 2: Make zoomed pixels reachable and crop handles precise

**Files:**
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.test.ts`

**Interfaces:**
- Consumes: geometry functions from Task 1 and existing `CaptureCropRecipe[]` emit.
- Produces: real-dimension zoom viewport, Ctrl/Command-wheel zoom, Space/middle-button canvas pan, eight named resize handles, reset-to-fit, and unchanged normalized recipes.

- [x] **Step 1: Add failing interaction tests**

Assert that the editor renders eight resize handles for the active region, exposes a labelled scrollable canvas, supports zoom buttons without modifying emitted recipes, and retains Escape plus keyboard nudge behavior. Assert controls include usable `aria-label` text and no double-click handler.

- [x] **Step 2: Run the component test and verify failure**

Run: `pnpm vitest run src/modules/capture/components/CaptureCropEditor.test.ts`

Expected: FAIL because only one resize handle and transformed zoom exist.

- [x] **Step 3: Replace transform zoom with a fitted canvas surface**

Measure the stage using `ResizeObserver`, read the rendered image's natural size, call `fitImageWithin`, and bind pixel `width`/`height` to `.image-shell`. Give the surrounding surface actual scaled dimensions so browser scroll extents include every zoomed pixel. Preserve viewport center when toolbar zoom changes and pointer anchor when Ctrl/Command-wheel zoom changes.

- [x] **Step 4: Add pan and eight-direction resize**

Use Space plus primary pointer or middle pointer to pan `scrollLeft`/`scrollTop`; do not begin pan from a region or control. Render `n`, `ne`, `e`, `se`, `s`, `sw`, `w`, and `nw` handles and delegate normalized math to `resizeCropRegion`. Use pointer capture/window cleanup and `touch-action: none` only on draggable crop controls.

- [x] **Step 5: Preserve accessibility and reduced motion**

Keep the modal focus trap and Escape behavior, add visible `:focus-visible` outlines, high-contrast borders, an explicit keyboard/pan hint, and disable canvas/region/list transitions under reduced motion.

- [x] **Step 6: Run component tests**

Run: `pnpm vitest run src/modules/capture/components/CaptureCropEditor.test.ts`

Expected: PASS.

---

### Task 3: Make multi-region output visually understandable and reorderable

**Files:**
- Modify: `src/modules/capture/components/CaptureCropEditor.vue`
- Modify: `src/modules/capture/components/CaptureCropEditor.test.ts`

**Interfaces:**
- Produces: cropped region thumbnails, move-up/move-down controls, animated-but-accessible active selection, and recipe order equal to the visible list order.

- [x] **Step 1: Write the failing order test**

Create three regions, move the third upward, apply, and assert the emitted recipe order matches the visible numbered list. Assert top/bottom rows disable impossible moves and delete keeps a valid active region.

- [x] **Step 2: Run the component test and verify failure**

Run: `pnpm vitest run src/modules/capture/components/CaptureCropEditor.test.ts`

Expected: FAIL because region reorder controls do not exist.

- [x] **Step 3: Implement visual filmstrip and ordering**

Render a clipped preview from `renderedDataUrl` for each normalized region. Add labelled up/down buttons that reorder the `regions` array after `checkpoint()`, keep the same active ID, and renumber both overlays and rows. Animate row movement with transform/opacity only and suppress it for reduced motion.

- [x] **Step 4: Run crop editor and full capture component tests**

Run: `pnpm vitest run src/modules/capture/components/CaptureCropEditor.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: PASS.

---

### Task 4: Verify and baseline the increment

**Files:**
- Modify: `docs/superpowers/plans/2026-07-22-question-crop-and-extraction.md`
- Modify: this plan

**Interfaces:**
- Produces: updated roadmap status, repeatable test evidence, and one local Git baseline commit.

- [x] **Step 1: Run frontend gates**

Run: `pnpm lint`, `pnpm typecheck`, `pnpm test -- --run`, and `pnpm build`.

Expected: PASS and initial application JavaScript remains below 300 KB gzip.

- [x] **Step 2: Run crop persistence gates**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`.

Expected: PASS apart from documented non-fatal SQLCipher `VirtualLock` or OpenSSL PDB warnings.

- [x] **Step 3: Perform visual acceptance**

Open the real crop editor with a tall worksheet and a landscape photo. Verify 100%, 200%, and 250% zoom can reach all four image edges; Space-drag pan works; all handles remain visible; three regions can be reordered; 1280×720 and 200% scaling retain the footer; reduced motion removes nonessential animation.

- [x] **Step 4: Review and commit locally**

Run `git diff --check`, inspect the full diff for temporary data or source paths, then commit only the plan, crop geometry, crop editor, tests, and roadmap with:

```powershell
git commit -m "feat: polish the desktop crop workbench"
```
