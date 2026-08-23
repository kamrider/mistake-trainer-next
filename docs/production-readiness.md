# Production readiness

This document separates automatically verifiable release gates from physical acceptance work.
Passing unit tests or installer smoke tests never substitutes for the manual matrix in
`windows-release-runbook.md`.

## Fastest controlled production scope

The minimum production pilot is:

- Windows 11 x64 on a security-supported release;
- offline-first local encrypted library;
- desktop capture, manual organization, library editing, review, backup, restore, and update;
- automatic question-region suggestions only with explicit human review;
- no promise of cloud sync when the build has no configured backend;
- documented Unknown publisher and SmartScreen behavior for the unsigned free channel.

Windows 10, native ARM64, automatic unreviewed question splitting, and hosted synchronization
must not be added to the pilot promise until their applicable evidence is recorded.

## Automated gates

- [x] Version identity is consistent across JavaScript, Cargo, lockfile, and Tauri config.
- [x] Frontend lint, typecheck, regression tests, coverage thresholds, and production build pass locally.
- [x] Rust format, strict Clippy, all-target tests, generated bindings, and architecture contracts pass locally.
- [x] The complete pnpm lockfile, including build/test tooling, has a zero-vulnerability audit.
- [x] The current Rust lockfile has a zero-vulnerability RustSec audit; CI enforces the same gate.
- [x] Full Git history has a recorded zero-finding credential scan.
- [x] GitHub vulnerability alerts, automated security fixes, private reporting, secret scanning, and push protection are enabled.
- [x] Weekly npm, Cargo, Python lab, and GitHub Actions dependency update checks are configured.
- [ ] The exact immutable `0.1.5` commit passes PR and `main` push CI.
- [ ] x64 and native ARM64 installers complete isolated installed-product smoke tests for `0.1.5`.
- [ ] The `0.1.5` updater manifest contains both architectures and every payload has a signature and checksum.

## Required human gates

- [ ] Windows 11 x64 fresh install and the complete create/capture/save/review/restart workflow.
- [ ] Encrypted backup creation, restore, previous-version upgrade, reinstall preservation, and rollback rehearsal.
- [ ] Windows display scaling at 100%, 125%, 150%, and 200% with no clipped critical controls.
- [ ] Offline installation with bundled WebView2 and documented SmartScreen/antivirus handling.
- [ ] Sleep/resume, reboot, low disk, unavailable or read-only storage, and interrupted update recovery.
- [ ] 150-image and 1 GB temporary capture workloads with bounded memory, responsive cancellation, and cleanup.
- [ ] Representative PDF import and DOCX opening in desktop Word.
- [ ] A release owner reviews the evidence ledger and explicitly approves promotion.

## Promotion rule

A candidate is suitable for a controlled pilot only when every automated gate is green and the
Windows 11 x64 rows above pass. It is suitable for the full support matrix only when every
applicable environment and workload row in `windows-release-runbook.md` has recorded evidence.
Any skipped feature must be hidden, disabled, or described as unavailable rather than silently
left in a partially configured state.
