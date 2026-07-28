# Commercial Windows Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing Windows-first desktop build into a commercial release baseline with an explicit support contract, offline-safe prerequisites, single-instance protection, actionable compatibility diagnostics, install/uninstall smoke tests, and a fail-closed signed release path.

**Architecture:** Keep the consumer application x64 and per-user, and treat Windows 11 as the primary supported platform. A small Rust compatibility module owns OS/build/architecture classification and feeds both a typed desktop command and the privacy-safe diagnostics export; packaging owns WebView2 delivery and downgrade prevention; GitHub Actions owns clean installer and signing gates.

**Tech Stack:** Tauri 2.11, Rust 1.97, `windows` 0.62, Vue 3, TypeScript, NSIS, PowerShell, GitHub Actions.

## Global Constraints

- Primary support is Windows 11 x64 on Microsoft-supported releases.
- Extended compatibility is Windows 10 22H2 with ESU and supported Windows 10 LTSC x64 releases.
- Windows 7, Windows 8/8.1, 32-bit Windows, Windows Server, Wine, and modified “lite” Windows images are not supported release targets.
- The consumer installer remains per-user and must not require administrator access for the application itself.
- The installer must carry the Evergreen WebView2 offline installer; installation cannot depend on public-network availability.
- The application continues to use the system Evergreen WebView2 runtime so Microsoft security servicing remains effective.
- Production release artifacts must be Authenticode signed with SHA-256 and an RFC 3161 timestamp; unsigned CI packages are test artifacts only.
- No diagnostic output may include account identifiers, learner names, problem content, tags, notes, filenames, database paths, cloud tokens, or signing secrets.
- Existing encrypted databases and assets must remain byte-for-byte compatible.

---

### Task 1: Freeze the Windows support and installer contract

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `docs/windows-support-policy.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: the existing NSIS-only Tauri bundle.
- Produces: a per-user bilingual NSIS installer with offline WebView2 delivery and downgrade rejection.

- [ ] **Step 1: Add a configuration contract test**

Create `scripts/windows-release-contract.ps1` with a `-ConfigOnly` mode that parses `src-tauri/tauri.conf.json` and fails unless:

```powershell
$config.bundle.targets -contains 'nsis'
$config.bundle.windows.allowDowngrades -eq $false
$config.bundle.windows.webviewInstallMode.type -eq 'offlineInstaller'
$config.bundle.windows.nsis.installMode -eq 'currentUser'
$config.bundle.windows.nsis.languages -contains 'SimpChinese'
$config.bundle.windows.nsis.languages -contains 'English'
```

- [ ] **Step 2: Run the contract and verify it fails**

Run:

```powershell
pwsh -File scripts/windows-release-contract.ps1 -ConfigOnly
```

Expected: non-zero exit because the Windows bundle contract is not configured yet.

- [ ] **Step 3: Configure the installer**

Add this Windows bundle configuration:

```json
"windows": {
  "allowDowngrades": false,
  "webviewInstallMode": {
    "type": "offlineInstaller",
    "silent": true
  },
  "nsis": {
    "installMode": "currentUser",
    "languages": ["SimpChinese", "English"],
    "displayLanguageSelector": false,
    "compression": "lzma",
    "installerIcon": "icons/icon.ico"
  }
}
```

- [ ] **Step 4: Document the support matrix**

`docs/windows-support-policy.md` must distinguish:

```text
Supported: Windows 11 x64, current Microsoft-serviced releases.
Extended: Windows 10 22H2 x64 with ESU; supported Windows 10 LTSC x64.
Unsupported: Windows 7/8/8.1, x86, Windows Server, Wine, stripped images.
ARM64: x64 emulation is best-effort until a native ARM64 artifact passes the full matrix.
```

Include installation, WebView2, GPU fallback, custom storage, security-update, and support-report expectations.

- [ ] **Step 5: Re-run the contract**

Run:

```powershell
pwsh -File scripts/windows-release-contract.ps1 -ConfigOnly
```

Expected: exit 0 with `Windows release config contract passed.`

### Task 2: Add native compatibility assessment and safe diagnostics

**Files:**
- Create: `src-tauri/src/modules/windows_compatibility.rs`
- Create: `src-tauri/src/commands/compatibility.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/modules/diagnostics.rs`
- Modify: `src-tauri/src/commands/diagnostics.rs`
- Create: `src-tauri/tests/windows_compatibility.rs`
- Modify: `src-tauri/tests/diagnostics.rs`
- Modify: `src/shared/api/bindings.ts`

**Interfaces:**
- Consumes: `windows::Win32::System::Registry`, `IsWow64Process2`, and the existing `AppResult<T>` command contract.
- Produces: `compatibility_status() -> Result<AppResult<WindowsCompatibilityStatus>, ()>` and diagnostic schema v2.

- [ ] **Step 1: Write classification tests**

Cover these exact cases:

```rust
assert_eq!(assess_windows(22631, "x86_64"), WindowsSupportLevel::Supported);
assert_eq!(assess_windows(19045, "x86_64"), WindowsSupportLevel::Extended);
assert_eq!(assess_windows(17763, "x86_64"), WindowsSupportLevel::Extended);
assert_eq!(assess_windows(17762, "x86_64"), WindowsSupportLevel::Unsupported);
assert_eq!(assess_windows(22631, "x86"), WindowsSupportLevel::Unsupported);
```

- [ ] **Step 2: Verify the tests fail**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test windows_compatibility
```

Expected: compile failure because the module and public types do not exist.

- [ ] **Step 3: Implement the compatibility module**

Define:

```rust
pub const MINIMUM_WINDOWS_BUILD: u32 = 17763;

pub enum WindowsSupportLevel {
    Supported,
    Extended,
    Unsupported,
}

pub struct WindowsCompatibilityStatus {
    pub support_level: WindowsSupportLevel,
    pub supported: bool,
    pub os_name: String,
    pub display_version: String,
    pub build_number: u32,
    pub update_build_revision: u32,
    pub process_architecture: String,
    pub native_architecture: String,
    pub webview2_version: Option<String>,
    pub minimum_windows_build: u32,
    pub summary: String,
}
```

Read only fixed registry values from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` and fixed WebView2 client keys. Unknown values must become `unknown`, never an error containing a path or registry payload.

- [ ] **Step 4: Add the typed command and generated binding**

Register:

```rust
#[tauri::command]
#[specta::specta]
pub async fn compatibility_status() -> Result<AppResult<WindowsCompatibilityStatus>, ()>
```

Then run:

```powershell
corepack pnpm bindings:generate
```

Expected: `compatibilityStatus` appears in `src/shared/api/bindings.ts`.

- [ ] **Step 5: Upgrade private diagnostics**

Set `REPORT_SCHEMA_VERSION` to `2`, add the compatibility status under `application.windows`, and add only fixed warning codes:

```text
windows_release_unsupported
windows_extended_support_only
webview2_runtime_not_detected
```

- [ ] **Step 6: Verify privacy and classification tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test windows_compatibility
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test diagnostics
```

Expected: all tests pass and diagnostic sentinel values remain absent.

### Task 3: Surface compatibility status and protect the local library

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: generated `commands.compatibilityStatus()` and `tauri-plugin-single-instance`.
- Produces: a dismissible extended-support notice, a persistent settings compatibility card, and one active process per Windows user session.

- [ ] **Step 1: Write failing UI tests**

The application test must verify:

```ts
expect(await screen.findByRole('alert')).toHaveTextContent('Windows 10 延伸兼容')
```

The settings test must verify the region name `Windows 兼容状态`, build number, architecture, WebView2 state, and support summary.

- [ ] **Step 2: Add the single-instance plugin**

Pin the matching Tauri 2 plugin and register it before setup:

```rust
.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}))
```

- [ ] **Step 3: Add the compatibility UI**

Load status only in the Tauri runtime. `unsupported` uses an assertive alert; `extended` uses a polite warning; `supported` remains visible in Settings but does not interrupt startup.

- [ ] **Step 4: Run focused tests**

Run:

```powershell
corepack pnpm vitest run src/app/App.test.ts src/app/views/SettingsView.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib
```

Expected: all focused tests pass.

### Task 4: Make startup failures actionable and add an installed self-check

**Files:**
- Create: `src-tauri/src/modules/startup_safety.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/tests/startup_safety.rs`
- Create: `scripts/windows-installer-smoke.ps1`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `current_windows_compatibility()` and the built NSIS installer.
- Produces: `--windows-self-check <output.json>`, a sanitized startup failure record, a native fatal-startup dialog, and install/launch/uninstall CI evidence.

- [ ] **Step 1: Write startup-safety tests**

Verify that a failure record contains only:

```json
{
  "schemaVersion": 1,
  "applicationVersion": "0.1.0",
  "occurredAtUtcMs": 1700000000000,
  "reasonCode": "tauri_startup_failed"
}
```

and that an existing record is atomically replaced without including an error string or filesystem path.

- [ ] **Step 2: Return startup errors instead of panicking**

Change:

```rust
pub fn run() -> Result<(), tauri::Error>
```

and handle the result in `main.rs`. In release builds, write the sanitized record under the application data root and show a Chinese native `MessageBoxW` with the support-file location category, never the internal error.

- [ ] **Step 3: Add the self-check CLI**

Before constructing Tauri, recognize:

```text
--windows-self-check <absolute-output-file>
```

Write the safe compatibility JSON using `create_new`, exit `0` for supported or extended systems, and exit `10` for unsupported systems.

- [ ] **Step 4: Add installer smoke automation**

`scripts/windows-installer-smoke.ps1` must:

1. Require exactly one `*-setup.exe`.
2. Install silently into a unique runner temporary directory.
3. Require the installed executable to exist.
4. Run `--windows-self-check` and require exit code `0`.
5. Parse the JSON and require x64 plus build `>= 17763`.
6. Run the generated uninstaller silently.
7. Require the application executable to be absent.

- [ ] **Step 5: Add the smoke step to CI**

After `pnpm tauri build`, run:

```powershell
pwsh -File scripts/windows-installer-smoke.ps1
```

Expected: install, self-check, and uninstall all pass on the clean Windows runner.

### Task 5: Add a fail-closed signed release path

**Files:**
- Create: `scripts/windows-signed-release.ps1`
- Create: `.github/workflows/release-windows.yml`
- Create: `docs/windows-release-runbook.md`

**Interfaces:**
- Consumes: base64 PFX, PFX password, expected certificate thumbprint, and RFC 3161 timestamp URL from the protected `production` GitHub environment.
- Produces: signed executable and NSIS installer, signature verification evidence, SHA-256 checksum, and a draft GitHub Release.

- [ ] **Step 1: Add release input validation**

The script must fail before building unless all four values are non-empty:

```text
WINDOWS_CERTIFICATE
WINDOWS_CERTIFICATE_PASSWORD
WINDOWS_CERTIFICATE_THUMBPRINT
WINDOWS_TIMESTAMP_URL
```

It must also require the tag version, `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` versions to match.

- [ ] **Step 2: Import and verify the certificate**

Decode the PFX only under `$env:RUNNER_TEMP`, import it into `Cert:\CurrentUser\My`, normalize the thumbprint to uppercase without spaces, and reject a mismatch before the build.

- [ ] **Step 3: Build with a temporary signing override**

Generate a temporary Tauri override containing:

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "<validated thumbprint>",
      "digestAlgorithm": "sha256",
      "timestampUrl": "<validated HTTPS RFC 3161 URL>",
      "tsp": true
    }
  }
}
```

Delete the decoded PFX and remove the certificate from the runner store in a `finally` block.

- [ ] **Step 4: Verify artifacts**

Require `Get-AuthenticodeSignature` status `Valid` for the application executable and installer, require the expected signer thumbprint, run the installer smoke test, and write `<installer>.sha256`.

- [ ] **Step 5: Publish only a draft release**

The tag workflow uses `environment: production`, `contents: write`, uploads the signed installer and checksum as workflow artifacts, and creates a draft GitHub Release. Promotion from draft to public remains a deliberate human action.

### Task 6: Run the commercial Windows release gate

**Files:**
- Modify: `docs/windows-support-policy.md`
- Modify: `docs/windows-release-runbook.md`

**Interfaces:**
- Consumes: all preceding tasks.
- Produces: reproducible evidence for a paid Windows beta.

- [ ] **Step 1: Run static and unit gates**

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm bindings:check
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Expected: every command exits 0.

- [ ] **Step 2: Build and smoke the offline installer**

```powershell
corepack pnpm tauri build
pwsh -File scripts/windows-installer-smoke.ps1
```

Expected: offline WebView2 NSIS installer is generated; silent install, self-check, and uninstall all pass.

- [ ] **Step 3: Record manual matrix evidence**

Document pass/fail evidence for:

```text
Windows 11 23H2/24H2/25H2 x64
Windows 10 22H2 ESU x64
Windows 10 LTSC 2019 x64
Chinese user name and Unicode storage path
standard user without administrator rights
offline installation
upgrade from the previous signed version
uninstall with encrypted user data preserved
125%, 150%, and 200% display scaling
single-instance focus behavior
```

## Self-Review

- Spec coverage: support scope, prerequisite delivery, downgrade safety, diagnostics, single-instance protection, startup failure UX, real installer smoke, signing, checksums, and manual OS/DPI coverage each have an explicit task.
- Scope boundary: native ARM64, Microsoft Store/MSIX, in-app updater hosting, telemetry, crash dump upload, and enterprise MSI are separate release tracks; this plan does not mislabel x64 emulation or unsigned CI artifacts as production support.
- Placeholder scan: every implementation task names exact files, commands, interfaces, and expected outcomes.
- Type consistency: `WindowsSupportLevel`, `WindowsCompatibilityStatus`, `compatibility_status`, diagnostic schema v2, and `--windows-self-check` retain the same names across Rust, TypeScript, UI, scripts, tests, and documentation.
