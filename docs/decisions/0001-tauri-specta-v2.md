# ADR 0001: Pin Tauri Specta v2 release candidate

## Status

Accepted for the Foundation phase; revisit before the signed v1 release.

## Context

`tauri-specta` 1.0.2 is the latest stable release, but its manifest depends on Tauri 1.
It cannot support this repository's Tauri 2.11 baseline. The upstream compatibility
table explicitly maps Tauri 2 to Tauri Specta 2 and Specta 2.

## Decision

Pin `tauri-specta` and `specta` to exactly `2.0.0-rc.25`, and
`specta-typescript` to exactly `0.0.12`. Generate bindings from the resource-manifested
Tauri executable with `--export-bindings`. CI regenerates the file and rejects drift.

## Consequences

- Upgrades are deliberate, never implicit semver updates.
- The generated client and runtime handler share one command registry.
- Before v1 signing, either upgrade to stable v2 or record a security and compatibility
  review that explicitly accepts the pinned candidate.
