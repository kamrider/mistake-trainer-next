# One-Time Windows LAN Permission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace per-network Public/Private setup with one Windows authorization attempt that persists after success and is retried on every later scan attempt until it succeeds.

**Architecture:** Keep the existing elevated helper, but install one exact application rule for all Windows firewall profiles, restricted to TCP, inbound, `LocalSubnet`, and edge traversal disabled. The Vue scan action will inspect the rule, invoke the helper only when missing/invalid, and immediately start the QR session after a successful repair. Obsolete network-settings commands, bindings, polling, guide UI, and documentation will be removed in the same change.

**Tech Stack:** Rust, Windows Firewall COM API, Tauri typed commands, Vue 3, TypeScript, Vitest.

## Global Constraints

- A successful firewall rule is persistent and must not prompt again.
- Cancellation or failure must leave preflight not-ready so the next scan click retries authorization.
- The allow rule must remain application-scoped, inbound TCP only, `LocalSubnet` only, enabled, and edge traversal disabled.
- The new helper must remove the legacy `Mistake Trainer Next - Mobile Capture (Private)` rule after the replacement rule is installed.
- Rework the current branch with a forward commit; do not rewrite already-pushed Git history.

---

### Task 1: Replace the private-only firewall contract

**Files:**
- Modify: `src-tauri/src/modules/capture_firewall.rs`
- Test: `src-tauri/src/modules/capture_firewall.rs`

**Interfaces:**
- Consumes: existing `repair_capture_firewall()` elevated-helper flow.
- Produces: `evaluate_preflight()` whose readiness depends on exact-rule validity, not Windows profile type.

- [ ] **Step 1: Write failing tests** proving Public, Private, Domain, and mixed profiles can start with an exact ready rule, while Missing/Invalid still require repair.
- [ ] **Step 2: Run** `scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml capture_firewall` and verify the former Public-blocking assertions fail.
- [ ] **Step 3: Implement** a new rule name, all-profile mask, exact validation, and post-install removal of the legacy private-only rule.
- [ ] **Step 4: Run the focused Rust tests** and expect all capture-firewall tests to pass.

### Task 2: Make scan click authorize-and-start

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Test: `src/app/views/CaptureView.test.ts`
- Test: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: `captureLanPreflight`, `captureLanFirewallRepair`, `captureLanStart`.
- Produces: one `mobileCapture(selectedAddress)` event that opens the panel, requests permission if needed, and starts the LAN session after success.

- [ ] **Step 1: Write failing Vue tests** for first-click repair-and-start, ready-rule direct start without repair, and cancelled repair retrying on the next click.
- [ ] **Step 2: Run** `corepack pnpm vitest run src/app/views/CaptureView.test.ts src/modules/capture/components/CaptureWorkspace.test.ts` and verify failures.
- [ ] **Step 3: Implement** `ensureLanPermission()` and route the toolbar scan button through the existing start command; keep the modal open for UAC progress/errors and QR output.
- [ ] **Step 4: Run focused Vue tests** and expect all new flows to pass.

### Task 3: Remove the obsolete per-network guide

**Files:**
- Modify: `src-tauri/src/commands/capture_lan.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/shared/api/bindings.ts` (generated)
- Modify: `src/shared/api/bindings.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Delete: `docs/superpowers/plans/2026-07-15-windows-lan-guided-setup.md`
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Removes: `CaptureLanSettingsPage`, `capture_lan_open_network_settings`, `lanGuidePolling`, network-profile selection UI, and 90-second settings polling.
- Retains: typed preflight, repair, address selection, session start/status/stop.

- [ ] **Step 1: Delete obsolete APIs and UI state**, regenerate bindings with `corepack pnpm bindings:generate`, and update contract assertions.
- [ ] **Step 2: Update acceptance documentation** to describe first-use UAC, persistent success, retry after cancellation, all-profile `LocalSubnet` scope, and legacy-rule cleanup.
- [ ] **Step 3: Run** `corepack pnpm lint`, `corepack pnpm typecheck`, `corepack pnpm test`, `corepack pnpm build`, `corepack pnpm bindings:check`, and `scripts/cargo-msvc.cmd test --manifest-path src-tauri/Cargo.toml --all-targets`.
- [ ] **Step 4: Build and commit** with `corepack pnpm tauri build`, then commit and push only after every gate passes.
