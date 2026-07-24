# Private Diagnostics Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a learner export one support-ready JSON diagnostic report from Settings without exposing pictures, answers, notes, tags, identities, credentials, absolute paths, file names, or cloud endpoints.

**Architecture:** A focused Rust diagnostics module owns the fixed report schema, aggregate SQLite queries, redaction boundary, and atomic JSON write. A thin Tauri command opens a native folder picker and returns only an opaque report ID plus a generated file label; Vue never receives the selected path or raw diagnostic content. The Settings page presents the privacy contract before generation and gives explicit cancellation, progress, success, and retry states.

**Tech Stack:** Rust stable, `rusqlite`, `serde`, `serde_json`, `rfd`, `uuid`; Tauri 2 typed commands through `tauri-specta`; Vue 3 strict TypeScript, Lucide, Vitest and Testing Library.

## Global Constraints

- Windows is the only v1 release platform.
- The report is a single UTF-8 JSON file with a fixed, versioned schema.
- Never include image bytes, OCR content, question or answer text, notes, tags, subjects, user names, e-mail addresses, account/profile/device/entity IDs, access or refresh tokens, encryption keys, environment variables, absolute paths, original file names, cloud URLs, request bodies, or raw SQLite error text.
- Vue receives no local path and cannot choose an arbitrary output file name.
- Database checks are reduced to fixed booleans or fixed status enums; raw `PRAGMA quick_check` output never leaves Rust.
- All numeric counts are non-negative and bounded to `u64`.
- Native folder-picker cancellation is a successful `null` result and must not display an error.
- The report is written to a same-directory temporary file, flushed, and atomically renamed; a failed write leaves no partial final report.
- UI motion uses only `transform` and `opacity`, follows the existing 120/180/240 ms tokens, and is disabled by `prefers-reduced-motion`.

---

## File Structure

- Create `src-tauri/src/modules/diagnostics.rs`: fixed DTOs, safe aggregate queries, JSON serialization, privacy validation helpers, and atomic file creation.
- Create `src-tauri/src/commands/diagnostics.rs`: native folder selection, background execution, typed `AppResult`, and public error copy.
- Create `src-tauri/tests/diagnostics.rs`: real encrypted-database integration coverage and recursive privacy assertions.
- Modify `src-tauri/src/modules/mod.rs`: export the diagnostics domain module.
- Modify `src-tauri/src/commands/mod.rs`: export the command module.
- Modify `src-tauri/src/bindings.rs`: register `diagnostics_export`.
- Modify `src/app/views/SettingsView.vue`: diagnostic section, state machine, privacy disclosure, feedback, and reduced-motion transition.
- Modify `src/app/views/SettingsView.test.ts`: cancellation, success, retry, duplicate-click, and path-redaction behavior.
- Modify `docs/plans/release.md`: record the delivered diagnostic contract and leave signed updater/performance acceptance as release work.

### Task 1: Safe report schema and atomic writer

**Files:**
- Create: `src-tauri/src/modules/diagnostics.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Test: `src-tauri/tests/diagnostics.rs`

**Interfaces:**
- Consumes: `rusqlite::Connection`, application version, storage kind, current UTC milliseconds, and a caller-selected directory used only inside Rust.
- Produces:

```rust
pub struct DiagnosticExportReceipt {
    pub report_id: String,
    pub file_label: String,
    pub generated_at_utc_ms: i64,
    pub warning_count: u32,
}

pub struct DiagnosticContext<'a> {
    pub app_version: &'a str,
    pub storage_kind: DiagnosticStorageKind,
    pub now_utc_ms: i64,
}

pub fn export_diagnostic_report(
    connection: &std::sync::Mutex<rusqlite::Connection>,
    destination: &std::path::Path,
    context: DiagnosticContext<'_>,
) -> Result<DiagnosticExportReceipt, DiagnosticError>;
```

- [ ] **Step 1: Write the failing privacy and aggregate tests**

Create an integration test that initializes the real encrypted schema, inserts sentinel secrets into account IDs, profile IDs, subjects, tags, notes, encrypted paths, and sync payloads, exports a report, recursively walks every JSON string, and asserts that no sentinel or absolute fixture path occurs. Assert the public schema is exactly:

```json
{
  "schemaVersion": 1,
  "reportId": "<uuid>",
  "generatedAtUtcMs": 1700000000000,
  "application": {
    "name": "Mistake Trainer Next",
    "version": "0.1.0",
    "platform": "windows",
    "architecture": "x86_64"
  },
  "library": {
    "storageKind": "default",
    "schemaVersion": 13,
    "integrity": "ok",
    "profileCount": 1,
    "problemCount": 1,
    "assetCount": 1,
    "captureBatchCount": 0,
    "reviewEventCount": 0,
    "exportSnapshotCount": 0
  },
  "sync": {
    "pendingOperationCount": 1,
    "failedOperationCount": 0,
    "unresolvedConflictCount": 0
  },
  "warnings": []
}
```

Also test that a non-directory destination fails, an existing generated final file is never overwritten, and an injected serialization/write failure leaves neither a final report nor a temporary file.

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test diagnostics
```

Expected: compilation fails because `modules::diagnostics` does not exist.

- [ ] **Step 3: Implement the fixed report and writer**

Define private serializable structs rather than serializing database rows. Query only `COUNT(*)`, `PRAGMA user_version`, and `PRAGMA quick_check(1)`. Convert `quick_check` to `DiagnosticIntegrity::Ok | Failed` without storing its raw text. Generate a UUIDv7 report ID and a Rust-owned file name:

```rust
let short_id = report_id.as_simple().to_string();
let file_label = format!(
    "Mistake-Trainer-Diagnostics-{}-{}.json",
    context.now_utc_ms,
    &short_id[..8],
);
```

Serialize with `serde_json::to_vec_pretty`, create a hidden-style temporary sibling using `OpenOptions::new().write(true).create_new(true)`, call `write_all` and `sync_all`, then `rename`. Clean up the temporary file on every failure.

- [ ] **Step 4: Run the focused test and verify it passes**

Run the command from Step 2.

Expected: all diagnostics integration tests pass and the exported JSON contains no sentinels.

- [ ] **Step 5: Commit the domain slice**

```powershell
git add src-tauri/src/modules/diagnostics.rs src-tauri/src/modules/mod.rs src-tauri/tests/diagnostics.rs
git commit -m "feat: export privacy-safe diagnostic reports"
```

### Task 2: Typed Tauri command boundary

**Files:**
- Create: `src-tauri/src/commands/diagnostics.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.ts` (generated)
- Test: `src-tauri/src/commands/diagnostics.rs`
- Test: `src-tauri/tests/command_contract.rs`
- Test: `src-tauri/tests/bindings_contract.rs`

**Interfaces:**
- Consumes: `State<LibraryRuntime>` and `State<ApplicationControlRoot>`.
- Produces:

```rust
#[tauri::command]
#[specta::specta]
pub async fn diagnostics_export(
    state: State<'_, LibraryRuntime>,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<Option<DiagnosticExportReceipt>>, ()>;
```

- [ ] **Step 1: Write failing command contract tests**

Assert `diagnostics_export` is registered, generated TypeScript exposes `commands.diagnosticsExport(): Promise<AppResult<DiagnosticExportReceipt | null>>`, and picker cancellation maps to `AppResult::success(None)`.

- [ ] **Step 2: Run contract tests and verify they fail**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test bindings_contract
```

Expected: the command and generated binding are missing.

- [ ] **Step 3: Implement the command**

Open an `rfd::FileDialog` folder picker in `spawn_blocking`, derive only the safe `default | custom` storage kind from `resolve_library_location(control_root)`, call the domain writer, and return its receipt. Map lock, database, serialization, and I/O failures to fixed Chinese messages with a fresh UUIDv7 diagnostic ID; do not interpolate the internal error into user copy or command output.

- [ ] **Step 4: Generate bindings and run command tests**

Run:

```powershell
pnpm bindings:generate
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test command_contract
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test bindings_contract
```

Expected: both contract suites pass and the generated file is the only binding change.

- [ ] **Step 5: Commit the command slice**

```powershell
git add src-tauri/src/commands/diagnostics.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src/shared/api/bindings.ts src-tauri/tests/command_contract.rs src-tauri/tests/bindings_contract.rs
git commit -m "feat: expose diagnostic report export"
```

### Task 3: Settings privacy UX and release evidence

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `docs/plans/release.md`

**Interfaces:**
- Consumes: `commands.diagnosticsExport()`.
- Produces: a Settings section `settings-diagnostics` with idle, generating, success, cancelled, and retryable-error states.

- [ ] **Step 1: Write failing UI tests**

Add tests that assert:

```ts
expect(screen.getByText('不会包含题图、答案、笔记、账户信息或本机路径')).toBeInTheDocument()
await fireEvent.click(screen.getByRole('button', { name: '生成安全诊断报告' }))
expect(api.diagnosticsExport).toHaveBeenCalledTimes(1)
```

Cover these outcomes:

- `ok: true, data: null` restores focus and shows no error.
- a receipt renders only `fileLabel`, `reportId`, generation time, and warning count.
- a fabricated absolute path returned by a broken test double is never rendered.
- repeated clicks while pending make one command call.
- an `AppResult` failure stays in the panel and the button is usable for retry.

- [ ] **Step 2: Run the focused UI test and verify it fails**

Run:

```powershell
pnpm exec vitest run src/app/views/SettingsView.test.ts
```

Expected: no diagnostics section or command exists.

- [ ] **Step 3: Implement the Settings section**

Add the section to `settingsSections`, import `FileJson2`, and keep the state local to the view:

```ts
const exportingDiagnostics = ref(false)
const diagnosticsMessage = ref('')
const diagnosticsReceipt = ref<DiagnosticExportReceipt>()

async function exportDiagnostics() {
  if (exportingDiagnostics.value || !isTauri()) return
  exportingDiagnostics.value = true
  diagnosticsMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.diagnosticsExport())
    if (!result.ok) diagnosticsMessage.value = result.error.userMessage
    else if (result.data) diagnosticsReceipt.value = result.data
  }
  catch {
    diagnosticsMessage.value = '诊断报告没有生成，现有资料不会受到影响，请检查保存位置后重试。'
  }
  finally {
    exportingDiagnostics.value = false
  }
}
```

Render a concise privacy list, a folder-selection button, an `aria-live="polite"` success receipt, and a retryable inline error. Animate the receipt with opacity and `translateY(8px)` only; disable it under reduced motion.

- [ ] **Step 4: Update release evidence and run all gates**

Record the delivered schema/redaction/atomic-write contract in `docs/plans/release.md`. Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm bindings:check
pnpm build
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml -- --check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

Expected: every command exits 0; initial JS remains below 300 KB gzip; the Settings lazy chunk remains below 120 KB gzip; the Windows NSIS installer is generated.

- [ ] **Step 5: Commit the completed feature**

```powershell
git add src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts docs/plans/release.md
git commit -m "feat: add private diagnostics experience"
```

## Self-Review

- Spec coverage: the plan covers a fixed safe schema, aggregate-only database access, atomic writing, native cancellation, typed command generation, private Settings UX, tests, build budgets, and Windows installer evidence.
- Deliberate exclusions: persistent application log collection, crash dump collection, remote upload, and device management are separate subsystems. They are not silently approximated here.
- Placeholder scan: every implementation step names its concrete behavior, files, commands, and expected result.
- Type consistency: `DiagnosticExportReceipt`, `DiagnosticContext`, `export_diagnostic_report`, and `diagnostics_export` retain the same names and fields across Rust, generated TypeScript, tests, and UI.
