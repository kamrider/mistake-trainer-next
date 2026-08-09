# Changelog

All notable product changes are recorded in this file. Versions follow Semantic Versioning.

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
- Public promotion requires Authenticode-signed and updater-signed x64 and ARM64 artifacts from the protected GitHub workflow.
- The supported Windows, DPI, offline, storage-pressure, sleep/resume, antivirus, and upgrade matrix must be recorded before a paid production release.

