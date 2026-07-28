# Mobile Quick Crop and Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a phone user finish rapid capture first, then optionally review, rotate, non-destructively crop, or restore each uploaded image without slowing or invalidating the upload queue.

**Architecture:** Keep the original normalized upload as the encrypted source of truth. The mobile page sends only a normalized rectangle and quarter-turn rotation to an authenticated LAN endpoint; Rust reuses the existing crop ledger, asset encryption, capacity checks, atomic writes, and 30-day source retention. The same crop use case accepts `collecting` only when an internal LAN caller opts in; the desktop command remains `organizing`-only.

**Tech Stack:** Existing Axum LAN server, Rust `image`, SQLCipher/AES-GCM capture store, one-file offline HTML/CSS/JavaScript mobile page, existing Rust test router.

## Global Constraints

- Do not add a runtime dependency or increase the mobile page's network surface beyond same-origin authenticated endpoints.
- Do not send file paths, encryption keys, account IDs, profile IDs, or crop source bytes to JavaScript.
- Do not replace, mutate, or automatically delete the encrypted source asset.
- The default camera loop must continue to enqueue the selected photo and immediately reopen the system camera before normalization or upload completes.
- Mobile crop supports one normalized rectangle and rotation `0 | 90 | 180 | 270`; desktop remains the 1–10 region editor.
- Crop and restore must be atomic and safe during concurrent uploads because all database mutations use the existing connection mutex and current batch revision.
- Every motion must respect `prefers-reduced-motion: reduce`; controls must be usable at 375 CSS pixels without horizontal overflow.

---

### Task 1: Permit the existing crop ledger from an authenticated collecting session

**Files:**
- Modify: `src-tauri/src/modules/capture_inbox.rs`
- Modify: `src-tauri/src/commands/capture_inbox.rs`
- Test: `src-tauri/tests/capture_inbox_store.rs`

**Interfaces:**
- Consumes: `apply_capture_crop`, `revert_capture_crop`, `CaptureBatchState`, existing revision checks.
- Produces: `ApplyCaptureCrop.allow_collecting: bool` and `RevertCaptureCrop.allow_collecting: bool`; `false` preserves desktop behavior, `true` accepts `collecting | organizing` but never `completed`.

- [x] **Step 1: Write the failing state-policy test**

Create a collecting batch and source item. Call `apply_capture_crop` with `allow_collecting: false` and assert `CaptureInboxError::InvalidState`; then call the same recipe with `allow_collecting: true` and assert one derived item. Revert with `allow_collecting: true` and assert the original item returns.

- [x] **Step 2: Run the focused test and verify it fails to compile**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store crop_collecting_requires_internal_permission`

Expected: FAIL because `allow_collecting` does not exist.

- [x] **Step 3: Add the explicit internal state policy**

Add the boolean to both internal input structs and replace the crop/revert state check with:

```rust
fn ensure_crop_revision(
    batch: &CaptureBatchSummary,
    expected_revision: u32,
    allow_collecting: bool,
) -> Result<(), CaptureInboxError> {
    let state_allowed = batch.state == CaptureBatchState::Organizing
        || (allow_collecting && batch.state == CaptureBatchState::Collecting);
    if !state_allowed {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    Ok(())
}
```

Set `allow_collecting: false` in both Tauri commands and all existing non-LAN tests.

- [x] **Step 4: Run the capture store tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store`

Expected: PASS.

---

### Task 2: Add authenticated mobile crop and restore endpoints

**Files:**
- Modify: `src-tauri/src/modules/capture_lan.rs`
- Test: in-file `capture_lan::tests`

**Interfaces:**
- Consumes: `POST /api/v1/items/{itemId}/crop` JSON body and `POST /api/v1/items/{itemId}/crop/revert`.
- Produces: session item field `cropDerivationId: string | null`; crop response `{ itemId, sourceItemId, cropDerivationId }`; restore returns HTTP 204.

- [x] **Step 1: Write the failing router test**

Upload an 8×4 two-color PNG. POST this JSON while the batch is collecting:

```json
{"rect":{"x":0.0,"y":0.0,"width":0.5,"height":1.0},"rotationDegrees":0}
```

Assert HTTP 200, a different active item ID, one derivation ID in `/session`, a red preview pixel, and the original source retained in SQL. POST the derived item to `/crop/revert`; assert 204, the source item is active again, and derivation rows are gone. Add an out-of-bounds request and assert HTTP 400 with no revision or item change.

- [x] **Step 2: Run the LAN tests and verify the new route fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests::mobile_crop_is_non_destructive_and_reversible`

Expected: FAIL with route not found.

- [x] **Step 3: Implement typed request/response and route handlers**

Add:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MobileCropRequest {
    rect: NormalizedCropRect,
    rotation_degrees: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileCropPayload {
    item_id: String,
    source_item_id: String,
    crop_derivation_id: String,
}
```

The crop handler must authorize first, lock the connection, load the current detail/revision, call `apply_capture_crop` with one `image/png` recipe (`max_edge: 4096`, `jpeg_quality: 90`, `allow_collecting: true`), touch session activity, notify desktop, and return the first derived IDs. The restore handler must find the requested active item's `crop_derivation_id`, call `revert_capture_crop` with the current revision and `allow_collecting: true`, then notify and return 204.

- [x] **Step 4: Run all capture LAN tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests`

Expected: PASS.

---

### Task 3: Build the mobile review and crop editor without breaking capture

**Files:**
- Modify: `src-tauri/mobile/capture.html`
- Test: in-file mobile page assertions in `src-tauri/src/modules/capture_lan.rs`

**Interfaces:**
- Consumes: session `items[].cropDerivationId`, preview endpoint, crop/restore endpoints.
- Produces: `拍完统一检查`, per-image `裁剪`/`恢复原图`, a modal crop editor with move/resize, 90° rotation, reset, previous/next, skip, and apply.

- [x] **Step 1: Expand the mobile-page contract assertions**

Assert the embedded page contains `id="reviewCapture"`, `role="dialog"`, `aria-modal="true"`, `openCropEditor`, `applyMobileCrop`, `restoreMobileCrop`, pointer capture, the crop endpoint strings, and reduced-motion rules. Assert it does not contain destructive canvas upload code for crop.

- [x] **Step 2: Run the mobile-page test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests::mobile_page_hardens_headers_and_keeps_heic_decoder_lazy`

Expected: FAIL on the first new assertion.

- [x] **Step 3: Reconcile server items after crop/restore**

Replace append-only restore logic with a reconciliation pass that:

1. Updates local uploaded items by `serverId`.
2. Removes only `done` server-backed items absent from the latest session.
3. Adds new derived/source items and lazily loads their preview.
4. Carries `cropDerivationId` into item state.

Never remove `preparing`, `pending`, `uploading`, or `failed` local queue entries.

- [x] **Step 4: Add the crop editor interaction**

Use the existing preview data URL as the visible source. Render quarter-turn rotation into an in-memory canvas only for display; send the normalized rectangle and rotation to Rust. Implement pointer move/resize against the displayed image bounds with an 8% minimum width/height and clamped `[0, 1]` coordinates. On apply, disable modal controls, POST the recipe, refresh session, animate to the next item, and keep the camera/upload state untouched.

- [x] **Step 5: Add review and recovery affordances**

Enable `拍完统一检查` when at least one uploaded item exists. Per-item `裁剪` opens the same editor; derived items show `恢复原图` and hide destructive delete until restored. The review modal shows `N / total`, supports previous/next and `保留原图，下一张`, traps focus, closes on Escape when idle, and returns focus to its launcher.

- [x] **Step 6: Add polished responsive motion**

Use opacity/transform for modal and item replacement, a 120 ms control response, 180 ms crop-box transitions, and a 240 ms modal transition. At `max-width: 520px`, keep the action row and footer inside 375 CSS pixels. Disable all nonessential animation under reduced motion.

- [x] **Step 7: Run capture LAN tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests`

Expected: PASS.

---

### Task 4: Document and verify the complete increment

**Files:**
- Modify: `docs/windows-capture-acceptance.md`
- Verify: repository quality gates

**Interfaces:**
- Consumes: completed mobile workflow.
- Produces: repeatable iPhone Safari and Android Chrome acceptance checklist.

- [x] **Step 1: Add real-device acceptance cases**

Document: continue shooting while earlier images upload; open unified review; crop and rotate; reload and confirm the crop remains; restore original; retry a failed crop without losing the source; finish and confirm the desktop organizer receives the active derivative; verify 375 px has no horizontal overflow.

- [x] **Step 2: Run formatting and focused Rust gates**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_inbox_store
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests
```

Expected: PASS, apart from already documented non-fatal SQLCipher `VirtualLock` or OpenSSL PDB warnings.

- [x] **Step 3: Run frontend and build gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test -- --run
pnpm build
```

Expected: PASS and initial JavaScript remains below 300 KB gzip.

- [x] **Step 4: Perform browser visual acceptance**

Serve the embedded mobile page fixture or the running LAN page, inspect at 375×812 and 430×932, and verify `document.documentElement.scrollWidth === window.innerWidth`, crop handles stay on the image, the fixed footer does not obscure controls, and reduced motion removes modal/item animations.

- [x] **Step 5: Self-review and create a local baseline**

Run `git diff --check`, inspect the full diff, confirm no token or source path is exposed, and commit only the related files with:

```powershell
git commit -m "feat: add mobile capture review and quick crop"
```
