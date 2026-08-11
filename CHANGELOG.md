# Changelog

All notable product changes are recorded in this file. Versions follow Semantic Versioning.

## 0.1.0 — 2026-08-10

First stable public Windows release using the no-cost GitHub distribution channel.

### Included

- Added six canonical mistake-reason tags, advanced library filters, and atomic bulk subject/tag
  editing with ownership-safe validation and synchronization outbox updates.
- Added persisted five-minute, ten-problem, and recently-forgotten quick review sessions with
  deterministic due-first selection and optional subject/tag filters.
- Added sparse-evidence weak-area summaries and a local-calendar seven-day due forecast to the
  learning report.
- Added on-demand, local-only capture quality checks, reversible suggested crops, and explicit
  four-corner perspective correction while retaining immutable encrypted source images.
- Optional startup update checks run at most once every 24 hours and stay silent when offline,
  unconfigured, current, or temporarily unavailable.
- New-version prompts require an explicit user choice and preserve the existing exact-version,
  Rust-owned install and signature-verification flow.
- Public GitHub Releases provide immutable x64 and native ARM64 installer URLs, checksums, Tauri
  updater signatures, and one dual-architecture `latest.json` manifest.

### Distribution security

- Windows installers and application executables are intentionally not Authenticode-signed;
  Windows may display Unknown publisher or Microsoft Defender SmartScreen warnings.
- Draft and published release pages must disclose that warning and must not describe the build as
  a verified Windows publisher.
- Every updater payload remains cryptographically signed with the protected Tauri updater key.
  Release CI verifies each installer/signature pair against the exact public key compiled into
  installed clients before any artifact is uploaded.
- The free channel uses public GitHub standard runners and GitHub Releases. It requires no paid
  certificate, timestamping account, runner, or download host.

## 0.1.0-rc.1 — 2026-08-08

First Windows release candidate for the commercial-quality baseline.

### Included

- Offline-first encrypted library and encrypted capture assets.
- Capture, import, organization, review, history, reporting, and export workflows.
- Validated backup creation, atomic restore, startup recovery, and legacy-library import.
- Optional cloud account and synchronization with conflict review and offline-safe behavior.
- Privacy-safe startup diagnostics and user-exported diagnostic reports.
- Per-user NSIS packaging, offline WebView2 delivery, single-instance handling, and Windows compatibility checks.
- Review-required local question splitting with conservative fallback behavior; recognition output is never treated as final without user confirmation.

### Release-candidate status

- The repository gates and local x64 installer smoke must pass on the exact candidate commit.
- The candidate evaluated an Authenticode requirement; the stable `0.1.0` release intentionally
  replaced it with the documented free unsigned-installer channel while retaining updater signing.
- The supported Windows, DPI, offline, storage-pressure, sleep/resume, antivirus, and upgrade
  matrix must be recorded before public production release.

