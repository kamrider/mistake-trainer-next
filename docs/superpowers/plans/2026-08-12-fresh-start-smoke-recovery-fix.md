# Fresh Start Smoke Recovery Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fresh-start failures visible in the active confirmation dialog and prevent installer smoke cleanup from deleting an isolated application profile while an owned application process is still running.

**Architecture:** Keep the Rust recovery preflight fail-closed: disappearing or changed control evidence must still block credential deletion. Surface that failure through the existing Vue dialog, and harden the PowerShell smoke runner with executable-path-scoped process shutdown plus a cleanup refusal when any live process remains inside the owned smoke root.

**Tech Stack:** Vue 3, TypeScript, Testing Library/Vitest, PowerShell 7/Windows PowerShell, Tauri 2 Windows installer smoke scripts.

## Global Constraints

- Keep product version `0.1.2`.
- Do not weaken `fresh_start_preflight` or delete a library directory.
- Never kill processes by executable name; scope process handling to a canonical owned install root.
- Preserve unrelated user changes, including `docs/superpowers/plans/2026-08-10-question-figure-hybrid-pipeline.md`.

---

### Task 1: Show fresh-start backend failures inside the active dialog

**Files:**
- Modify: `src/app/LibraryFreshStartDialog.vue`
- Modify: `src/app/App.vue`
- Test: `src/app/LibraryFreshStartDialog.test.ts`

**Interfaces:**
- Consumes: `libraryRecoveryMessage: Ref<string>` from `App.vue`.
- Produces: `LibraryFreshStartDialog` prop `message?: string`, rendered as a live alert while the dialog stays open.

- [ ] **Step 1: Write the failing dialog error test**

Add this test to `LibraryFreshStartDialog.test.ts`:

```ts
it('shows a backend rejection inside the active dialog', () => {
  render(LibraryFreshStartDialog, {
    props: {
      busy: false,
      message: '资料库状态已经变化；没有删除任何凭据。',
    },
  })

  expect(screen.getByRole('alert')).toHaveTextContent(
    '资料库状态已经变化；没有删除任何凭据。',
  )
})
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `corepack pnpm vitest run src/app/LibraryFreshStartDialog.test.ts`

Expected: FAIL because the dialog does not expose an alert containing the backend message.

- [ ] **Step 3: Implement the dialog message prop and wiring**

Change the dialog props and render the message after the description:

```ts
defineProps<{ busy: boolean; message?: string }>()
```

```vue
<p
  v-if="message"
  class="dialog-error"
  role="alert"
>
  {{ message }}
</p>
```

Add scoped styling:

```css
.dialog-error { padding: 11px 12px; border: 1px solid rgba(185,88,63,.35); border-radius: 10px; color: var(--cinnabar); background: rgba(185,88,63,.08); line-height: 1.55; }
```

Pass the existing recovery message from `App.vue`:

```vue
<LibraryFreshStartDialog
  v-if="freshStartDialogOpen"
  :busy="libraryRecoveryBusy"
  :message="libraryRecoveryMessage"
  @cancel="freshStartDialogOpen = false"
  @confirm="confirmFreshStart"
/>
```

Open the dialog through `openFreshStartDialog()`, which clears any error left by a different recovery action before setting `freshStartDialogOpen.value = true`.

- [ ] **Step 4: Run focused tests and typecheck**

Run: `corepack pnpm vitest run src/app/LibraryFreshStartDialog.test.ts`

Expected: PASS.

Run: `corepack pnpm typecheck`

Expected: PASS.

### Task 2: Prevent smoke cleanup from orphaning a GUI with deleted profile data

**Files:**
- Modify: `scripts/windows-installer-smoke-inner.ps1`
- Modify: `scripts/windows-installer-smoke-cleanup.ps1`
- Modify: `tests/windows-installer-smoke-isolation.test.ts`
- Modify: `scripts/windows-installer-smoke-cleanup.tests.ps1`

**Interfaces:**
- Consumes: canonical `$installRoot` and `$smokeRoot` paths already created by the inner smoke runner.
- Produces: `Stop-OwnedSmokeProcesses([string]$InstallRoot): bool` and `Test-OwnedSmokeProcessPresent([string]$Root): bool`.

- [ ] **Step 1: Add failing contract assertions**

Extend `tests/windows-installer-smoke-isolation.test.ts` with assertions that the inner runner calls an owned-path process sweep before uninstall and that current-root cleanup refuses live owned processes:

```ts
expect(inner).toContain('Stop-OwnedSmokeProcesses -InstallRoot $installRoot')
expect(cleanup).toContain('function Test-OwnedSmokeProcessPresent')
expect(cleanup).toContain('if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { return $false }')
expect(inner).not.toContain('Stop-Process -Name')
```

- [ ] **Step 2: Run the smoke isolation contract and verify it fails**

Run: `corepack pnpm vitest run tests/windows-installer-smoke-isolation.test.ts`

Expected: FAIL because the owned-path sweep and live-process cleanup guard do not exist.

- [ ] **Step 3: Add path-scoped live-process detection**

Add to `scripts/windows-installer-smoke-cleanup.ps1`:

```powershell
function Test-OwnedSmokeProcessPresent {
  param([Parameter(Mandatory)][string]$Root)
  $canonicalRoot = [IO.Path]::GetFullPath($Root).TrimEnd('\')
  $prefix = "$canonicalRoot\"
  foreach ($process in @(Get-Process -ErrorAction Stop)) {
    try { $canonicalPath = [IO.Path]::GetFullPath([string]$process.Path) } catch { continue }
    if ($canonicalPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { return $true }
  }
  return $false
}
```

Before recursive `Remove-Item`, make both stale and current cleanup refuse a root with a live owned process:

```powershell
if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { continue }
if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { return $false }
```

- [ ] **Step 4: Add deterministic owned-process shutdown to the inner runner**

Add to `scripts/windows-installer-smoke-inner.ps1`:

```powershell
function Stop-OwnedSmokeProcesses {
  param([Parameter(Mandatory)][string]$InstallRoot)
  $canonicalRoot = [IO.Path]::GetFullPath($InstallRoot).TrimEnd('\')
  $prefix = "$canonicalRoot\"
  foreach ($process in @(Get-Process -ErrorAction Stop)) {
    try { $canonicalPath = [IO.Path]::GetFullPath([string]$process.Path) } catch { continue }
    if (-not $canonicalPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { continue }
    Stop-Process -InputObject $process -Force -ErrorAction SilentlyContinue
  }
  $deadline = [DateTime]::UtcNow.AddSeconds(10)
  while ([DateTime]::UtcNow -lt $deadline) {
    if (-not (Test-OwnedSmokeProcessPresent -Root $canonicalRoot)) { return $true }
    Start-Sleep -Milliseconds 200
  }
  return -not (Test-OwnedSmokeProcessPresent -Root $canonicalRoot)
}
```

Call it in `finally` after the recorded-process loop and before invoking the uninstaller:

```powershell
if (-not (Stop-OwnedSmokeProcesses -InstallRoot $installRoot)) {
  $failureCodes += 'owned_process_cleanup_failed'
}
```

- [ ] **Step 5: Cover cleanup refusal with a live executable fixture**

In `scripts/windows-installer-smoke-cleanup.tests.ps1`, create a current smoke root, copy a harmless long-running executable into its `installed` directory, start it, assert `Remove-OwnedCurrentSmokeRoot` returns false and preserves the root, then stop the process and assert cleanup succeeds. Use the current PowerShell executable only as the copied fixture and stop it in `finally`.

- [ ] **Step 6: Run smoke-script tests**

Run: `corepack pnpm vitest run tests/windows-installer-smoke-isolation.test.ts tests/windows-installer-smoke-selection.test.ts`

Expected: PASS.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows-installer-smoke-cleanup.tests.ps1`

Expected: output contains `Owned stale smoke cleanup rejection matrix passed` and exit code 0.

### Task 3: Full verification

**Files:**
- Verify only; no additional source files.

**Interfaces:**
- Consumes: completed Vue and smoke-script changes.
- Produces: a release-ready `0.1.2` source tree with regression coverage.

- [ ] **Step 1: Run lint and the targeted suite**

Run: `corepack pnpm lint`

Expected: PASS with zero warnings.

Run: `corepack pnpm vitest run src/app/LibraryFreshStartDialog.test.ts tests/windows-installer-smoke-isolation.test.ts tests/windows-installer-smoke-selection.test.ts`

Expected: PASS.

- [ ] **Step 2: Review the diff for scope and safety**

Run: `git diff -- src/app/LibraryFreshStartDialog.vue src/app/App.vue src/app/LibraryFreshStartDialog.test.ts scripts/windows-installer-smoke-inner.ps1 scripts/windows-installer-smoke-cleanup.ps1 scripts/windows-installer-smoke-cleanup.tests.ps1 tests/windows-installer-smoke-isolation.test.ts`

Expected: only inline error visibility and owned-path smoke process cleanup changes; no Rust recovery-boundary changes and no version change.

### Task 4: Bootstrap a missing application control root before evidence inspection

**Files:**
- Modify: `src-tauri/src/infrastructure/storage_location.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/storage_location.rs`

**Interfaces:**
- Consumes: the Tauri-resolved `app.path().app_data_dir()` path.
- Produces: `prepare_control_root(control_root: &Path) -> Result<(), StorageLocationError>`, which creates a missing product control directory and then validates that the resulting directory is not a symlink or Windows reparse point.

- [ ] **Step 1: Add a failing missing-control-root regression test**

Add a test that creates a nonexistent child path under a temporary parent, calls `prepare_control_root`, then runs startup with retained complete credentials and verifies `LocalDataMissing` rather than `ResetIncomplete`:

```rust
#[test]
fn missing_control_root_is_prepared_before_retained_credentials_are_classified() {
    let parent = tempdir().unwrap();
    let control = parent.path().join("com.mistaketrainer.next");
    let secrets = MemorySecretStore::default();
    seed_complete_credentials(&secrets);

    prepare_control_root(&control).expect("missing product control root should be created safely");
    let startup = initialize_configured_application_library_if_accessible(&control, &secrets, 100)
        .expect("retained credentials with missing data are recoverable");

    assert!(matches!(
        startup,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
    ));
    assert!(control.is_dir());
    assert!(!control.join("library").exists());
}
```

- [ ] **Step 2: Run the Rust test and verify it fails to compile**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location missing_control_root_is_prepared_before_retained_credentials_are_classified`

Expected: FAIL because `prepare_control_root` does not exist.

- [ ] **Step 3: Implement safe control-root preparation and call it at Tauri setup**

Add this public helper to `storage_location.rs`:

```rust
pub fn prepare_control_root(control_root: &Path) -> Result<(), StorageLocationError> {
    match fs::create_dir(control_root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(StorageLocationError::File(error)),
    }
    validate_control_root(control_root)
}
```

Call it immediately after resolving `control_root` in `lib.rs`, before any control-file inspection:

```rust
infrastructure::storage_location::prepare_control_root(&control_root)?;
```

- [ ] **Step 4: Verify startup and storage behavior**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test storage_location`

Expected: PASS.

Run: `corepack pnpm test`

Expected: PASS.

- [ ] **Step 5: Rebuild and reinstall 0.1.2**

Run: `corepack pnpm tauri build`

Expected: NSIS produces `src-tauri/target/release/bundle/nsis/Mistake Trainer Next_0.1.2_x64-setup.exe`.
