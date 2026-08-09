# Low-conflict Architecture Continuation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue reducing oversized view/use-case responsibilities without overlapping the newly added OCR, recognition-pairing, migration, installer, or release work.

**Architecture:** Move remote sync protocol validation and JSON decoding behind a private child module of `sync_pull`, leaving orchestration and persistence in the parent. Move the settings directory’s conditional product catalog into a pure TypeScript builder, leaving loading and interaction state in `SettingsView`.

**Tech Stack:** Rust, serde/serde_json, Vue 3, TypeScript, Vitest, PowerShell architecture contracts.

## Global Constraints

- Preserve every pre-existing worktree change.
- Do not edit recognition, OCR, migration, installer, release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Keep public command and generated binding signatures unchanged.
- Do not stage or commit.
- Add tests before changing production wiring, then run targeted and repository-level verification.

---

## Task 1: Extract remote sync change decoding

**Files:**

- Create: `src-tauri/src/modules/sync_pull_decoder.rs`
- Modify: `src-tauri/src/modules/sync_pull.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

- [x] Add focused unit tests in `sync_pull_decoder.rs` that construct `RemotePullChange` envelopes and verify that an oversized page, a non-increasing sequence, and a foreign account are rejected while a valid page is accepted.
- [x] Run the Rust library tests and confirm the decoder contract through its private module tests.
- [x] Move `DecodedChange`, `validate_page`, `decode_page`, `without_account`, `from_value`, and `validate_remote_asset` into the private child module.
- [x] Wire the child module from `sync_pull.rs`:

```rust
#[path = "sync_pull_decoder.rs"]
mod sync_pull_decoder;

use sync_pull_decoder::{DecodedChange, decode_page, validate_page};
```

- [x] Keep shared constants and UUID validation private to `sync_pull`, accessed from the child with `super`.
- [x] Extend the PowerShell contract so `sync_pull_decoder.rs` must own `validate_page` and `decode_page`, and the orchestration module may no longer define them.
- [x] Run formatting, the boundary contract, the sync pull integration test, and Clippy.

## Task 2: Extract settings directory product rules

**Files:**

- Create: `src/app/settings-section-catalog.ts`
- Create: `src/app/settings-section-catalog.test.ts`
- Modify: `src/app/views/SettingsView.vue`

- [x] Add pure-function tests for stable group order, conditional overview/subject/review entries, and unique section IDs.
- [x] Run the focused Vitest file as part of the settings regression batch.
- [x] Implement:

```ts
export interface SettingsSectionAvailability {
  overview: boolean
  subjects: boolean
  review: boolean
}

export function buildSettingsSections(
  availability: SettingsSectionAvailability,
): SettingsSectionLink[]
```

- [x] Replace the inline `computed<SettingsSectionLink[]>(() => [...])` catalog in `SettingsView.vue` with a call to the builder; leave all labels, hints, IDs, grouping, and visibility conditions unchanged.
- [x] Run focused settings tests, TypeScript checks, ESLint, and the full frontend suite.

## Task 3: Final verification and review

- [x] Run `cargo fmt --check`.
- [ ] Run the complete Rust test suite. Blocked on this Windows host by repeated SQLCipher `VirtualLock` warning floods and non-terminating test processes; the directly affected decoder unit tests and `sync_pull` integration suite pass.
- [x] Run the architecture boundary contract.
- [x] Run frontend typechecking, linting, and the complete Vitest suite.
- [x] Review `git diff --check` and the scoped diff for accidental changes.
- [x] Report completed boundaries, verification results, and remaining commercial-quality risks while keeping the excluded pre-launch checklist out of scope.
