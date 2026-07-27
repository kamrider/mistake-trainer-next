# Optional OCR Capability Center Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Windows settings flow that checks whether the current computer is suitable for local OCR, recommends an honest model tier, and downloads or removes only explicitly confirmed optional components without changing capture, library, review, sync, or backup behavior.

**Architecture:** A library-independent Rust module owns the hardware assessment, immutable component catalog, verified streaming downloads, and app-data component directory. Typed Tauri commands expose only bounded status and component IDs; a focused Vue panel presents one recommended action, requires confirmation before network use, and treats unavailable components as informative rather than pretending they are usable.

**Tech Stack:** Rust 2024, Tauri 2, reqwest/rustls, SHA-256, Windows memory/disk APIs, Specta bindings, Vue 3, Vitest.

## Global Constraints

- Opening Settings must never download, install, enable, or remove a component.
- Existing capture, library, review, sync, backup, and application-startup paths must not depend on OCR capability state.
- The encrypted library and its storage location are not used for model files; optional components live under the fixed application-control root.
- The page receives component IDs and redacted capacity numbers, never local paths or arbitrary download URLs.
- Downloads use compile-time HTTPS URLs, exact byte limits, SHA-256 verification, temporary files, and atomic rename.
- A failed, cancelled, short, oversized, or hash-mismatched download leaves no installed component and preserves any previously verified component.
- PP-OCRv6 small is the only recommended first download on balanced hardware; medium is offered only on higher-tier hardware.
- OpenCV must not be advertised as downloadable until a signed, executable product runtime and immutable manifest exist.
- Installing a model does not enable automatic cropping or OCR; production use remains gated by the 60-image comparison and 300-image safety gate.
- Every user-facing state must distinguish “本机预检通过”, “模型已下载”, and “识别功能已验证可用”.

---

### Task 1: Hardware assessment and immutable catalog

**Files:**
- Create: `src-tauri/src/modules/ocr_capability.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Test: unit tests inside `src-tauri/src/modules/ocr_capability.rs`

**Interfaces:**
- Consumes: application-control root plus platform probes.
- Produces:

```rust
pub enum OcrHardwareTier { ManualOnly, Basic, Balanced, Performance }
pub enum OcrComponentId { Ppocrv6Small, Ppocrv6Medium, OpencvPreprocess }
pub enum OcrComponentState { NotInstalled, Installed, Corrupt, Unavailable }

pub struct OcrCapabilityStatus {
    pub assessment: OcrHardwareAssessment,
    pub components: Vec<OcrComponentStatus>,
}
```

- [ ] **Step 1: Write failing policy tests**

```rust
#[test]
fn four_cores_eight_gib_avx2_recommends_small() {
    let assessment = assess_hardware(HardwareFacts {
        logical_processors: 4,
        total_memory_bytes: 8 * GIB,
        available_component_bytes: 3 * GIB,
        avx2: true,
        architecture: "x86_64",
    });
    assert_eq!(assessment.tier, OcrHardwareTier::Balanced);
    assert_eq!(assessment.recommended_component_id, Some(OcrComponentId::Ppocrv6Small));
}

#[test]
fn medium_requires_eight_cores_and_sixteen_gib() {
    let assessment = assess_hardware(HardwareFacts {
        logical_processors: 8,
        total_memory_bytes: 16 * GIB,
        available_component_bytes: 5 * GIB,
        avx2: true,
        architecture: "x86_64",
    });
    assert_eq!(assessment.tier, OcrHardwareTier::Performance);
}
```

- [ ] **Step 2: Run the targeted tests**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml ocr_capability
```

Expected: FAIL because `ocr_capability` does not exist.

- [ ] **Step 3: Implement deterministic assessment and catalog metadata**

Use `std::thread::available_parallelism`, `GlobalMemoryStatusEx`, `GetDiskFreeSpaceExW`,
and `is_x86_feature_detected!("avx2")`. Keep assessment thresholds in named constants.
The OpenCV entry must be:

```rust
OcrComponentDescriptor {
    id: OcrComponentId::OpencvPreprocess,
    display_name: "OpenCV 图像预处理",
    availability: ComponentAvailability::RuntimeNotPublished,
    files: &[],
}
```

- [ ] **Step 4: Run tests and commit**

Run the targeted Rust test command and expect PASS.

```powershell
git add src-tauri/src/modules/ocr_capability.rs src-tauri/src/modules/mod.rs
git commit -m "feat: assess optional OCR capability"
```

### Task 2: Verified optional-component storage

**Files:**
- Modify: `src-tauri/src/modules/ocr_capability.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: unit tests inside `src-tauri/src/modules/ocr_capability.rs`

**Interfaces:**
- Consumes: `OcrComponentId`, fixed catalog descriptors, application-control root.
- Produces:

```rust
pub async fn install_component(
    root: &Path,
    component_id: OcrComponentId,
    client: &reqwest::Client,
) -> Result<OcrComponentStatus, OcrCapabilityError>;

pub fn remove_component(
    root: &Path,
    component_id: OcrComponentId,
) -> Result<OcrComponentStatus, OcrCapabilityError>;
```

- [ ] **Step 1: Write failing filesystem and transport tests**

Test an already verified install, exact-byte success, HTTP failure, oversized stream,
wrong SHA-256, interrupted two-file bundle, unavailable OpenCV descriptor, idempotent
removal, and preservation of an older verified bundle.

- [ ] **Step 2: Run the targeted tests**

Expected: the new install/storage tests fail.

- [ ] **Step 3: Implement capped streaming and atomic bundle promotion**

Download only into `<control-root>/optional-components/.staging/<uuid>`, hash while
streaming, reject any file beyond its catalog byte count, sync files, write a versioned
manifest, then atomically rename the completed directory. Never accept a URL from Vue.

- [ ] **Step 4: Run tests and commit**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml ocr_capability
git add src-tauri/src/modules/ocr_capability.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: verify optional OCR component downloads"
```

### Task 3: Typed command boundary

**Files:**
- Create: `src-tauri/src/commands/ocr_capability.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/ocr_capability_command.rs`

**Interfaces:**
- Produces:

```rust
pub async fn ocr_capability_status(
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<OcrCapabilityStatus>, ()>;

pub async fn ocr_component_install(
    component_id: OcrComponentId,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<OcrComponentStatus>, ()>;

pub async fn ocr_component_remove(
    component_id: OcrComponentId,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<OcrComponentStatus>, ()>;
```

- [ ] **Step 1: Write failing command-contract tests**

Assert stable `AppResult<T>` shapes, opaque enum IDs, retryable network failures,
non-retryable integrity failures, and no serialized path or URL.

- [ ] **Step 2: Register commands and generate bindings**

```powershell
pnpm bindings:generate
```

Expected: `src/shared/api/bindings.ts` contains the three commands and all capability
types.

- [ ] **Step 3: Run command and binding tests**

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test ocr_capability_command
pnpm test -- src/shared/api/bindings.test.ts
```

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/commands/ocr_capability.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/tests/ocr_capability_command.rs src/shared/api/bindings.ts
git commit -m "feat: expose optional OCR component commands"
```

### Task 4: One-decision settings experience

**Files:**
- Create: `src/modules/ocr/components/OcrCapabilityPanel.vue`
- Create: `src/modules/ocr/components/OcrCapabilityPanel.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: generated `commands.ocrCapabilityStatus`,
  `commands.ocrComponentInstall`, and `commands.ocrComponentRemove`.
- Produces: an independently failing settings region that cannot block other Settings
  sections.

- [ ] **Step 1: Write failing interaction tests**

Cover browser preview, loading, balanced recommendation, manual-only hardware, explicit
confirmation, successful install, retryable failure, installed state, removal
confirmation, medium hidden on insufficient hardware, and OpenCV “运行时尚未发布”.

- [ ] **Step 2: Run the focused test**

```powershell
pnpm test -- src/modules/ocr/components/OcrCapabilityPanel.test.ts
```

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the panel**

The primary balanced-hardware copy is:

```text
本机预检通过
推荐 PP‑OCRv6 small（约 31 MB）
下载只会在确认后开始；未下载时拍照、整理和题库保持原样。
```

The confirmation must repeat download size, local-only storage, ModelScope/RapidAI
source, Apache-2.0 model lineage, and that installation does not yet enable automatic
recognition.

- [ ] **Step 4: Integrate without coupling Settings load**

Mount `<OcrCapabilityPanel />` as its own `settings-ocr` section. Its command errors stay
inside the component and must not change `SettingsView.loading` or `errorMessage`.

- [ ] **Step 5: Run tests and commit**

```powershell
pnpm test -- src/modules/ocr/components/OcrCapabilityPanel.test.ts src/app/views/SettingsView.test.ts
git add src/modules/ocr/components src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: add optional OCR setup flow"
```

### Task 5: Documentation and regression gate

**Files:**
- Modify: `docs/architecture.md`
- Create: `docs/windows-ocr-capability-acceptance.md`
- Modify: `THIRD_PARTY_NOTICES.md`
- Create: `third-party-licenses/paddleocr-Apache-2.0.txt`

**Interfaces:**
- Documents the boundary between component readiness and production OCR enablement.

- [ ] **Step 1: Add acceptance checks**

Document low-memory, low-disk, balanced, and performance machines; offline install
failure; corrupted response; successful small install/removal; restart persistence;
Settings/capture/library/review regression; no startup network request; and no model
cache in encrypted backup.

- [ ] **Step 2: Run all automated gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Expected: all pass.

- [ ] **Step 3: Inspect the diff**

```powershell
git diff --check
git status --short
```

Confirm unrelated user changes are preserved and no model binaries, local paths, or
temporary downloads are tracked.

## Self-Review

- Spec coverage: hardware suitability, optional behavior, explicit confirmation,
  model download, OpenCV availability honesty, simplified flow, isolation from existing
  features, removal, integrity validation, typed commands, and regression checks all
  map to concrete tasks.
- Scope boundary: this center prepares the product for optional OCR but does not bypass
  the real-image evidence gates or silently enable an unverified capture path.
- Placeholder scan: no TBD/TODO or unspecified implementation step remains.
- Type consistency: `OcrComponentId`, `OcrComponentStatus`, and
  `OcrCapabilityStatus` are produced by Rust, generated by Specta, and consumed by the
  Vue component under the same names.
