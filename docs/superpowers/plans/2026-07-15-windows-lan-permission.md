# Windows LAN Permission Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make phone LAN capture recoverable for ordinary Windows users without commands: explain the permission before use, detect unusable public-only/firewall states, provide one-click UAC repair, and generate a QR code only after the private-network preflight passes.

**Architecture:** Add a small Windows-only firewall adapter behind a platform-neutral Rust module. The normal Tauri process reads active firewall profiles and the named app rule; an explicitly requested elevated invocation of the same signed executable installs a program-scoped inbound rule and exits before Tauri starts. Vue consumes typed preflight commands and turns the existing LAN dialog into a guided state machine. No PowerShell is exposed or executed, public-profile access is never enabled, and the LAN HTTP protocol remains unchanged.

**Tech Stack:** Rust 1.97, `windows` 0.62.2 COM/Win32 bindings, Tauri 2.11, Specta bindings, Vue 3, Vitest/Testing Library.

## Global Constraints

- Windows v1 only; non-Windows builds return an unsupported preflight without mutating the host.
- The allow rule is restricted to the current executable, inbound TCP, Private profile, and `LocalSubnet` remote addresses.
- Never enable Public-profile access, remove Windows-created Public block rules, open a fixed port, or change the user's network category.
- Firewall mutation occurs only after an explicit user click and a Windows UAC confirmation.
- QR generation is blocked when only Public profiles are active or the private allow rule is absent.
- The elevated mode accepts one hard-coded argument and no user-controlled paths, commands, ports, or rule text.
- Existing capture token, expiry, same-origin, upload limits, encryption, and outbox behavior remain unchanged.

---

## File Structure

- Create `src-tauri/src/modules/capture_firewall.rs`: public preflight types, pure decision logic, Windows COM inspection/repair, elevation launcher, and non-Windows stubs.
- Modify `src-tauri/Cargo.toml`: add exact Windows-only `windows` dependency and only the required Win32 features.
- Modify `src-tauri/src/main.rs`: handle `--configure-capture-firewall` before binding export or Tauri startup.
- Modify `src-tauri/src/modules/mod.rs`: expose the firewall module.
- Modify `src-tauri/src/commands/capture_lan.rs`: add preflight/repair/settings commands and enforce preflight before starting a server.
- Modify `src-tauri/src/bindings.rs`: register commands and regenerate `src/shared/api/bindings.ts`.
- Modify `src/app/views/CaptureView.vue`: own preflight state and command effects.
- Modify `src/modules/capture/components/CaptureWorkspace.vue`: render first-use, blocked, repair, and ready states.
- Modify `src/modules/capture/components/CaptureWorkspace.test.ts`: cover the user-visible permission state machine.
- Modify `docs/windows-capture-acceptance.md`: replace command-oriented recovery with installer/app UX acceptance.

### Task 1: Platform-neutral preflight contract and Windows inspection

**Files:**
- Create: `src-tauri/src/modules/capture_firewall.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `CaptureLanPreflight { supported, active_profiles, firewall_rule, can_start, needs_network_change, needs_firewall_repair }`.
- Produces: `CaptureLanFirewallRuleState::{Ready, Missing, Invalid, Unavailable}` and `CaptureLanProfile::{Domain, Private, Public}`.
- Produces: `capture_lan_preflight() -> Result<CaptureLanPreflight, CaptureFirewallError>`.
- Consumes: current executable path from `std::env::current_exe()`.

- [ ] **Step 1: Write pure decision tests**

```rust
#[test]
fn public_only_profile_blocks_qr_even_with_a_rule() {
    let value = evaluate_preflight(true, &[CaptureLanProfile::Public], CaptureLanFirewallRuleState::Ready);
    assert!(!value.can_start);
    assert!(value.needs_network_change);
    assert!(!value.needs_firewall_repair);
}

#[test]
fn private_profile_and_exact_rule_allow_qr() {
    let value = evaluate_preflight(true, &[CaptureLanProfile::Private], CaptureLanFirewallRuleState::Ready);
    assert!(value.can_start);
}

#[test]
fn missing_rule_requires_repair_without_enabling_public_access() {
    let value = evaluate_preflight(true, &[CaptureLanProfile::Private], CaptureLanFirewallRuleState::Missing);
    assert!(!value.can_start);
    assert!(value.needs_firewall_repair);
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail because the module is absent**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_firewall`

Expected: compilation failure naming the missing module/types.

- [ ] **Step 3: Implement the types, evaluator, and Windows COM reader**

Use `INetFwPolicy2::CurrentProfileTypes` for active Domain/Private/Public bits. Query `INetFwPolicy2::Rules().Item(CAPTURE_FIREWALL_RULE_NAME)` and validate all of these properties: enabled, allow, inbound, TCP (`6`), Private-only profiles, `LocalSubnet`, and normalized `ApplicationName == current_exe`. A rule with the right name but any different property is `Invalid`; item-not-found is `Missing`; COM initialization/policy failures are `Unavailable` and retain a diagnostic error.

Add only Windows target dependencies:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "=0.62.2", features = [
  "Win32_Foundation",
  "Win32_NetworkManagement_WindowsFirewall",
  "Win32_System_Com",
  "Win32_System_Threading",
  "Win32_UI_Shell",
] }
```

- [ ] **Step 4: Run focused tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_firewall`

Expected: evaluator and live read-only inspection tests pass without changing firewall state.

- [ ] **Step 5: Commit the inspection layer**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/modules/mod.rs src-tauri/src/modules/capture_firewall.rs
git commit -m "feat: inspect Windows LAN capture permission"
```

### Task 2: Explicit one-click elevated repair and server-side gate

**Files:**
- Modify: `src-tauri/src/modules/capture_firewall.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/commands/capture_lan.rs`
- Modify: `src-tauri/src/bindings.rs`
- Regenerate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `capture_lan_preflight` Tauri command returning `AppResult<CaptureLanPreflight>`.
- Produces: `capture_lan_firewall_repair` Tauri command returning the refreshed `AppResult<CaptureLanPreflight>`.
- Produces: `capture_lan_open_network_settings` returning `AppResult<bool>`.
- Produces: `run_capture_firewall_helper_if_requested() -> Option<i32>` used by `main` before Tauri startup.
- Consumes: hard-coded CLI switch `--configure-capture-firewall`.

- [ ] **Step 1: Add failing command and binding contract tests**

Assert the generated binding source contains `captureLanPreflight`, `captureLanFirewallRepair`, `captureLanOpenNetworkSettings`, and `CaptureLanPreflight`. Add a pure argument parser test proving all arguments except the exact hard-coded helper switch are ignored.

- [ ] **Step 2: Run tests and confirm the new contracts are absent**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_firewall`

Run: `corepack pnpm test -- src/shared/api/bindings.test.ts`

Expected: both suites fail on absent helper/commands.

- [ ] **Step 3: Implement elevated helper mode**

The normal process calls `ShellExecuteExW` with verb `runas`, the current executable, and only `--configure-capture-firewall`; wait for the child and read its exit code. The elevated process initializes COM, replaces only `Mistake Trainer Next - Mobile Capture (Private)`, and creates an `INetFwRule` with:

```text
ApplicationName = current executable
Direction = Inbound
Action = Allow
Protocol = TCP
Profiles = Private
RemoteAddresses = LocalSubnet
Enabled = true
EdgeTraversal = false
```

Cancellation maps to a non-retry loop message: “没有更改 Windows 权限；需要时可再次点击修复连接。” Other failures include a diagnostic ID but never expose a command or executable path.

- [ ] **Step 4: Enforce preflight in `capture_lan_start`**

Before binding the random port, return `capture_lan_public_network` if no Private/Domain profile is active and `capture_lan_firewall_required` if the exact rule is missing/invalid. This prevents stale UI or direct IPC calls from generating an unusable QR.

- [ ] **Step 5: Implement settings launch**

Use `ShellExecuteW` for `ms-settings:network-status`; do not change network category. The command only opens the Windows page and returns whether the launch was accepted.

- [ ] **Step 6: Regenerate bindings and run command tests**

Run: `.\scripts\cargo-msvc.cmd run --manifest-path src-tauri\Cargo.toml -- --export-bindings`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_firewall`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test bindings_contract`

Expected: all pass and the generated file has no drift.

- [ ] **Step 7: Commit repair and typed commands**

```bash
git add src-tauri/src/main.rs src-tauri/src/modules/capture_firewall.rs src-tauri/src/commands/capture_lan.rs src-tauri/src/bindings.rs src/shared/api/bindings.ts
git commit -m "feat: add one-click private firewall repair"
```

### Task 3: Guided Vue permission state machine

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: `CaptureLanPreflight` and the three commands from Task 2.
- Produces props: `lanPreflight: CaptureLanPreflight | undefined`, `lanPreflightBusy: boolean`.
- Produces events: `refreshLanPreflight`, `repairLanFirewall`, `openLanNetworkSettings`.

- [ ] **Step 1: Write failing UI tests**

Cover these exact states:

```ts
it('does not offer QR generation on a public-only network')
it('repairs a missing private firewall rule from one button')
it('offers QR generation only when network and rule are ready')
it('keeps public access explicitly disabled in all permission copy')
```

The public-only test clicks “打开网络设置” and expects only `openLanNetworkSettings`. The missing-rule test clicks “修复连接” and expects `repairLanFirewall`. Neither may emit `mobileCapture`.

- [ ] **Step 2: Run the component test and verify failures**

Run: `corepack pnpm test -- src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: failures on absent props, buttons, and events.

- [ ] **Step 3: Implement the dialog states**

When the panel opens, refresh both addresses and preflight. Render:

- public-only: “当前网络被 Windows 标记为公用网络”; primary button “打开网络设置”; no QR button;
- missing/invalid rule: explain the UAC prompt; primary button “修复连接”; no QR button;
- repair busy: disabled “正在请求 Windows 权限…”;
- ready: trusted-network reminder, interface selector, and “生成二维码”;
- unavailable: non-destructive diagnostic copy and retry button.

Never render a terminal command, executable path, or advice to allow Public networks.

- [ ] **Step 4: Implement view command effects**

`repairLanFirewall` calls the typed repair command, refreshes preflight and addresses on success, and leaves the dialog open after UAC cancellation. `startMobileCapture` refreshes preflight immediately before invoking the start command so a network switch cannot bypass the gate.

- [ ] **Step 5: Run UI, lint, and type checks**

Run: `corepack pnpm test -- src/modules/capture/components/CaptureWorkspace.test.ts`

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Expected: all pass.

- [ ] **Step 6: Commit the guided UI**

```bash
git add src/app/views/CaptureView.vue src/modules/capture/components/CaptureWorkspace.vue src/modules/capture/components/CaptureWorkspace.test.ts
git commit -m "feat: guide Windows LAN permission recovery"
```

### Task 4: Acceptance documentation and full verification

**Files:**
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Consumes: completed app behavior from Tasks 1-3.
- Produces: repeatable acceptance instructions for first allow, UAC cancel, repair, public network, private hotspot, update path, and uninstall backlog.

- [ ] **Step 1: Update acceptance cases**

Document exact expected behavior for: no existing rule; cancel UAC; click repair again; Public-only active profile; switch to a personal hotspot marked Private; successful QR GET; and rule validation after an executable path change. State that installer creation/removal of the same named rule remains part of Release phase when bundling is enabled.

- [ ] **Step 2: Run complete quality gates**

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Run: `corepack pnpm test`

Run: `corepack pnpm build`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test bindings_contract`

Expected: all pass. Existing OpenSSL PDB and SQLCipher `VirtualLock` warnings may remain warnings; no new warnings are accepted.

- [ ] **Step 3: Perform Windows manual acceptance**

On a Private personal hotspot, remove only the named app allow rule, open the LAN dialog, verify “修复连接”, accept UAC, generate a fresh QR, load it from a phone, and upload one JPEG. Then switch the active profile to Public and confirm the app blocks QR generation and offers network settings instead of suggesting Public access.

- [ ] **Step 4: Commit docs and final fixes**

```bash
git add docs/windows-capture-acceptance.md
git commit -m "docs: add LAN permission recovery acceptance"
```

## Self-Review

- Spec coverage: first-use explanation, denied-prompt recovery, one-click UAC, Public-profile refusal, exact minimal rule, server-side enforcement, typed UI, cancellation, diagnostics, and acceptance are each assigned to a task.
- Security boundary: no Public rule, no network-category mutation, no fixed port, no user-controlled elevated arguments, no removal of Windows block rules, and no terminal instructions in product UI.
- Placeholder scan: every implementation, validation, failure, and test step is concrete.
- Type consistency: `CaptureLanPreflight`, `capture_lan_preflight`, `capture_lan_firewall_repair`, and `capture_lan_open_network_settings` use the same names in Rust, generated TypeScript, Vue, and tests.

