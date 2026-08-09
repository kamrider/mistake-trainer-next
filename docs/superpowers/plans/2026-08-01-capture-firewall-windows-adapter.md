# Capture Firewall Windows Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate native Windows Firewall COM and elevation mechanics from the capture-firewall public policy facade without changing LAN access behavior or security scope.

**Architecture:** Keep `capture_firewall.rs` as the public DTO/error/constants owner, deterministic preflight-policy owner, cross-platform facade, and exact helper-argument gate. Add a private `capture_firewall_windows.rs` adapter that owns Windows COM lifecycle, firewall rule inspection/installation, executable-path normalization, UAC launch, process wait, and native error conversion; the facade continues calling three platform operations under `cfg(windows)`.

**Tech Stack:** Rust 2024, windows-rs Win32 COM/Firewall/Shell APIs, serde/specta DTOs, Vitest source contracts, Rust unit tests

## Global Constraints

- Preserve every public function, type, constant, error variant/message, serde/specta shape, and non-Windows behavior.
- Preserve exact helper argument matching, rule names, current-executable binding, TCP/inbound/allow semantics, all-profile coverage, `LocalSubnet`-only scope, disabled edge traversal, enabled state, legacy-rule removal, UAC cancellation mapping, 60-second wait, handle cleanup, and exit-code handling.
- Keep preflight policy and `remote_scope_is_exact_local_subnet` in the facade so native inspection consumes domain policy rather than redefining it.
- Declare the Windows adapter privately and compile it only on Windows; do not modify commands, `main.rs`, bindings, dependencies, or existing Rust tests.
- Format only the selected facade and new adapter; preserve the dirty worktree and do not stage or commit.
- Do not implement licensing, privacy/legal policy text, support operations, account deletion, device migration, update failure recovery, or SLA work.

---

### Task 1: Structural Contract And Unit Baseline

**Files:**
- Create: `tests/capture-firewall-windows-adapter-boundary.test.ts`
- Test: `src-tauri/src/modules/capture_firewall.rs`

- [x] **Step 1: Add the failing source contract**

Assert the private Windows-only adapter declaration, exactly three adapter operations, public facade ownership, native dependency isolation, and the security-critical rule/elevation tokens.

- [x] **Step 2: Run the source contract and verify red**

Run: `npm test -- --run tests/capture-firewall-windows-adapter-boundary.test.ts`

Expected: FAIL because the private adapter file and facade declaration do not exist.

- [x] **Step 3: Run the existing firewall unit baseline**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib modules::capture_firewall::tests`

Expected: 8/8 preflight, subnet-scope, and helper-argument cases pass before extraction.

### Task 2: Extract Native Windows Adapter

**Files:**
- Create: `src-tauri/src/modules/capture_firewall_windows.rs`
- Modify: `src-tauri/src/modules/capture_firewall.rs`

- [x] **Step 1: Move the native implementation without semantic edits**

Move the complete body of `windows_impl` into the private path module, retaining its imports, `ComGuard`, three `pub(super)` operations, rule inspection/installation, elevation flow, path normalization, and wide-string helper.

- [x] **Step 2: Keep platform routing and policy in the facade**

Replace the inline module with `#[cfg(windows)] #[path = "capture_firewall_windows.rs"] mod windows_impl;`. Keep all public functions and non-Windows branches at their existing module paths.

- [x] **Step 3: Format and run focused green tests**

Format only both target Rust files. Run the source contract and the 8 firewall unit tests; both must pass.

### Task 3: Adjacent And Full Regression

- [x] **Step 1: Run adjacent command and LAN suites**

Run the command contract, capture LAN API boundary, and capture LAN store/integration suites selected from current test inventory.

- [x] **Step 2: Run strict Rust gates**

Run all-target Clippy with `-D warnings`, then the complete Rust suite.

- [x] **Step 3: Run frontend and static gates**

Run complete Vitest, typecheck, and lint.

### Task 4: Security Review, Hygiene, And Record

- [x] **Step 1: Review native security semantic identity**

Review public API identity, exact helper gate, COM lifecycle, rule-name lookup/removal, executable binding, all firewall properties, LocalSubnet equality, UAC cancellation, timeout/handle closure, exit-code propagation, cfg routing, and source-contract robustness. Fix every Critical or Important finding.

- [x] **Step 2: Verify scope and workspace hygiene**

Run target whitespace checks, `git diff --check`, and confirm the staged index remains empty.

- [x] **Step 3: Record exact verification evidence**

Check completed steps and append red/green totals, regression results, line counts, preserved invariants, review verdict, and exact batch scope without staging or committing.

## Verification Record

- Red phase: the new source contract failed 1/3 before extraction because the private Windows adapter and cfg-gated path declaration did not exist; the remaining two checks were intentionally guarded until the adapter existed.
- Characterization baseline: the eight capture-firewall unit tests passed 8/8 before extraction, covering public/domain/mixed/private profiles, missing/non-Windows states, exact LocalSubnet rejection, and the exact single helper argument.
- Focused green phase: the source contract passed 3/3 and the capture-firewall unit tests passed 8/8 after extraction.
- Adjacent compatibility: the current Cargo inventory contains no separate capture-LAN integration target. `command_contract` passed 8/8 and `profile_command` passed 5/5 (13/13 total), including the in-flight LAN transition lock. An initially requested non-existent `bindings_contract` target was removed before any adjacent tests ran.
- Rust quality gates: the Windows/MSVC all-target Clippy build passed with `-D warnings`, proving the new native file compiles against the actual Win32 APIs; the complete Rust suite exited 0 with 113/113 library unit tests and every non-ignored integration test passing. Three environment-dependent OCR runtime/corpus probes remained explicitly ignored.
- Frontend quality gates: the first full Vitest run, launched concurrently with typecheck and lint, passed 653/654 and timed out one unrelated `App.test.ts` route transition at its one-second wait. The isolated file then passed 6/6 and the serial full rerun passed 113/113 files and 654/654 tests, identifying resource contention rather than a product regression. Serial `vue-tsc --noEmit` and ESLint with zero warnings passed.
- File shape: `capture_firewall.rs` reduced from 473 to 230 lines. The 242-line private Windows adapter now has one owner for COM lifecycle, firewall profile discovery, exact rule inspection/installation, executable-path normalization, UAC elevation, process wait/exit handling, and native error conversion.
- Preserved invariants: public constants/functions/types/errors and non-Windows behavior; exact single helper argument; current-executable binding; rule names and legacy cleanup; enabled inbound TCP allow across domain/private/public profiles; exact LocalSubnet scope; disabled edge traversal; rule validation; COM balance including changed-mode handling; UAC cancellation; 60-second wait; handle closure; exit-code propagation; and facade-owned preflight policy are unchanged.
- Review verdict: no Critical or Important findings. Local review was used because this task did not authorize a reviewer subagent. The source contract requires exactly three native operations, prevents Win32/unsafe code from returning to the facade, and locks every security-critical rule/elevation token to the adapter.
- Hygiene and scope: target trailing-whitespace and `git diff --check` checks passed; the staged index is empty. Only the previously clean facade plus the new Windows adapter, architecture contract, and this plan belong to this batch. Existing dirty/untracked files were preserved; nothing was staged or committed.
