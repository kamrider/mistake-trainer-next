# Mistake Trainer Next

Windows-first, offline-first mistake training desktop app built with Vue 3 and Tauri.

## Status

The walking skeleton, encrypted local library, review loop, and Capture Inbox Next are
implemented. Capture supports persistent desktop batch organizing and an explicitly
started QR phone session on trusted private networks. The legacy Electron application
is a read-only migration fixture and is never modified by this repository.

## Commands

```powershell
corepack pnpm install
corepack pnpm dev
corepack pnpm test
corepack pnpm typecheck
corepack pnpm mobile:vendor:check
corepack pnpm tauri dev
```

The `tauri` script enters the repository-managed MSVC, Perl, and OpenSSL build
environment automatically; no global Perl installation is required.

See [docs/architecture.md](docs/architecture.md) for boundaries and invariants.
See [docs/windows-capture-acceptance.md](docs/windows-capture-acceptance.md) for the
phone and desktop acceptance walkthrough.
See [docs/cloud-regional-deployment.md](docs/cloud-regional-deployment.md) for the
mainland network and cloud-provider boundary.
See [docs/windows-support-policy.md](docs/windows-support-policy.md) for the
commercial Windows support matrix, installer prerequisites, and release evidence.
