# Mistake Trainer Next

Windows-first, offline-first mistake training desktop app built with Vue 3 and Tauri.

## Status

Foundation and walking-skeleton implementation are in progress. The legacy Electron
application is a read-only migration fixture and is never modified by this repository.

## Commands

```powershell
corepack pnpm install
corepack pnpm dev
corepack pnpm test
corepack pnpm typecheck
corepack pnpm tauri dev
```

The `tauri` script enters the repository-managed MSVC, Perl, and OpenSSL build
environment automatically; no global Perl installation is required.

See [docs/architecture.md](docs/architecture.md) for boundaries and invariants.
