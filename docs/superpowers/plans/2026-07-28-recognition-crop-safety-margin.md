# Recognition Crop Safety Margin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand every automatically recognized question/answer crop by eight pixels on all four sides without allowing the crop to leave the source image.

**Architecture:** Keep the existing model-free foreground and whitespace detection unchanged. Add one pixel-bound helper at the proposal-construction boundary so both single- and multi-column regions receive the same fixed safety margin before their coordinates are normalized.

**Tech Stack:** Rust 2024, `image` crate, built-in Rust unit tests

## Global Constraints

- Preserve the existing adaptive padding and add an eight-pixel safety margin after detection.
- Clamp every expanded edge to `0..image_dimension`.
- Do not change confidence scoring, reading order, role inheritance, or blank-page fallback behavior.
- Do not modify unrelated Windows release/update work already present in the worktree.

---

### Task 1: Expand recognized crop bounds safely

**Files:**
- Modify: `src-tauri/src/infrastructure/recognition_visual_split.rs`
- Test: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Consumes: detected half-open pixel bounds `(start, end)` and a source-image dimension `limit`.
- Produces: `expand_with_safety_margin(start: usize, end: usize, limit: usize) -> (usize, usize)`.

- [x] **Step 1: Write the failing boundary test**

Add this unit test to `recognition_visual_split.rs`:

```rust
#[test]
fn expands_detected_bounds_by_eight_pixels_and_clamps_to_the_page() {
    assert_eq!(expand_with_safety_margin(20, 40, 100), (12, 48));
    assert_eq!(expand_with_safety_margin(3, 95, 100), (0, 100));
}
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml infrastructure::recognition_visual_split::tests::expands_detected_bounds_by_eight_pixels_and_clamps_to_the_page
```

Expected: compilation fails because `expand_with_safety_margin` is not defined.

- [x] **Step 3: Add the safety-margin helper and apply it to every detected proposal**

Add the constant and helper:

```rust
const CROP_SAFETY_MARGIN_PX: usize = 8;

fn expand_with_safety_margin(start: usize, end: usize, limit: usize) -> (usize, usize) {
    (
        start.saturating_sub(CROP_SAFETY_MARGIN_PX),
        limit.min(end.saturating_add(CROP_SAFETY_MARGIN_PX)),
    )
}
```

At proposal construction, expand both axes after the existing adaptive padding:

```rust
let (left, right) = expand_with_safety_margin(x_start, x_end, width);
let (top, bottom) = expand_with_safety_margin(top, bottom, height);
```

Build the normalized rectangle from `left`, `right`, `top`, and `bottom`, retaining the existing `bottom <= top` guard.

- [x] **Step 4: Run visual-split unit tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml infrastructure::recognition_visual_split::tests
```

Expected: all non-ignored visual-split tests pass.

- [x] **Step 5: Run formatting and inspect the focused diff**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
git diff --check -- src-tauri/src/infrastructure/recognition_visual_split.rs
git diff -- src-tauri/src/infrastructure/recognition_visual_split.rs
```

Expected: formatting and whitespace checks pass; the diff contains only the margin constant/helper, proposal-bound expansion, and its unit test.

- [ ] **Step 6: Commit**

```powershell
git add docs/superpowers/plans/2026-07-28-recognition-crop-safety-margin.md src-tauri/src/infrastructure/recognition_visual_split.rs
git commit -m "fix: add margin around recognized question crops"
```

Only run this step if the user explicitly asks for a commit.

## Self-Review

- Spec coverage: the plan adds pixels on the top, bottom, left, and right and clamps near image edges.
- Placeholder scan: no placeholders or deferred implementation steps remain.
- Type consistency: the helper takes and returns half-open `usize` pixel bounds used by the existing normalized crop conversion.
