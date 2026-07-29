# Windows Signed Updates Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Add a production-safe Windows updater that is disabled in ordinary builds, verifies Tauri update signatures, publishes one complete x64/ARM64 static manifest, and gives users an explicit check/install experience with safe failure recovery.

**Architecture:** Keep update networking and installation inside Rust. Release builds inject an HTTPS endpoint and public verification key through a temporary Tauri configuration, while ordinary builds expose a typed `disabled` status and perform no update request. Each architecture job produces an Authenticode-signed NSIS installer, its Tauri `.sig`, and checksum; the aggregation job validates both architecture payloads and creates `latest.json` only after every invariant passes.

**Tech Stack:** Tauri 2.11, `tauri-plugin-updater` 2.10.1, Rust/Tokio, Vue 3/TypeScript, PowerShell 5.1-compatible release scripts, Vitest, Cargo tests, GitHub Actions.

---

## Task 1: Define the fail-closed update command contract

**Files:**
- Create: `src-tauri/src/commands/updates.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Test: `src-tauri/tests/command_contract.rs`
- Test: `tests/repository-contract.test.ts`

**Step 1: Write failing contract tests**

Add Rust serialization tests for:

```rust
WindowsUpdateStatus {
    enabled: false,
    current_version: "0.1.0",
}

WindowsUpdateCheckReport {
    available: true,
    current_version: "0.1.0",
    version: Some("0.2.0"),
    published_at: Some("2026-07-28T00:00:00Z"),
}
```

Assert that no endpoint, public key, local path, signature, remote notes, or raw updater error can occur in the public DTO. Add repository tests requiring the three typed commands and disallowing the JavaScript updater package.

**Step 2: Run the focused tests to verify failure**

Run:

```powershell
corepack pnpm vitest run tests/repository-contract.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
```

Expected: failures because the update module, types, and commands do not exist.

**Step 3: Add the updater dependency and safe DTOs**

Add the exact Windows dependency:

```toml
tauri-plugin-updater = "=2.10.1"
```

Define Specta/Serde types:

```rust
pub struct WindowsUpdateStatus {
    pub enabled: bool,
    pub current_version: String,
}

pub struct WindowsUpdateCheckReport {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub published_at: Option<String>,
}

pub struct WindowsUpdateInstallReceipt {
    pub accepted_version: String,
}
```

**Step 4: Implement commands with closed error codes**

Implement:

- `windows_update_status`
- `windows_update_check`
- `windows_update_install`

Rules:

- Read effective updater configuration from the compiled Tauri config; enabled means exactly one or more HTTPS endpoints and a non-empty public key.
- Disabled status never constructs an updater or makes a network request.
- Check returns only version and RFC 3339 publication time.
- Install re-checks the update and requires the caller’s expected version to match.
- Use a process-wide async mutex to reject duplicate checks/installations.
- Map failures to a fixed code set: `update_disabled`, `update_busy`, `update_check_failed`, `update_version_changed`, `update_install_failed`.
- Log only the generated diagnostic ID and closed code, never the endpoint, key, signature, URL, or raw response.

**Step 5: Mount the plugin and commands**

Mount `tauri_plugin_updater::Builder::new().build()` only on Windows, register the commands in Specta, and keep non-Windows command stubs disabled without updater networking.

**Step 6: Generate bindings and run focused tests**

Run:

```powershell
corepack pnpm bindings:generate
corepack pnpm vitest run tests/repository-contract.test.ts src/shared/api/bindings.test.ts
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
```

Expected: all focused tests pass.

**Step 7: Commit**

```powershell
git add src-tauri/src/commands/updates.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src/shared/api/bindings.ts src-tauri/tests/command_contract.rs tests/repository-contract.test.ts
git commit -m "feat: add fail-closed Windows update commands"
```

## Task 2: Build and validate signed updater artifacts

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/windows-signed-release.ps1`
- Modify: `scripts/windows-release-contract.ps1`
- Create: `scripts/windows-update-manifest.ps1`
- Create: `scripts/windows-update-contract.test.ps1`
- Modify: `.github/workflows/release-windows.yml`
- Modify: `docs/windows-release-runbook.md`

**Step 1: Write failing PowerShell contract tests**

Cover:

- missing endpoint, public key, private key, or signing password blocks production release;
- endpoint must be absolute HTTPS and must not contain credentials;
- ordinary `tauri.conf.json` has `createUpdaterArtifacts: false` and no endpoint/public key;
- both x64 and ARM64 installers, signatures, and checksums are required;
- signature file content must be non-empty and is embedded in JSON, not linked;
- `latest.json` platform keys are exactly `windows-x86_64` and `windows-aarch64`;
- URLs are derived from an explicit HTTPS artifact base URL;
- version must equal the tag and all artifact file names must be unique;
- no secret value is written to generated config or workflow logs except the required public key.

**Step 2: Run the contract test to verify failure**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-update-contract.test.ps1
```

Expected: failure because the manifest generator and updater release fields do not exist.

**Step 3: Make the base configuration inert**

Set:

```json
"bundle": {
  "createUpdaterArtifacts": false
}
```

Do not add an endpoint or key to source-controlled configuration.

**Step 4: Extend the signed release script**

Require production environment values:

- `WINDOWS_UPDATE_ENDPOINT`
- `WINDOWS_UPDATE_ARTIFACT_BASE_URL`
- `WINDOWS_UPDATER_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Generate the temporary override from validated variables:

```powershell
$override = @{
  bundle = @{ createUpdaterArtifacts = $true }
  plugins = @{
    updater = @{
      pubkey = $updaterPublicKey
      endpoints = @($updateEndpoint.AbsoluteUri)
      windows = @{ installMode = 'passive' }
    }
  }
}
```

Verify one installer and one adjacent `.sig`, validate Authenticode on the executable and installer, run installed-GUI smoke, and generate the SHA-256 file. Clean all temporary certificate and configuration material in `finally`.

**Step 5: Implement deterministic manifest generation**

`windows-update-manifest.ps1` accepts a verified artifact directory, release tag, artifact base URL, and output path. It validates both architectures and emits UTF-8 without BOM:

```powershell
$manifest = @{
  version = $releaseVersion
  pub_date = $publicationDateUtc
  platforms = @{
    'windows-x86_64' = @{ signature = $x64Signature; url = $x64ArtifactUrl }
    'windows-aarch64' = @{ signature = $arm64Signature; url = $arm64ArtifactUrl }
  }
}
```

Do not include release notes from an untrusted source.

**Step 6: Extend GitHub Actions**

Pass updater keys/URLs from the protected `production` environment, upload `.sig` files, aggregate exactly six architecture artifacts, generate and validate `latest.json`, then create the draft release with seven files. The production manifest endpoint remains an operator-provided HTTPS value; the workflow never invents an endpoint.

**Step 7: Run both PowerShell versions**

Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-update-contract.test.ps1
pwsh -NoProfile -File .\scripts\windows-update-contract.test.ps1
.\scripts\windows-release-contract.ps1 -ConfigOnly
```

Expected: every command passes on Windows PowerShell 5.1 and PowerShell 7.

**Step 8: Document key custody and rollout**

Document that Tauri updater signing keys are independent from Authenticode, the private key must be backed up offline, losing it prevents updates to installed clients, and the draft release must not be published until `latest.json` and public HTTPS assets are reachable.

**Step 9: Commit**

```powershell
git add src-tauri/tauri.conf.json scripts/windows-signed-release.ps1 scripts/windows-release-contract.ps1 scripts/windows-update-manifest.ps1 scripts/windows-update-contract.test.ps1 .github/workflows/release-windows.yml docs/windows-release-runbook.md
git commit -m "build: publish signed dual-architecture updates"
```

## Task 3: Add the explicit Settings update experience

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/shared/api/bindings.ts`

**Step 1: Write failing UI tests**

Cover:

- disabled builds show “当前安装包未接入自动更新” and never call check;
- enabled builds show current version and an explicit “检查更新” button;
- checking disables duplicate clicks and announces progress;
- up-to-date result is neutral success;
- available result shows the verified version and a separate install button;
- install requires an explicit confirmation click and sends the expected version;
- version-changed and network failures remain retryable without claiming installation;
- no endpoint, key, signature, URL, raw error, or local path is rendered;
- focus returns to the triggering button after recoverable failures.

**Step 2: Run the focused test to verify failure**

Run:

```powershell
corepack pnpm vitest run src/app/views/SettingsView.test.ts
```

Expected: update settings tests fail because the panel is absent.

**Step 3: Implement state and actions**

Load `windowsUpdateStatus` alongside the existing settings calls. Keep separate `checking` and `installing` flags. Normalize every `AppResult`, discard stale available results before each retry, and store only the typed version/date.

**Step 4: Implement accessible UI**

Add an “应用更新” section before diagnostics:

- disabled copy accurately explains manual installer updates;
- enabled copy says updates are signature-verified;
- a live region reports checking/failure/up-to-date states;
- installing copy says the app will close and the passive installer will continue;
- buttons have keyboard focus restoration and no automatic background check.

**Step 5: Run focused UI and type checks**

Run:

```powershell
corepack pnpm vitest run src/app/views/SettingsView.test.ts
corepack pnpm typecheck
corepack pnpm bindings:check
```

Expected: all pass.

**Step 6: Commit**

```powershell
git add src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts src/shared/api/bindings.ts
git commit -m "feat: add explicit signed update experience"
```

## Task 4: Verify the commercial Windows release path

**Files:**
- Modify if required: `docs/windows-acceptance.md`
- Modify if required: `README.md`

**Step 1: Run repository-wide frontend gates**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm bindings:check
```

Expected: all pass with no binding drift.

**Step 2: Run repository-wide Rust gates**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Expected: all targets pass with zero warnings.

**Step 3: Run Windows release contracts**

Run:

```powershell
.\scripts\windows-release-contract.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-update-contract.test.ps1
pwsh -NoProfile -File .\scripts\windows-update-contract.test.ps1
```

Expected: all pass.

**Step 4: Build and smoke an ordinary unsigned local installer**

Run:

```powershell
corepack pnpm tauri build --bundles nsis
.\scripts\windows-installer-smoke.ps1 -InstallerDirectory .\src-tauri\target\release\bundle\nsis -ExpectedArchitecture x86_64
```

Expected: installer and real GUI smoke pass; Settings reports updates disabled and performs no network request.

**Step 5: Perform inline code review**

Review the branch diff for:

- default-deny network behavior;
- secret leakage;
- stale update races;
- version confusion;
- x64/ARM64 manifest completeness;
- PowerShell 5.1 compatibility;
- cleanup of imported certificates and temporary config;
- no regression to local-only operation.

Fix every material finding and rerun the affected gates.

**Step 6: Commit final documentation fixes**

```powershell
git add docs/windows-acceptance.md README.md
git commit -m "docs: define signed update acceptance"
```

Skip this commit if no documentation changes are required.
