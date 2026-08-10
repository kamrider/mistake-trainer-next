# Windows commercial release runbook

This runbook is the production gate for the paid Windows build. A locally built or CI-built
unsigned executable is a development artifact, not a product release.

## Release contract

- Publish separately labeled Windows x64 and native ARM64 installers under the scope in
  [windows-support-policy.md](windows-support-policy.md).
- Ship the per-user NSIS installer with the offline Evergreen WebView2 prerequisite.
- Sign both the application executable and installer with the same trusted code-signing
  certificate, SHA-256 digest, and an HTTPS RFC 3161 timestamp.
- Sign every updater payload with the separate Tauri updater key. Publish one `latest.json`
  containing both `windows-x86_64` and `windows-aarch64`; a partial manifest is not releasable.
- Keep automatic update networking disabled in ordinary/local builds. Production builds receive
  the HTTPS manifest endpoint and updater public key only through the protected release environment.
- Never publish directly from CI. The workflow creates a draft GitHub Release for a human to
  review and promote.
- Build only an annotated `vX.Y.Z` tag that resolves to the current `origin/main` commit after a
  successful completed CI `push` run for that exact SHA. The signed workflow verifies all four
  facts before either architecture receives signing credentials.
- Keep a previous signed installer available for rollback. Downgrades are blocked by the
  installer, so rollback requires an explicit uninstall/reinstall procedure and a verified
  encrypted backup.

Authoritative platform references:

- [Tauri Windows installers](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Windows code signing](https://v2.tauri.app/distribute/sign/windows/)
- [Tauri updater signatures and static manifests](https://v2.tauri.app/plugin/updater/)
- [Microsoft WebView2 distribution](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution)
- [Microsoft Authenticode timestamping](https://learn.microsoft.com/windows/win32/seccrypto/time-stamping-authenticode-signatures)

## Installed-client behavior

- A production Windows build schedules a non-blocking update check 1.5 seconds after the first
  application frame. Browser previews, ordinary builds without updater configuration, and
  offline launches do not contact the release endpoint.
- The per-device key `mistake-trainer.update.lastAutomaticCheckUtcMs.v1` limits automatic checks
  to one attempt every 24 hours. A failed automatic check remains silent; Settings always keeps
  an explicit manual retry path. If the attempt timestamp cannot be persisted, the automatic
  endpoint request fails closed and the manual Settings action remains available.
- Only newer update metadata accepted by Tauri's manifest and version checks opens the dialog.
  The user must choose **Update now**; there is no background download, forced update,
  ignored-version state, or silent install. Cryptographic payload verification happens after
  download and before installation.
- Installation rechecks the exact displayed version and verifies the Tauri updater signature.
  Version races trigger one fresh check instead of installing the stale result.

## One-time GitHub environment setup

Before changing repository visibility, run a history-aware scanner such as `gitleaks git --redact
--no-banner --log-opts="--all" .` from a clean clone and require zero findings. Also inspect
GitHub Actions logs and workflow artifacts for PFX files, updater private keys, service-role keys,
and credential-bearing URLs. Do not make the repository public until every finding is removed
from history or the exposed credential has been revoked and rotated.

Create a protected `production` environment with required reviewers. Configure:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX secret;
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX password secret;
- `WINDOWS_CERTIFICATE_THUMBPRINT`: expected signer thumbprint secret;
- `WINDOWS_TIMESTAMP_URL`: HTTPS RFC 3161 timestamp service environment variable.
- `WINDOWS_UPDATER_PUBLIC_KEY`: public Tauri updater verification key;
- `TAURI_SIGNING_PRIVATE_KEY`: Tauri updater private key secret;
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: updater private-key password secret.

The updater key pair is independent from the Authenticode certificate. The public verification
key is safe to distribute and is compiled into production builds; the private key and its
password must never enter the repository, workflow artifacts, issue text, or support diagnostics.
Back up the private key offline: losing it prevents shipping trusted updates to already-installed
clients. Prefer a hardware-backed or managed Authenticode signing service when operationally
available, and rotate credentials only through a separate audited migration procedure.

The repository must be public before a release is promoted. The workflow derives the installed
client endpoint as `https://github.com/<owner>/<repository>/releases/latest/download/latest.json`
from GitHub's trusted `GITHUB_REPOSITORY` value, and writes immutable tag-scoped installer URLs
into the manifest. No manually maintained update URL or credential-bearing download URL is used.
Public repository visibility does not grant an open-source license; adding a software license is
a separate product-owner decision.

## Prepare a release

1. Update the same semantic version in `package.json`, `src-tauri/Cargo.toml`, and
   `src-tauri/tauri.conf.json`.
2. Run all repository gates and the unsigned installer smoke test on a clean Windows machine.
   The smoke must report runtime readiness, create a real WebView2 window, keep it alive for
   10 seconds, prove a second launch hands off to the first instance, and close without producing
   `startup-failure.json`.
3. Merge only reviewed changes to `main` and wait for the CI `push` run on that exact commit to
   complete successfully.
4. Create and push an annotated `vX.Y.Z` tag from that current `main` commit.
5. Wait for **Signed Windows Release**. Missing secrets, certificate mismatch, non-HTTPS
   timestamping/update URLs, version drift, incomplete architecture artifacts, invalid
   Authenticode/updater signatures, or install/self-check/uninstall failure all block the job.
6. Download the workflow artifact. Verify both installer SHA-256 files, inspect
   `Get-AuthenticodeSignature` for `Valid` status and the expected publisher, and confirm
   `latest.json` contains exactly the x64 and ARM64 entries with the intended public URLs.
7. Complete the manual matrix below, including an upgrade from the previous signed version, then
   publish the draft GitHub Release as a non-prerelease release.
8. After publication, confirm the anonymous `releases/latest/download/latest.json` request and
   both immutable installer URLs return HTTP 200, then verify their checksums and signatures.
   Drafts, prereleases, private repository assets, and manifests pointing to inaccessible assets
   are not valid production update sources.

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
