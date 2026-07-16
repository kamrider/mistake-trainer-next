# Windows LAN Guided Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the LAN capture permission dialog into a self-contained Windows walkthrough that tells a non-technical user exactly which Settings controls to click, opens the correct Wi-Fi or Ethernet page, automatically notices completion, and explains UAC repair without terminal commands.

**Architecture:** Keep Windows policy and firewall decisions in Rust. Extend the existing settings launcher with a typed destination enum, then render the network and firewall states as numbered Vue walkthroughs. `CaptureView` temporarily polls the existing read-only preflight after opening Settings so the dialog advances as soon as Windows reports a safe state.

**Tech Stack:** Rust, Windows `ms-settings:` URI scheme, tauri-specta bindings, Vue 3, TypeScript, Vitest, Testing Library.

## Global Constraints

- Never change the Windows network profile automatically.
- Never enable Public firewall access or weaken the existing Private-only gate.
- Never display PowerShell, `netsh`, executable paths, or manual firewall commands.
- Use the Microsoft-documented `ms-settings:network-wifi` and `ms-settings:network-ethernet` destinations.
- Keep the guide usable with keyboard navigation, reduced motion, and narrow desktop windows.

---

### Task 1: Typed Windows Settings destinations

**Files:**
- Modify: `src-tauri/src/modules/capture_firewall.rs`
- Modify: `src-tauri/src/commands/capture_lan.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts`
- Test: `src/shared/api/bindings.test.ts`

**Interfaces:**
- Produces: `CaptureLanSettingsPage = 'overview' | 'wifi' | 'ethernet'` and `captureLanOpenNetworkSettings(page)`.
- Consumes: the existing `AppResult<boolean>` command result.

- [ ] **Step 1: Add a failing binding contract assertion**

```ts
expect(bindings).toContain('CaptureLanSettingsPage')
expect(bindings).toContain('captureLanOpenNetworkSettings: (page: CaptureLanSettingsPage)')
```

- [ ] **Step 2: Verify the binding test fails**

Run: `corepack pnpm exec vitest run src/shared/api/bindings.test.ts`

Expected: FAIL because the settings destination type is absent.

- [ ] **Step 3: Add the enum and URI mapping**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLanSettingsPage { Overview, Wifi, Ethernet }

fn settings_uri(page: CaptureLanSettingsPage) -> &'static str {
    match page {
        CaptureLanSettingsPage::Overview => "ms-settings:network-status",
        CaptureLanSettingsPage::Wifi => "ms-settings:network-wifi",
        CaptureLanSettingsPage::Ethernet => "ms-settings:network-ethernet",
    }
}
```

Pass the enum through `capture_lan_open_network_settings(page)` and use a lifetime-safe UTF-16 buffer with `ShellExecuteExW`.

- [ ] **Step 4: Regenerate bindings and run Rust/binding tests**

Run: `corepack pnpm bindings:generate`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml capture_firewall`

Run: `corepack pnpm exec vitest run src/shared/api/bindings.test.ts`

Expected: all pass and the generated binding accepts the typed page parameter.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/modules/capture_firewall.rs src-tauri/src/commands/capture_lan.rs src-tauri/src/bindings.rs src/shared/api/bindings.ts src/shared/api/bindings.test.ts
git commit -m "feat: open exact Windows network settings"
```

### Task 2: Numbered network and UAC walkthroughs

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Test: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: `CaptureLanPreflight` and `CaptureLanSettingsPage`.
- Produces: `openLanNetworkSettings(page)` and a four-step in-dialog tutorial.

- [ ] **Step 1: Add failing interaction tests**

```ts
expect(screen.getByText('点击当前已连接的 Wi‑Fi 名称')).toBeVisible()
await user.click(screen.getByRole('button', { name: '打开 Wi‑Fi 设置' }))
expect(view.emitted('openLanNetworkSettings')).toEqual([['wifi']])
await user.click(screen.getByRole('button', { name: '网线 / 扩展坞' }))
expect(screen.getByText('进入“网络配置文件类型”')).toBeVisible()
```

Also assert that the UAC state explains clicking **是**, cancellation, and administrator-password fallback.

- [ ] **Step 2: Verify tests fail**

Run: `corepack pnpm exec vitest run src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: FAIL because the walkthrough and typed event do not exist.

- [ ] **Step 3: Implement the walkthrough UI**

Add a Wi-Fi/Ethernet choice, numbered cards, exact Windows labels, a prominent **设置完成，立即检测** button, and a collapsible **还是检测不过？** section covering Public Wi-Fi, VPN/virtual adapters, managed work/school devices, and personal-hotspot fallback. Expand the firewall state with numbered UAC steps and retain the existing explicit repair action.

- [ ] **Step 4: Add responsive and reduced-motion styles**

Use the existing paper/ink tokens. The step cards stack below 720px, keyboard focus remains visible, and no new continuous animation is introduced.

- [ ] **Step 5: Run component, lint, and type tests**

Run: `corepack pnpm exec vitest run src/modules/capture/components/CaptureWorkspace.test.ts`

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Expected: all pass with zero lint warnings.

- [ ] **Step 6: Commit**

```bash
git add src/modules/capture/components/CaptureWorkspace.vue src/modules/capture/components/CaptureWorkspace.test.ts
git commit -m "feat: add guided Windows LAN setup"
```

### Task 3: Automatic completion detection and acceptance documentation

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `docs/windows-capture-acceptance.md`

**Interfaces:**
- Consumes: `openLanNetworkSettings(page)` and `captureLanPreflight()`.
- Produces: a bounded 90-second, read-only polling window that stops when the profile is safe or the view unmounts.

- [ ] **Step 1: Pass the typed destination and start bounded polling**

```ts
async function openLanNetworkSettings(page: CaptureLanSettingsPage) {
  const result = normalizeAppResult(await commands.captureLanOpenNetworkSettings(page))
  if (result.ok) startLanGuidePolling()
}
```

Poll every two seconds, stop after 90 seconds, stop immediately when `needsNetworkChange` becomes false, and clear the timer in `onBeforeUnmount`.

- [ ] **Step 2: Update manual acceptance cases**

Document Wi-Fi, Ethernet, cancellation, managed-device fallback, mixed Public/Private adapters, and automatic dialog advancement after returning from Settings.

- [ ] **Step 3: Run full quality gates**

Run: `corepack pnpm lint`

Run: `corepack pnpm typecheck`

Run: `corepack pnpm test`

Run: `corepack pnpm build`

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Run: `corepack pnpm bindings:check`

Run: `corepack pnpm tauri build`

Expected: every command exits 0; only the existing OpenSSL PDB linker warning may remain.

- [ ] **Step 4: Commit and push**

```bash
git add src/app/views/CaptureView.vue docs/windows-capture-acceptance.md docs/superpowers/plans/2026-07-15-windows-lan-guided-setup.md
git commit -m "docs: verify guided Windows network setup"
git push origin feature/capture-library
```
