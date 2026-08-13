# Architecture Boundary Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Establish executable dependency-direction rules, remove production `src/modules` dependencies on `src/app`, and make Rust startup inventory policy independent of infrastructure implementations.

**Architecture:** `src/shared` owns reusable UI mechanics and cross-feature contracts; `src/app` owns composition and concrete controllers; `src/modules` consumes only shared contracts and its own feature code. In Rust, pure startup inventory classification stays in `application`, filesystem probing stays in `infrastructure`, and current module-to-infrastructure exceptions are ratcheted so later remediation can only shrink them.

**Tech Stack:** Vue 3.5, TypeScript 5.9, Vitest 4, Rust 2024, Tauri 2, PowerShell architecture contracts.

## Global Constraints

- Preserve all existing user changes, including the current modifications to `src/app/App.vue`, Windows installer smoke scripts, storage-location code, and fresh-start recovery files.
- Do not change runtime behavior or user-facing copy in this foundation phase.
- Production files under `src/modules` must not import from `src/app`.
- Rust `domain` code must not depend on `application`, `commands`, `infrastructure`, or `modules`.
- Rust application policy files other than the explicit startup composition root must not depend on `commands`, `infrastructure`, or `modules`.
- Existing Rust module-to-infrastructure dependencies are a temporary explicit baseline; new dependencies fail the architecture test and later phases must shrink the baseline.
- Keep generated `src/shared/api/bindings.ts` unchanged.

---

### Task 1: Executable dependency-direction contract

**Files:**
- Create: `tests/architecture-dependency-direction.test.ts`
- Modify: `package.json`

**Interfaces:**
- Consumes: repository source paths rooted at `src` and `src-tauri/src`.
- Produces: a Vitest architecture suite and the command `pnpm contract:architecture`.

- [x] **Step 1: Write the failing architecture test**

Create a test that recursively reads production `.ts`, `.vue`, and `.rs` files and reports path-qualified violations. The core assertions are:

```ts
expect(frontendFeatureToAppImports()).toEqual([])
expect(frontendSharedToInnerImports()).toEqual([])
expect(rustLayerViolations('src-tauri/src/domain', [
  'application', 'commands', 'infrastructure', 'modules',
])).toEqual([])
expect(rustLayerViolations('src-tauri/src/application', [
  'commands', 'infrastructure', 'modules',
], ['startup.rs'])).toEqual([])
expect(rustModuleInfrastructureImports()).toEqual([
  'src-tauri/src/modules/auth_session_state.rs',
  'src-tauri/src/modules/auth_sync.rs',
  'src-tauri/src/modules/backup_creation.rs',
  'src-tauri/src/modules/backup_portability.rs',
  'src-tauri/src/modules/backup_validation.rs',
  'src-tauri/src/modules/capture_crop.rs',
  'src-tauri/src/modules/capture_inbox.rs',
  'src-tauri/src/modules/capture_quality.rs',
  'src-tauri/src/modules/capture_recognition_transaction.rs',
  'src-tauri/src/modules/exports_generation.rs',
  'src-tauri/src/modules/legacy.rs',
  'src-tauri/src/modules/legacy_import_transaction.rs',
  'src-tauri/src/modules/library_reset.rs',
  'src-tauri/src/modules/problem_query_repository.rs',
  'src-tauri/src/modules/problems.rs',
  'src-tauri/src/modules/product_check.rs',
  'src-tauri/src/modules/storage_migration.rs',
  'src-tauri/src/modules/storage_migration_snapshot.rs',
  'src-tauri/src/modules/sync_pull.rs',
  'src-tauri/src/modules/sync_pull_transaction.rs',
  'src-tauri/src/modules/sync_push.rs',
])
```

Ignore frontend test files and raw-source test imports when checking production dependencies. Normalize path separators to `/` before comparisons.

- [x] **Step 2: Run the test and verify the current violations are exposed**

Run:

```powershell
pnpm vitest run tests/architecture-dependency-direction.test.ts
```

Expected: FAIL. The first assertion lists feature imports from `src/app`; the application assertion lists `src-tauri/src/application/library_inventory.rs`.

- [x] **Step 3: Add the dedicated package command**

Add the following script without changing existing scripts:

```json
"contract:architecture": "vitest run tests/architecture-dependency-direction.test.ts && powershell -NoProfile -ExecutionPolicy Bypass -File scripts/rust-boundary-contract.ps1"
```

- [x] **Step 4: Keep the test red until Tasks 2–4 remove the violations**

Run:

```powershell
pnpm contract:architecture
```

Expected: FAIL in the new Vitest suite before the existing PowerShell contract runs.

### Task 2: Shared UI mechanics boundary

**Files:**
- Create: `src/shared/ui/dialog-focus.ts`
- Create: `src/shared/ui/dialog-scroll-lock.ts`
- Create: `src/shared/ui/dialog-document-boundary.ts`
- Create: `src/shared/ui/composables/useActionConfirmation.ts`
- Create: `src/shared/ui/composables/useMenuButton.ts`
- Create: `src/shared/ui/composables/useUnsavedChangesGuard.ts`
- Create: `src/shared/ui/components/ActionConfirmDialog.vue`
- Move tests from the corresponding `src/app` locations into `src/shared/ui`.
- Modify: all Vue/TypeScript consumers returned by `rg -l "dialog-focus|dialog-document-boundary|dialog-scroll-lock|ActionConfirmDialog|useActionConfirmation|useMenuButton|useUnsavedChangesGuard" src`.
- Delete: the corresponding implementations under `src/app` after all imports move.

**Interfaces:**
- Consumes: Vue lifecycle APIs and DOM APIs only.
- Produces: stable imports below; no shared file may import `@/app` or `@/modules`.

```ts
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { getDialogFocusableElements, trapDialogFocus } from '@/shared/ui/dialog-focus'
import ActionConfirmDialog from '@/shared/ui/components/ActionConfirmDialog.vue'
import { useActionConfirmation } from '@/shared/ui/composables/useActionConfirmation'
import { useMenuButton } from '@/shared/ui/composables/useMenuButton'
import {
  type NavigationAttempt,
  useUnsavedChangesGuard,
} from '@/shared/ui/composables/useUnsavedChangesGuard'
```

- [x] **Step 1: Copy the existing implementations into the shared boundary**

Preserve function signatures and behavior exactly. Update only internal shared imports, for example:

```ts
// src/shared/ui/dialog-document-boundary.ts
import { acquireDialogScrollLock } from './dialog-scroll-lock'
```

```ts
// src/shared/ui/composables/useUnsavedChangesGuard.ts
import type { ActionConfirmationRequest } from './useActionConfirmation'
import { useActionConfirmation } from './useActionConfirmation'
```

- [x] **Step 2: Move focused unit tests beside the shared implementations**

Update test-local imports to their new relative locations. Preserve all test bodies. The moved component test imports:

```ts
import ActionConfirmDialog from './ActionConfirmDialog.vue'
```

The moved unsaved-changes test imports:

```ts
import ActionConfirmDialog from '../components/ActionConfirmDialog.vue'
import { useUnsavedChangesGuard } from './useUnsavedChangesGuard'
```

- [x] **Step 3: Update every production consumer to the shared aliases**

Use the exact stable imports in the Interfaces block. This includes both `src/modules/**` and `src/app/**`; no production consumer should retain a compatibility import through `src/app`.

- [x] **Step 4: Update source-path assertions**

Change the safety readability fixture path to:

```ts
const confirmPath = 'src/shared/ui/components/ActionConfirmDialog.vue'
```

- [x] **Step 5: Delete obsolete app-owned implementations and run focused tests**

Run:

```powershell
pnpm vitest run src/shared/ui src/modules/capture/components/CaptureWorkspace.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts
```

Expected: PASS, with no unresolved imports.

### Task 3: Shared synchronization contract

**Files:**
- Create: `src/shared/contracts/sync-controller.ts`
- Modify: `src/app/sync-controller.ts`
- Modify: `src/modules/sync/components/SyncConflictCenter.vue`
- Modify: `src/modules/sync/components/SyncConflictCenter.test.ts`
- Modify: `src/modules/legacy/components/LegacyImportPanel.vue`
- Modify: `src/modules/legacy/components/LegacyImportPanel.test.ts`

**Interfaces:**
- Consumes: `AppResult<SyncNowReport>` and Vue `InjectionKey`.
- Produces: `SyncTrigger`, `SyncController`, and `syncControllerKey` without exposing app orchestration.

```ts
export type SyncTrigger = 'startup' | 'online' | 'visible' | 'manual' | 'mutation'

export interface SyncController {
  run: (reason: SyncTrigger) => Promise<AppResult<SyncNowReport>>
  scheduleMutation: () => void
  dispose: () => void
}

export const syncControllerKey: InjectionKey<SyncController> = Symbol('sync-controller')
```

- [x] **Step 1: Add the shared consumer contract**

Create `src/shared/contracts/sync-controller.ts` with the exact interface above and imports from `../api/app-result` and `../api/bindings`.

- [x] **Step 2: Make the app controller implement and re-export the shared contract**

Replace its local contract declarations with:

```ts
import type { SyncController, SyncTrigger } from '../shared/contracts/sync-controller'

export {
  syncControllerKey,
  type SyncController,
  type SyncTrigger,
} from '../shared/contracts/sync-controller'
```

Keep `SyncPhase`, status copy, debounce logic, and `createSyncController` in `src/app/sync-controller.ts`.

- [x] **Step 3: Point feature consumers directly at the shared contract**

Use:

```ts
import { syncControllerKey } from '@/shared/contracts/sync-controller'
```

Existing app consumers may continue through the app re-export during this phase so the modified user-owned `App.vue` need not be rewritten.

- [x] **Step 4: Run synchronization controller and feature tests**

Run:

```powershell
pnpm vitest run src/app/sync-controller.test.ts src/modules/sync/components/SyncConflictCenter.test.ts src/modules/legacy/components/LegacyImportPanel.test.ts
```

Expected: PASS.

### Task 4: Pure Rust startup-inventory policy

**Files:**
- Modify: `src-tauri/src/application/library_inventory.rs`
- Modify: `src-tauri/src/application/startup.rs`
- Create: `src-tauri/src/infrastructure/library_inventory.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src-tauri/src/infrastructure/runtime_credentials.rs`
- Modify: boundary tests that assert the old type owner, if any.

**Interfaces:**
- Consumes: application-owned `CredentialEnvelopeState` and `LibraryArtifactState`.
- Produces: pure `classify_startup_inventory` policy plus infrastructure filesystem probing.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialEnvelopeState {
    Absent,
    Complete,
    Partial,
}

pub fn inspect_library_artifacts(
    library_root: &Path,
) -> Result<LibraryArtifactState, std::io::Error>;
```

- [x] **Step 1: Move the credential-envelope state into application policy**

Define `CredentialEnvelopeState` in `application/library_inventory.rs` and remove its infrastructure import. Import that type in `runtime_credentials.rs`; re-export it from `runtime.rs` for compatibility:

```rust
pub use crate::application::library_inventory::CredentialEnvelopeState;
```

- [x] **Step 2: Move filesystem probing into an infrastructure adapter**

Create:

```rust
use std::path::Path;

use crate::application::library_inventory::LibraryArtifactState;

pub fn inspect_library_artifacts(
    library_root: &Path,
) -> Result<LibraryArtifactState, std::io::Error> {
    for path in [library_root.join("library.db"), library_root.join("assets")] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Ok(LibraryArtifactState::Present),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(LibraryArtifactState::Absent)
}
```

Expose it with `pub mod library_inventory;` and import it from that path in `application/startup.rs`.

- [x] **Step 3: Verify application policy is now infrastructure-independent**

Run:

```powershell
pnpm vitest run tests/architecture-dependency-direction.test.ts
```

Expected: PASS, including the production feature-to-app and application-layer assertions.

- [x] **Step 4: Run Rust inventory and startup tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib library_inventory
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib startup
```

Expected: PASS.

### Task 5: Foundation verification and documentation sync

**Files:**
- Modify: `docs/architecture.md`
- Modify: this plan's checkboxes as tasks complete.

**Interfaces:**
- Consumes: the completed dependency contract and shared boundaries.
- Produces: documented, executable dependency rules and a verified foundation for the next remediation plan.

- [x] **Step 1: Document the enforced frontend and Rust dependency rules**

Add these rules to the dependency section:

```markdown
- Frontend feature modules consume app-wide capabilities through contracts in `src/shared`; they do not import `src/app` implementations.
- Reusable modal, focus, menu, and confirmation mechanics live in `src/shared/ui`.
- Rust application policy is infrastructure-independent. `application/startup.rs` is the explicit composition root that may coordinate infrastructure and module operations.
- `pnpm contract:architecture` enforces dependency direction and ratchets remaining Rust module-to-infrastructure exceptions.
```

- [x] **Step 2: Run the complete proportional verification set**

Run:

```powershell
pnpm contract:architecture
pnpm typecheck
pnpm lint
pnpm test
```

Expected: all commands PASS.

- [x] **Step 3: Inspect the final diff for unrelated changes**

Run:

```powershell
git status --short
git diff -- tests/architecture-dependency-direction.test.ts package.json src/shared src/modules src/app src-tauri/src/application src-tauri/src/infrastructure docs/architecture.md
```

Expected: the diff contains only architecture-foundation changes plus the user's pre-existing modifications, which remain preserved.

- [x] **Step 4: Prepare the next remediation plan**

The next plan must start from the now-enforced boundary and cover feature public entrypoints plus extraction of bootstrap, sync lifecycle, and workspace/profile orchestration from `src/app/App.vue`. It must not weaken or baseline-away any rule established here.
