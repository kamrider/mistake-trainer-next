# Mobile Quick Shoot Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the unreliable “continuous shooting” promise with a fast, honest, one-tap repeat-shoot loop that remains usable in WeChat, Safari, and Chrome while earlier photos process and upload.

**Architecture:** Keep the native file/camera picker because the LAN page is ordinary HTTP and cannot reliably use `getUserMedia`. The page detects WeChat only to explain its picker restriction, attempts an automatic reopen only while a browser still reports active user activation, and otherwise keeps one large fixed “继续拍一张” action visible. Upload, normalization, crop, and session APIs remain unchanged.

**Tech Stack:** Existing one-file mobile HTML/CSS/JavaScript page, Axum embedded-page contract tests, Rust test runner.

## Global Constraints

- Do not add a dependency, cloud hop, TLS certificate flow, or new network endpoint.
- Do not claim that a native camera picker can reopen without another user gesture.
- A confirmed photo must enter the local queue before any next-shot action and must upload in the background.
- WeChat guidance must be contextual and must not block capture.
- The fixed next-shot action must remain usable at 375 CSS pixels without horizontal overflow.
- Motion and vibration must be nonessential; reduced-motion mode disables the visual pulse, and unavailable vibration is silently ignored.

---

### Task 1: Lock the honest quick-shoot contract

**Files:**
- Modify: `src-tauri/src/modules/capture_lan.rs`
- Test: `src-tauri/src/modules/capture_lan.rs`

**Interfaces:**
- Consumes: the embedded `MOBILE_PAGE` string.
- Produces: contract assertions for `快速拍一张`, `继续拍一张`, `MicroMessenger`, `navigator.userActivation`, and the absence of the former automatic-continuous-shoot copy.

- [x] **Step 1: Write failing embedded-page assertions**

Add assertions to `mobile_page_hardens_headers_and_keeps_heic_decoder_lazy`:

```rust
assert!(MOBILE_PAGE.contains("快速拍一张"));
assert!(MOBILE_PAGE.contains("继续拍一张"));
assert!(MOBILE_PAGE.contains("MicroMessenger"));
assert!(MOBILE_PAGE.contains("navigator.userActivation"));
assert!(!MOBILE_PAGE.contains("开始连续拍照"));
assert!(!MOBILE_PAGE.contains("正在打开下一张"));
```

- [x] **Step 2: Run the focused test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests::mobile_page_hardens_headers_and_keeps_heic_decoder_lazy
```

Expected: FAIL because the page still promises automatic continuous shooting.

- [x] **Step 3: Commit only after Task 2 passes**

Keep the failing test and implementation in one reviewable commit because the embedded HTML is compiled by the same Rust module.

---

### Task 2: Implement a reliable one-tap repeat-shoot dock

**Files:**
- Modify: `src-tauri/mobile/capture.html`
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Consumes: existing `cameraInput`, `cameraPick`, `nextCamera`, `cameraMode`, `addFiles(files)`, and background upload queue.
- Produces: `quickShootGuidance()`, `tryAutoRelaunchCamera()`, contextual WeChat guidance, a captured-count status, and fixed one-tap continuation.

- [x] **Step 1: Replace misleading copy and expose a status title**

Use these exact visible labels:

```html
<button id="cameraPick" ...>
  <b id="cameraPickTitle">快速拍一张</b>
  <small>上一张会在后台上传，不用等待</small>
</button>
```

The fixed dock uses `id="cameraModeTitle"`, initial title `快速拍摄已就绪`, primary action `继续拍一张`, and secondary action `收起快拍`.

- [x] **Step 2: Add contextual guidance and safe relaunch**

Implement:

```js
const isWeChat = /MicroMessenger/i.test(navigator.userAgent || '')
const quickShootGuidance = () => isWeChat
  ? '微信会拦截网页自动重开相机。上一张已在后台处理，点“继续拍一张”即可。'
  : '上一张会在后台处理；若系统相机没有自动出现，点“继续拍一张”。'
const tryAutoRelaunchCamera = () => {
  if (navigator.userActivation?.isActive) launchCamera()
  else cameraModeCopy.textContent = quickShootGuidance()
}
```

After `addFiles(files)` has synchronously rendered queue entries, increment the accepted-photo count, update `cameraModeTitle`, optionally call `navigator.vibrate?.(18)`, and call `tryAutoRelaunchCamera()`. Never wait for image normalization or upload before enabling the next-shot button.

- [x] **Step 3: Polish the fixed dock**

Make the primary action occupy the available width on narrow screens, keep the secondary action compact, add an `is-ready` transform/opacity pulse only under `prefers-reduced-motion: no-preference`, and keep the finish bar unobscured.

- [x] **Step 4: Update acceptance language**

Document that:

1. Native camera sheets may require one explicit tap per photo.
2. WeChat must show contextual guidance after the first photo.
3. The fixed `继续拍一张` action remains visible while earlier photos normalize or upload.
4. A browser may auto-reopen only when it still exposes active user activation.
5. No UI may describe this as unattended continuous shooting.

- [x] **Step 5: Run focused and formatting verification**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_lan::tests
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 6: Commit the increment**

```powershell
git add src-tauri/mobile/capture.html src-tauri/src/modules/capture_lan.rs docs/windows-capture-acceptance.md docs/superpowers/plans/2026-07-23-mobile-quick-shoot-loop.md
git commit -m "fix: make mobile repeat capture reliable"
```

---

## Self-Review

- Spec coverage: copy honesty, one-tap fixed action, WeChat guidance, background upload, reduced motion, and 375 px layout each have an implementation and acceptance step.
- Placeholder scan: no `TBD`, `TODO`, unspecified handler, or unbounded “handle edge cases” step remains.
- Type consistency: the plan introduces only page-local functions and existing DOM IDs; no Rust or TypeScript public interface changes.
