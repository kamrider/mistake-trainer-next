# Commercial Windows Runtime Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the installed Windows product prove that WebView2 and the real desktop startup path work, and leave a privacy-safe support record when startup or a Rust panic prevents normal use.

**Architecture:** Keep OS support classification separate from runtime readiness. A versioned self-check report combines the existing Windows classification with fixed prerequisite failure codes; the installer smoke then launches the real GUI twice to prove WebView2 startup and single-instance behavior. Startup failure records use a closed reason-code enum and are read into the local diagnostic export without copying panic text, file paths, user content, or machine identity.

**Tech Stack:** Rust 1.97, Tauri 2.11, serde/serde_json, Windows PowerShell 5.1, NSIS, Vitest, GitHub Actions.

## Global Constraints

- Official support remains current Microsoft-supported Windows 11 x64.
- Extended compatibility remains Windows 10 22H2 x64 with ESU, supported Windows 10 LTSC x64, and native Windows 11 ARM64.
- Windows 7, Windows 8/8.1, 32-bit Windows, Windows Server, Wine, and modified Windows images remain unsupported.
- The consumer installer remains per-user and must not require administrator access for the application itself.
- The installer must carry the Evergreen WebView2 offline installer and the installed application must detect a usable WebView2 Runtime.
- Public reports may contain only fixed reason codes, application/platform versions, timestamps, and bounded aggregates; never panic text, paths, usernames, question content, image metadata, credentials, endpoints, or machine identifiers.
- Startup and panic reporting must be best-effort and must never replace, delete, or mutate the encrypted library.
- The self-check output path remains an absolute caller-provided path; it must never be inferred from page state.
- Windows PowerShell scripts must run under Windows PowerShell 5.1 as well as PowerShell 7.
- x64 and ARM64 installer jobs must run the same readiness and real-startup contract.

---

### Task 1: Versioned Windows Runtime Readiness

**Files:**
- Modify: `src-tauri/src/modules/startup_safety.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/startup_safety.rs`
- Modify: `src-tauri/tests/windows_compatibility.rs`

**Interfaces:**
- Consumes: `WindowsCompatibilityStatus`, including `support_level` and `webview2_version`.
- Produces: `WindowsSelfCheckReport`, schema version `2`, `ready: bool`, and `failure_codes: Vec<WindowsSelfCheckFailureCode>`.

- [ ] **Step 1: Write failing self-check readiness tests**

Add a deterministic report builder and test it with explicit compatibility fixtures:

```rust
#[test]
fn self_check_requires_supported_windows_and_webview2() {
    let ready = build_windows_self_check_report("1.2.3", 100, supported_windows(Some("150.0")));
    assert!(ready.ready);
    assert!(ready.failure_codes.is_empty());

    let missing_runtime =
        build_windows_self_check_report("1.2.3", 100, supported_windows(None));
    assert!(!missing_runtime.ready);
    assert_eq!(
        missing_runtime.failure_codes,
        vec![WindowsSelfCheckFailureCode::Webview2RuntimeMissing]
    );

    let unsupported =
        build_windows_self_check_report("1.2.3", 100, unsupported_windows(Some("150.0")));
    assert!(!unsupported.ready);
    assert_eq!(
        unsupported.failure_codes,
        vec![WindowsSelfCheckFailureCode::WindowsUnsupported]
    );
}
```

Serialize the missing-runtime case and assert schema version `2`, `ready: false`, and the exact public code `webview2_runtime_missing`.

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test startup_safety --test windows_compatibility
```

Expected: compilation fails because the report builder and failure-code enum do not exist.

- [ ] **Step 3: Implement the closed readiness contract**

In `startup_safety.rs`, define:

```rust
pub const SELF_CHECK_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsSelfCheckFailureCode {
    WindowsUnsupported,
    Webview2RuntimeMissing,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsSelfCheckReport {
    pub schema_version: u32,
    pub application_version: String,
    pub checked_at_utc_ms: i64,
    pub ready: bool,
    pub failure_codes: Vec<WindowsSelfCheckFailureCode>,
    pub windows: WindowsCompatibilityStatus,
}
```

`build_windows_self_check_report` must append `WindowsUnsupported` only for `Unsupported` OS classification and append `Webview2RuntimeMissing` only when `webview2_version` is absent. `write_windows_self_check` writes the complete report and returns `Ok(report.ready)`.

Change `main.rs` so `--windows-self-check` exits `0` only for `Ok(true)`, `10` for `Ok(false)`, and `11` for invalid input or write failure.

- [ ] **Step 4: Run focused tests**

Run the command from Step 2.

Expected: all startup-safety and Windows compatibility tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/modules/startup_safety.rs src-tauri/src/main.rs src-tauri/tests/startup_safety.rs src-tauri/tests/windows_compatibility.rs
git commit -m "feat: make Windows self-check verify runtime readiness"
```

---

### Task 2: Sanitized Panic and Startup Failure Record

**Files:**
- Modify: `src-tauri/src/modules/startup_safety.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/startup_safety.rs`

**Interfaces:**
- Consumes: the fixed application data root returned by `default_application_data_root()`.
- Produces: `StartupFailureReason`, `write_startup_failure_record`, `read_startup_failure_record`, and `install_panic_recording_hook`.

- [ ] **Step 1: Write failing record safety tests**

Add tests that write both reasons, replace a forged record containing a private path, and reject malformed or oversized input:

```rust
#[test]
fn startup_failure_reasons_are_fixed_and_private() {
    let directory = tempfile::tempdir().unwrap();
    write_startup_failure_record(
        directory.path(),
        "1.2.3",
        100,
        StartupFailureReason::RustPanic,
    )
    .unwrap();

    let record = read_startup_failure_record(directory.path()).unwrap().unwrap();
    assert_eq!(record.reason_code, StartupFailureReason::RustPanic);
    let serialized = serde_json::to_string(&record).unwrap();
    assert!(!serialized.contains("Users"));
    assert!(!serialized.contains("panic message"));
}
```

The existing atomic replacement test must continue to prove that only one final file remains.

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test startup_safety
```

Expected: compilation fails because the reason enum and reader do not exist.

- [ ] **Step 3: Implement fixed reason recording**

Define:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupFailureReason {
    TauriStartupFailed,
    RustPanic,
}
```

Keep the record bounded to schema version, application version, UTC timestamp, and the enum. `read_startup_failure_record` must read no more than 4096 bytes, require the current schema, and return `Ok(None)` for absent, malformed, oversized, or unknown-code records.

`install_panic_recording_hook` captures only the application-data root and application version. Its closure writes `RustPanic` with the current timestamp and then calls the previous hook; it must not serialize `PanicHookInfo`.

Install the hook only on the normal application path after helper/self-check/binding CLI modes have returned. Pass `TauriStartupFailed` when `run()` returns an error.

- [ ] **Step 4: Run focused tests**

Run the command from Step 2.

Expected: every startup-safety test passes and serialized records contain no panic message or path.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/modules/startup_safety.rs src-tauri/src/main.rs src-tauri/tests/startup_safety.rs
git commit -m "feat: record sanitized Windows startup failures"
```

---

### Task 3: Include Fixed Startup Evidence in Diagnostic Export

**Files:**
- Modify: `src-tauri/src/modules/diagnostics.rs`
- Modify: `src-tauri/src/commands/diagnostics.rs`
- Modify: `src-tauri/tests/diagnostics.rs`
- Modify: `docs/windows-support-policy.md`

**Interfaces:**
- Consumes: `read_startup_failure_record(&control_root)`.
- Produces: diagnostic schema version `3`, optional `application.lastStartupFailure`, and warning code `previous_startup_failure_detected`.

- [ ] **Step 1: Write failing privacy and schema tests**

Extend the real encrypted-database diagnostic test to pass a fixed startup summary and require:

```json
{
  "schemaVersion": 3,
  "application": {
    "lastStartupFailure": {
      "applicationVersion": "1.2.3",
      "occurredAtUtcMs": 100,
      "reasonCode": "rust_panic"
    }
  },
  "warnings": [
    { "code": "previous_startup_failure_detected" }
  ]
}
```

Keep the existing sentinel scan and add panic text, an absolute Windows path, and a username as forbidden sentinels.

- [ ] **Step 2: Run the diagnostic test and verify failure**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test diagnostics
```

Expected: assertions fail because schema version `2` has no startup failure summary.

- [ ] **Step 3: Add the optional fixed summary**

Add `startup_failure: Option<&StartupFailureRecord>` to `DiagnosticContext`. Serialize an owned, fixed-field copy under `DiagnosticApplication.last_startup_failure`; append `previous_startup_failure_detected` when present.

In the command, read the record from `control_root` before exporting. Treat absent, malformed, or unreadable records as `None`; diagnostic export must remain available even if the support marker is damaged.

Update the support policy to state that exported startup evidence contains only a fixed reason code, application version, and timestamp.

- [ ] **Step 4: Run diagnostic and command contract tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test diagnostics --test command_contract
corepack pnpm bindings:check
```

Expected: tests pass and generated TypeScript bindings are unchanged because the report body remains file-only.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/modules/diagnostics.rs src-tauri/src/commands/diagnostics.rs src-tauri/tests/diagnostics.rs docs/windows-support-policy.md
git commit -m "feat: include fixed startup evidence in diagnostics"
```

---

### Task 4: Real Installed GUI and Single-Instance Smoke

**Files:**
- Modify: `scripts/windows-installer-smoke.ps1`
- Modify: `docs/windows-release-runbook.md`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: self-check schema version `2`, the installed executable, and the existing per-user NSIS uninstaller.
- Produces: x64 and ARM64 evidence for install, runtime readiness, real WebView2 startup, single-instance behavior, and uninstall.

- [ ] **Step 1: Tighten the self-check assertions**

Require:

```powershell
Assert-Smoke ($selfCheck.schemaVersion -eq 2) 'unexpected self-check schema version.'
Assert-Smoke ($selfCheck.ready -eq $true) 'installed runtime readiness was false.'
Assert-Smoke (@($selfCheck.failureCodes).Count -eq 0) 'installed runtime readiness reported failures.'
Assert-Smoke (-not [string]::IsNullOrWhiteSpace($selfCheck.windows.webview2Version)) 'WebView2 Runtime was not detected after installation.'
```

- [ ] **Step 2: Add isolated real-startup and second-launch checks**

Before launching, save `APPDATA` and `LOCALAPPDATA`, then point both to directories inside `$smokeRoot`. Start the installed executable and require it to remain alive for ten seconds. Start the executable a second time, require that process to exit within ten seconds with code `0`, and require the first process to remain alive.

Stop only the exact first process, wait for exit, then assert that the isolated application-data directory contains no `startup-failure.json`. Restore both environment variables in `finally` before removing the smoke root.

- [ ] **Step 3: Verify under Windows PowerShell 5.1 locally**

Run:

```powershell
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-release-contract.ps1
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-installer-smoke.ps1 -InstallerDirectory src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis -ExpectedArchitecture x86_64
```

Expected: release contract and full installed GUI smoke pass.

- [ ] **Step 4: Document and gate both architectures**

Update the runbook so the automated evidence explicitly includes actual WebView2 startup and duplicate-launch focus behavior. Keep both existing CI jobs calling the shared smoke script; no architecture-specific bypass is allowed.

- [ ] **Step 5: Run repository gates**

Run:

```powershell
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
corepack pnpm bindings:check
git diff --check
```

Expected: all commands exit `0`. Existing SQLCipher `VirtualLock` and OpenSSL missing-PDB messages may remain non-fatal environment warnings.

- [ ] **Step 6: Commit**

```powershell
git add scripts/windows-installer-smoke.ps1 docs/windows-release-runbook.md .github/workflows/ci.yml
git commit -m "test: prove installed Windows GUI startup"
```

---

## Self-Review

- Spec coverage: the plan closes false-positive runtime readiness, missing real WebView2 launch evidence, missing single-instance CI evidence, unrecorded Rust panic/startup failures, and the absent support-record-to-diagnostic-export link.
- Deliberate split: signed automatic updating is a separate release-channel plan because it requires an updater signing key, a stable HTTPS endpoint, rollback/channel policy, and customer-entitlement decisions. This plan does not commit an unusable public key or pretend a private GitHub release is a customer update CDN.
- Placeholder scan: every change step defines exact fields, codes, commands, and expected results; no implementation placeholder remains.
- Type consistency: `WindowsSelfCheckFailureCode`, `WindowsSelfCheckReport`, `StartupFailureReason`, `StartupFailureRecord`, and `previous_startup_failure_detected` retain the same spelling across producer, consumer, tests, scripts, and documentation.
