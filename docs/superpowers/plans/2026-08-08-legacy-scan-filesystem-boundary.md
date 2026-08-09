# Legacy Scan Filesystem Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate untrusted legacy-tree filesystem validation, bounded reads, content hashing, and fingerprint traversal from legacy metadata parsing and import-plan construction without changing public behavior.

**Architecture:** Keep `legacy_scan.rs` responsible for legacy JSON DTOs, member discovery, scan reports, normalization, and import-plan construction. Add a private nested `legacy_scan_filesystem.rs` module that owns byte budgets, safe relative-path validation, bounded file reads, asset hashing, reparse/symlink rejection, traversal budgets, and the public tree fingerprint implementation; `legacy_scan.rs` re-exports the same compatibility surface used by the facade and sibling transaction modules.

**Tech Stack:** Rust 2024, std filesystem APIs, SHA-256, Vitest source contracts, PowerShell architecture contracts, Cargo test/fmt/clippy

## Global Constraints

- Do not change the legacy JSON schema, scan issue codes/details, import plans, fingerprints, byte budgets, traversal budgets, errors, public Rust paths, Tauri commands, bindings, database schema, or dependencies.
- Preserve `legacy_tree_fingerprint(root: &Path) -> Result<String, LegacyScanError>` through `modules::legacy::legacy_tree_fingerprint`.
- Preserve sibling compatibility imports through `legacy_scan::{MAX_ASSET_BYTES, is_safe_relative_path, read_bounded, take_chars}`.
- `legacy_scan_filesystem.rs` must not own serde DTOs, JSON parsing, scan reports, issue reporting, time parsing, import-plan construction, or database behavior.
- Preserve all unrelated working-tree changes; do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not implement licensing, privacy/legal, support operations, account deletion, device migration/recovery, updater recovery, or SLA work.

---

### Task 1: Legacy scan filesystem ownership boundary

**Files:**
- Create: `tests/legacy-scan-filesystem-boundary.test.ts`
- Create: `src-tauri/src/modules/legacy_scan_filesystem.rs`
- Modify: `src-tauri/src/modules/legacy_scan.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Public compatibility: `pub use legacy_scan_filesystem::legacy_tree_fingerprint;`
- Sibling compatibility: `pub(super) use legacy_scan_filesystem::{MAX_ASSET_BYTES, is_safe_relative_path, read_bounded};`; the nested definitions use `pub(in crate::modules::legacy)` so the re-export can reach sibling transaction modules without becoming crate-public.
- Scan-internal imports: `BoundedReadError`, `MAX_METADATA_BYTES`, `MAX_TOTAL_ASSET_BYTES`, and `sha256_file`.
- Filesystem child consumes `super::{LegacyScanError, MAX_DIRECTORY_ENTRIES, MAX_RECORDS}` only.

- [x] **Step 1: Write the failing source ownership contract**

Create `tests/legacy-scan-filesystem-boundary.test.ts`:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(path: string) {
  return readFileSync(resolve(path), 'utf8')
}

const scanPath = 'src-tauri/src/modules/legacy_scan.rs'
const filesystemPath = 'src-tauri/src/modules/legacy_scan_filesystem.rs'

describe('legacy scan filesystem ownership boundary', () => {
  it('isolates untrusted tree IO from legacy parsing and reporting', () => {
    const scan = source(scanPath)
    expect(scan).toContain('#[path = "legacy_scan_filesystem.rs"]')
    expect(scan).toContain('pub use legacy_scan_filesystem::legacy_tree_fingerprint;')
    expect(scan).toContain('pub(super) use legacy_scan_filesystem::{')
    expect(scan).not.toContain('pub fn legacy_tree_fingerprint(')
    expect(scan).not.toContain('fn collect_fingerprint_files(')
    expect(scan).not.toContain('pub(super) fn read_bounded(')
    expect(scan).not.toContain('pub(super) fn is_safe_relative_path(')

    const filesystem = source(filesystemPath)
    for (const definition of [
      'pub fn legacy_tree_fingerprint(',
      'fn collect_fingerprint_files(',
      'pub(super) fn read_bounded(',
      'pub(super) fn is_safe_relative_path(',
      'pub(super) fn sha256_file(',
      'fn is_windows_reparse_point(',
    ]) {
      expect(filesystem).toContain(definition)
    }
    expect(filesystem).not.toMatch(/serde|serde_json|LegacyScanReport|LegacyIssue/)
    expect(filesystem).not.toMatch(/OffsetDateTime|rusqlite|INSERT |UPDATE |DELETE /)
  })
})
```

- [x] **Step 2: Run the contract and verify the red state**

Run: `pnpm exec vitest run tests/legacy-scan-filesystem-boundary.test.ts`

Expected: FAIL because `legacy_scan.rs` does not declare the filesystem child.

- [x] **Step 3: Extract filesystem budgets and primitives**

Create `legacy_scan_filesystem.rs` with these imports and constants:

```rust
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{LegacyScanError, MAX_DIRECTORY_ENTRIES, MAX_RECORDS};

pub(super) const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
pub(in crate::modules::legacy) const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_TOTAL_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_DIRECTORY_DEPTH: usize = 32;
const MAX_FINGERPRINT_ENTRIES: usize = MAX_RECORDS + MAX_DIRECTORY_ENTRIES;
```

Move these implementations without changing bodies, messages, limits, hashing order, or error mapping. Define `is_safe_relative_path`, `BoundedReadError`, and `read_bounded` with `pub(in crate::modules::legacy)` so their parent can preserve the existing sibling-visible compatibility export and its return type:

- `legacy_tree_fingerprint`
- `collect_fingerprint_files`
- both platform variants of `is_windows_reparse_point`
- `is_safe_relative_path`
- `BoundedReadError`
- `read_bounded`
- `sha256_file`

Expose `sha256_file` as `pub(super)` so its parent scanner can reuse it. Remove the moved `fs::File`/`Read`/`Component`/`Sha256` ownership and five moved constants from `legacy_scan.rs`; retain `fs`, `io`, `Path`, and `PathBuf` there for member discovery and `LegacyScanError`.

- [x] **Step 4: Wire compatibility re-exports and internal imports**

At the start of `legacy_scan.rs`, add:

```rust
#[path = "legacy_scan_filesystem.rs"]
mod legacy_scan_filesystem;

pub use legacy_scan_filesystem::legacy_tree_fingerprint;
pub(super) use legacy_scan_filesystem::{
    MAX_ASSET_BYTES, is_safe_relative_path, read_bounded,
};
use legacy_scan_filesystem::{
    BoundedReadError, MAX_METADATA_BYTES, MAX_TOTAL_ASSET_BYTES, sha256_file,
};
```

Do not change `legacy.rs`, `legacy_import_transaction.rs`, `legacy_rollback_transaction.rs`, commands, or bindings.

- [x] **Step 5: Update the repository architecture contract**

In `scripts/rust-boundary-contract.ps1`:

- require `legacy_scan.rs` to declare and re-export the filesystem child;
- move ownership requirements for `legacy_tree_fingerprint`, `collect_fingerprint_files`, traversal constants, `is_safe_relative_path`, `read_bounded`, and `sha256_file` to `legacy_scan_filesystem.rs`;
- reject those definitions from `legacy_scan.rs`;
- reject `serde`, `serde_json`, `LegacyScanReport`, `LegacyIssue`, `OffsetDateTime`, `rusqlite`, and SQL mutation keywords from the filesystem child;
- continue rejecting public scan/build functions and scan DTOs from `legacy.rs`.

- [x] **Step 6: Run focused ownership and legacy behavior tests**

Run: `pnpm exec vitest run tests/legacy-scan-filesystem-boundary.test.ts`

Expected: PASS.

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: all 16 existing legacy tests PASS with unchanged fingerprint, scan, import, rollback, and command behavior.

- [x] **Step 7: Run static and full regression gates**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm contract:rust-boundaries
```

Expected: all commands PASS; only existing environment-dependent ignored tests remain ignored.

- [x] **Step 8: Review isolation and whitespace**

Run: `git diff --check`

Inspect the two implementation files, both architecture contracts, and this plan. Confirm public paths, limits, error text, hashing order, bounded-read semantics, and legacy reports remain unchanged; confirm commands, bindings, schema, dependencies, and `recognition_visual_split.rs` were untouched by this task.

## Self-Review

- Spec coverage: the plan covers filesystem ownership, compatibility paths, byte/traversal invariants, focused legacy behavior, full Rust gates, and scope isolation.
- Placeholder scan: no TBD, TODO, undefined implementation, or generic error-handling step remains.
- Type consistency: `legacy_scan_filesystem` consumes the parent error and shared record/directory budgets; `legacy_scan` re-exports the same public and sibling-visible functions used before extraction.
