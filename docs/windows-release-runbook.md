# Windows commercial release runbook

This runbook is the production gate for the paid Windows build. A locally built or CI-built
unsigned executable is a development artifact, not a product release.

## Release contract

- Publish separately labeled Windows x64 and native ARM64 installers under the scope in
  [windows-support-policy.md](windows-support-policy.md).
- Ship the per-user NSIS installer with the offline Evergreen WebView2 prerequisite.
- Sign both the application executable and installer with the same trusted code-signing
  certificate, SHA-256 digest, and an HTTPS RFC 3161 timestamp.
- Never publish directly from CI. The workflow creates a draft GitHub Release for a human to
  review and promote.
- Keep a previous signed installer available for rollback. Downgrades are blocked by the
  installer, so rollback requires an explicit uninstall/reinstall procedure and a verified
  encrypted backup.

Authoritative platform references:

- [Tauri Windows installers](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Windows code signing](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft WebView2 distribution](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution)
- [Microsoft Authenticode timestamping](https://learn.microsoft.com/windows/win32/seccrypto/time-stamping-authenticode-signatures)

## One-time GitHub environment setup

Create a protected `production` environment with required reviewers. Configure:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX secret;
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX password secret;
- `WINDOWS_CERTIFICATE_THUMBPRINT`: expected signer thumbprint secret;
- `WINDOWS_TIMESTAMP_URL`: HTTPS RFC 3161 timestamp service environment variable.

Do not store any of these in the repository, workflow artifacts, issue text, or support
diagnostics. Prefer a hardware-backed or managed signing service when operationally available.
Rotate credentials through a separate audited procedure.

## Prepare a release

1. Update the same semantic version in `package.json`, `src-tauri/Cargo.toml`, and
   `src-tauri/tauri.conf.json`.
2. Run all repository gates and the unsigned installer smoke test on a clean Windows machine.
   The smoke must report runtime readiness, create a real WebView2 window, keep it alive for
   10 seconds, prove a second launch hands off to the first instance, and close without producing
   `startup-failure.json`.
3. Merge only reviewed changes to `main`.
4. Create and push an annotated `vX.Y.Z` tag from the reviewed commit.
5. Wait for **Signed Windows Release**. Missing secrets, certificate mismatch, non-HTTPS
   timestamping, version drift, invalid signatures, or install/self-check/uninstall failure all
   block the job.
6. Download the workflow artifact. Verify the installer SHA-256 locally and inspect
   `Get-AuthenticodeSignature` for `Valid` status and the expected publisher.
7. Complete the manual matrix below, then publish the draft GitHub Release.

## Required manual matrix

Test with standard, non-administrator user accounts and Unicode Windows user names where
possible:

| Environment | Required checks |
| --- | --- |
| Windows 11 x64, current release | fresh install, launch, second-launch focus, capture, backup, uninstall |
| Windows 11 x64, 125%, 150%, 200% DPI | primary workflows, dialogs, drag/drop, no clipped controls |
| Windows 10 22H2 ESU or supported LTSC x64 | install, extended-support notice, core local workflows |
| Windows 11 ARM64 native | ARM64 installer, native self-check, capture/backup and optional recognition performance |
| Windows 11 ARM64 x64 emulation | migration-only check; direct users to the native installer |
| Offline machine | installer completes with bundled WebView2 prerequisite and app launches |
| Antivirus/SmartScreen test machine | signed publisher is correct; record any warning or quarantine |
| Upgrade from previous signed version | encrypted database/assets preserved; no downgrade allowed |

Also verify sleep/resume, reboot, a network outage during optional cloud sync, 150-image capture,
1 GB temporary capture data, low disk space, and a read-only or unavailable custom storage
location. Attach results to the release ticket; do not upload user content or private paths.

## Support triage

Ask the customer for:

1. app version, Windows edition/display version/build, native architecture, and WebView2 version
   shown in Settings;
2. the generated **安全诊断报告**;
3. whether the installer signature shows the expected publisher;
4. exact reproducible actions, without sharing question images unless the customer explicitly
   consents.

The startup failure record under the app's application-data category contains only schema
version, app version, timestamp, and one of the fixed reason codes `tauri_startup_failed` or
`rust_panic`. It never contains panic text, an internal error, stack trace, database path,
account identity, or question content. A valid record is included automatically when the user
exports a safe diagnostic report.
