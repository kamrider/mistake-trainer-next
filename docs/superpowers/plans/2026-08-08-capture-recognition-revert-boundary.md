# Capture Recognition Revert Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate recognition apply, revert/query, and shared operation-ledger ownership without changing the public API or transaction behavior.

**Architecture:** Keep `capture_recognition_transaction.rs` responsible for validating, staging, and atomically applying accepted recognition suggestions. Move revert validation, revert mutation, and latest-operation lookup into `capture_recognition_revert.rs`; move the serialized operation ledger into a SQL-free shared internal module. `capture_recognition.rs` remains the public facade and re-exports the same three functions under the same paths.

**Tech Stack:** Rust 2024, rusqlite, serde, Vitest source contracts, Cargo test/fmt/clippy

## Global Constraints

- Do not implement launch-only licensing, privacy/legal, support, account deletion, device migration/recovery, updater recovery, or SLA work.
- Preserve all existing working-tree changes and do not stage or commit files.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Preserve `apply_capture_recognition`, `revert_capture_recognition`, and `latest_capture_recognition_operation` public signatures and facade exports.
- Move existing SQL and transaction bodies without changing statement text, ordering, error variants, cleanup timing, revision checks, or report fields.
- Keep the shared ledger internal to `modules::capture_recognition`; it must not own SQL, filesystem operations, or command DTOs.
- Do not change generated TypeScript bindings or Tauri command registration.

---

### Task 1: Recognition operation ownership boundary

**Files:**
- Create: `tests/capture-recognition-operation-boundary.test.ts`
- Create: `src-tauri/src/modules/capture_recognition_operation_ledger.rs`
- Create: `src-tauri/src/modules/capture_recognition_revert.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`
- Modify: `src-tauri/src/modules/capture_recognition_transaction.rs`
- Modify: `scripts/rust-boundary-contract.ps1` (synchronize the repository gate with the new ownership boundary)

**Interfaces:**
- Consumes: existing recognition operation ledger JSON and the existing apply/revert/latest function bodies.
- Produces: unchanged facade exports; `pub(super) RecognitionOperationLedger` and ledger entry types; a dedicated revert/query module with `pub fn revert_capture_recognition` and `pub fn latest_capture_recognition_operation`.

- [x] **Step 1: Write the failing source ownership contract**

Create `tests/capture-recognition-operation-boundary.test.ts` with exact path and ownership assertions:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(path: string) {
  return readFileSync(resolve(path), 'utf8')
}

const facadePath = 'src-tauri/src/modules/capture_recognition.rs'
const applyPath = 'src-tauri/src/modules/capture_recognition_transaction.rs'
const revertPath = 'src-tauri/src/modules/capture_recognition_revert.rs'
const ledgerPath = 'src-tauri/src/modules/capture_recognition_operation_ledger.rs'

describe('capture recognition operation ownership boundary', () => {
  it('keeps apply, revert/query, and serialization ownership separate', () => {
    const facade = source(facadePath)
    expect(facade).toContain('#[path = "capture_recognition_operation_ledger.rs"]')
    expect(facade).toContain('#[path = "capture_recognition_revert.rs"]')
    expect(facade).toContain('pub use capture_recognition_transaction::apply_capture_recognition;')
    expect(facade).toContain('pub use capture_recognition_revert::{')

    const apply = source(applyPath)
    expect(apply).toContain('pub fn apply_capture_recognition(')
    expect(apply).not.toContain('pub fn revert_capture_recognition(')
    expect(apply).not.toContain('pub fn latest_capture_recognition_operation(')

    const revert = source(revertPath)
    expect(revert).toContain('pub fn revert_capture_recognition(')
    expect(revert).toContain('pub fn latest_capture_recognition_operation(')
    expect(revert).not.toContain('pub fn apply_capture_recognition(')

    const ledger = source(ledgerPath)
    expect(ledger).toContain('pub(super) struct RecognitionOperationLedger')
    expect(ledger).not.toMatch(/SELECT |INSERT |UPDATE |DELETE /)
    expect(ledger).not.toContain('std::fs')
  })
})
```

- [x] **Step 2: Run the contract and verify the red state**

Run: `pnpm exec vitest run tests/capture-recognition-operation-boundary.test.ts`

Expected: FAIL because the facade does not declare the ledger and revert modules.

- [x] **Step 3: Extract the shared serialized ledger**

Create `capture_recognition_operation_ledger.rs` with the four existing serde types. Keep `#[serde(rename_all = "camelCase")]` on every type and expose only the types and fields to their parent module with `pub(super)`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionOperationLedger {
    pub(super) source_items: Vec<RecognitionLedgerSource>,
    pub(super) created_items: Vec<RecognitionLedgerItem>,
    pub(super) created_drafts: Vec<RecognitionLedgerDraft>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerSource {
    pub(super) item_id: String,
    pub(super) asset_id: String,
    pub(super) superseded_by_derivation_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerItem {
    pub(super) item_id: String,
    pub(super) asset_id: String,
    pub(super) derivation_id: String,
    pub(super) source_sequence: i64,
    pub(super) staged_role: String,
    pub(super) draft_id: Option<String>,
    pub(super) role: Option<String>,
    pub(super) position: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerDraft {
    pub(super) draft_id: String,
    pub(super) position: i64,
}
```

Remove those definitions and the now-unused serde import from `capture_recognition_transaction.rs`; import the four ledger types from `super::capture_recognition_operation_ledger`.

- [x] **Step 4: Extract revert validation, mutation, and latest lookup**

Create `capture_recognition_revert.rs` with these imports:

```rust
use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};

use crate::modules::capture_inbox::{get_capture_batch_detail, remove_encrypted_blob};

use super::{
    CaptureRecognitionError, CaptureRecognitionOperationSummary, CaptureRecognitionRevertReport,
    RevertCaptureRecognition,
    capture_recognition_operation_ledger::RecognitionOperationLedger,
};
```

Move these existing functions into the new file without altering their bodies:

- `revert_capture_recognition`
- `latest_capture_recognition_operation`
- `validate_recognition_revert_state`

Leave `mark_recognition_suggestions_stale`, staged-asset cleanup, and apply-only structs in `capture_recognition_transaction.rs`.

- [x] **Step 5: Wire the unchanged public facade**

In `capture_recognition.rs`, declare the two new path modules and replace the combined export with exact ownership exports:

```rust
#[path = "capture_recognition_operation_ledger.rs"]
mod capture_recognition_operation_ledger;
#[path = "capture_recognition_revert.rs"]
mod capture_recognition_revert;
#[path = "capture_recognition_transaction.rs"]
mod capture_recognition_transaction;

pub use capture_recognition_revert::{
    latest_capture_recognition_operation, revert_capture_recognition,
};
pub use capture_recognition_transaction::apply_capture_recognition;
```

- [x] **Step 6: Run focused ownership and recognition tests**

Run: `pnpm exec vitest run tests/capture-recognition-operation-boundary.test.ts`

Expected: PASS.

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition`

Expected: the complete recognition integration test binary PASS with unchanged apply, revert, stale, cleanup, and latest-operation behavior.

- [x] **Step 7: Run Rust formatting, lint, and full regression gates**

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check`

Expected: PASS.

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

Expected: PASS with no warnings.

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`

Expected: all Rust targets PASS.

Run: `pnpm contract:rust-boundaries`

Expected: PASS.

- [x] **Step 8: Review isolation and whitespace**

Run: `git diff --check`

Expected: PASS with no whitespace errors.

Inspect the five implementation files, the ownership contract, and this plan. Preserve every unrelated working-tree change and confirm `recognition_visual_split.rs` remains untouched by this task.
