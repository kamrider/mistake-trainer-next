# Installer Smoke Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure Windows installer smoke testing cannot attach to a production instance, touch production AppData or credentials, or leave an orphaned GUI when the test shell is interrupted.

**Architecture:** Separate the host launcher from the inner production-identity smoke runner. Local runs execute inside Windows Sandbox; CI runs only on an explicitly ephemeral worker. Inside the guest, put every installer/application/uninstaller process in one Windows Job Object with `KILL_ON_JOB_CLOSE`, use owned run markers for cleanup, and prove production host state remains unchanged.

**Tech Stack:** PowerShell 7/Windows PowerShell, Windows Sandbox, Windows Job Objects through a small C# interop helper, GitHub Actions Windows runners, Vitest source contracts, Tauri NSIS artifacts.

## Global Constraints

- A production-identity GUI smoke test must never run directly in a normal developer Windows session.
- Local production-identity smoke requires Windows Sandbox; if Sandbox is unavailable, the command fails with instructions to use CI.
- CI smoke requires both `CI=true` and `MISTAKE_TRAINER_EPHEMERAL_WINDOWS=1`; either value missing is a hard failure before installer launch.
- All spawned installer, application, helper, and uninstaller processes belong to one Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
- Cleanup may remove only a canonical directory beneath the selected runner temp whose basename matches `mistake-trainer-installer-smoke-<32 lowercase hex>` and whose ownership marker matches the same run ID.
- The smoke runner never kills a process by image name; it may stop only PIDs it created and assigned to its Job Object.
- Host AppData, LocalAppData, Windows Credential Manager, installed-program registry keys, Start Menu shortcuts, and running production application remain byte-for-byte or presence-for-presence unchanged.
- The inner smoke still installs and exercises the actual production artifact; it does not substitute a test build.

---

## File Structure

- Create `scripts/windows-job-object.ps1`: Job Object interop, child assignment, and deterministic handle disposal.
- Create `scripts/windows-installer-smoke-inner.ps1`: current smoke logic, allowed only in an ephemeral guest/runner.
- Replace `scripts/windows-installer-smoke.ps1`: host orchestrator that selects CI-inner or Windows-Sandbox execution.
- Create `scripts/windows-installer-smoke-sandbox.ps1`: generate a `.wsb`, map read-only inputs/read-write results, and wait for a signed result file.
- Create `scripts/windows-installer-smoke-guest.ps1`: command executed by Windows Sandbox after logon.
- Modify `.github/workflows/ci.yml`: assert ephemeral mode before x64/ARM64 smoke.
- Modify `scripts/windows-signed-release.ps1`: call the host orchestrator and reject unsafe local fallback.
- Expand `tests/windows-installer-smoke-selection.test.ts`: source-level safety contracts.
- Create `tests/windows-installer-smoke-isolation.test.ts`: host/inner/job-object contracts.
- Modify `docs/windows-release-runbook.md`: execution and interruption semantics.

### Task 1: Put all child processes in a kill-on-close Job Object

**Files:**
- Create: `scripts/windows-job-object.ps1`
- Create: `scripts/windows-job-object.tests.ps1`

**Interfaces:**
- Produces: `New-KillOnCloseJob`, `Start-ProcessInJob`, `Wait-JobProcessExit`, and `Close-KillOnCloseJob`.

- [ ] **Step 1: Write a failing interruption test**

The test launches a nested PowerShell process that sleeps for 120 seconds, assigns it to the Job Object, writes its PID to a temporary file, then terminates the owner process without calling cleanup. The parent test waits up to five seconds and asserts `Get-Process -Id <pid>` returns nothing.

- [ ] **Step 2: Run the Job Object test**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-job-object.tests.ps1`

Expected: FAIL because the helper does not exist and an ordinary `Start-Process` child survives abrupt owner termination.

- [ ] **Step 3: Implement the interop helper**

Use `Add-Type` once to define `CreateJobObjectW`, `SetInformationJobObject`, `CreateProcessW`, `AssignProcessToJobObject`, `ResumeThread`, `TerminateProcess`, and `CloseHandle`. Set `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.BasicLimitInformation.LimitFlags` to `0x00002000`. `Start-ProcessInJob` calls a C# `StartAssigned` method that creates the process with `CREATE_SUSPENDED`, assigns it to the Job Object before any application code can run, resumes the primary thread only after assignment succeeds, and terminates the suspended child on failure. `Close-KillOnCloseJob` closes the safe handle exactly once.

```powershell
function Start-ProcessInJob {
  param(
    [Parameter(Mandatory)]$Job,
    [Parameter(Mandatory)][string]$FilePath,
    [string[]]$ArgumentList = @()
  )
  $commandLine = [MistakeTrainer.JobObjects]::QuoteCommandLine($FilePath, $ArgumentList)
  $pid = [MistakeTrainer.JobObjects]::StartAssigned($Job.Handle, $FilePath, $commandLine)
  [System.Diagnostics.Process]::GetProcessById($pid)
}
```

- [ ] **Step 4: Run normal-exit and abrupt-exit tests**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-job-object.tests.ps1`

Expected: PASS for normal wait, explicit close, abrupt owner death, already-exited child, and failed assignment; no unrelated `powershell.exe` process is stopped.

- [ ] **Step 5: Commit the process containment primitive**

```powershell
git add scripts/windows-job-object.ps1 scripts/windows-job-object.tests.ps1
git commit -m "test: contain Windows smoke processes in a Job Object"
```

### Task 2: Extract an ephemeral-only inner smoke runner

**Files:**
- Create: `scripts/windows-installer-smoke-inner.ps1`
- Modify: `scripts/windows-installer-smoke.ps1`
- Modify: `tests/windows-installer-smoke-selection.test.ts`
- Create: `tests/windows-installer-smoke-isolation.test.ts`

**Interfaces:**
- Consumes: Task 1 Job Object helpers and the current installer selection contract.
- Produces: an inner runner that accepts `InstallerDirectory`, `ExpectedArchitecture`, `RunId`, and `ResultDirectory`.

- [ ] **Step 1: Add failing inner-runner safety contracts**

```ts
expect(inner).toContain("$env:CI -eq 'true'")
expect(inner).toContain("$env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS -eq '1'")
expect(inner).toContain('New-KillOnCloseJob')
expect(inner).toContain('Start-ProcessInJob')
expect(inner).not.toContain('Stop-Process -Name')
expect(inner).not.toContain("Get-Process -Name 'mistake-trainer-next'")
```

- [ ] **Step 2: Run source contracts**

Run: `pnpm test -- tests/windows-installer-smoke-selection.test.ts tests/windows-installer-smoke-isolation.test.ts`

Expected: FAIL because the current monolithic script permits direct local execution and uses ordinary `Start-Process`.

- [ ] **Step 3: Move the existing smoke sequence into the inner runner**

Before resolving the installer, require both ephemeral environment markers. Validate `RunId` against `^[0-9a-f]{32}$`. Create `$smokeRoot` from the fixed prefix plus `RunId`, then write `.mistake-trainer-installer-smoke.json` with schema version 1, run ID, current owner PID, and UTC creation time. Replace every installer, self-check, product-check, first GUI, second GUI, and uninstaller launch with `Start-ProcessInJob`.

The `finally` order is: request normal GUI close; wait ten seconds; stop only still-running recorded PIDs; close the Job Object; run the recorded uninstaller in a new short-lived Job Object if it still exists; validate the ownership marker and canonical containment; remove the exact smoke root; write `result.json` atomically.

- [ ] **Step 4: Preserve the exact installer selection contract**

Keep manifest version and requested architecture selection unchanged. Require exactly one installed application executable and one uninstaller. Assert each GUI process path canonicalizes beneath `$installRoot` before waiting for its window.

- [ ] **Step 5: Run PowerShell syntax and Vitest contracts**

Run: `powershell -NoProfile -Command "[scriptblock]::Create((Get-Content .\scripts\windows-installer-smoke-inner.ps1 -Raw)) | Out-Null"`

Expected: PASS.

Run: `pnpm test -- tests/windows-installer-smoke-selection.test.ts tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit the ephemeral inner runner**

```powershell
git add scripts/windows-installer-smoke-inner.ps1 scripts/windows-installer-smoke.ps1 tests/windows-installer-smoke-selection.test.ts tests/windows-installer-smoke-isolation.test.ts
git commit -m "test: require an ephemeral Windows installer smoke runner"
```

### Task 3: Run local smoke only inside Windows Sandbox

**Files:**
- Create: `scripts/windows-installer-smoke-sandbox.ps1`
- Create: `scripts/windows-installer-smoke-guest.ps1`
- Modify: `scripts/windows-installer-smoke.ps1`
- Modify: `tests/windows-installer-smoke-isolation.test.ts`

**Interfaces:**
- Consumes: Task 2 inner runner.
- Produces: a host wrapper that uses CI directly or launches `WindowsSandbox.exe` with mapped folders.

- [ ] **Step 1: Add failing host-mode contracts**

Assert that the host wrapper has exactly two paths: CI invokes the inner runner only when both ephemeral markers are present; non-CI invokes the Sandbox wrapper. Assert there is no `-ForceLocal`, `-SkipSandbox`, or direct-inner fallback parameter.

- [ ] **Step 2: Run source contracts**

Run: `pnpm test -- tests/windows-installer-smoke-isolation.test.ts`

Expected: FAIL until host/sandbox/guest scripts exist.

- [ ] **Step 3: Implement the Sandbox host wrapper**

Check the `Containers-DisposableClientVM` optional feature state and `WindowsSandbox.exe`. If unavailable, fail before installation with `本机未启用 Windows Sandbox；请在 CI 的临时 Windows runner 中执行安装器冒烟测试。`. Create a host result directory beneath the runner temp, copy only the selected installer plus the four smoke scripts into a read-only input directory, and generate a `.wsb` with networking disabled, clipboard redirection disabled, printer redirection disabled, input mapped read-only, and results mapped read-write.

The LogonCommand runs `windows-installer-smoke-guest.ps1`. The guest sets `CI=true` and `MISTAKE_TRAINER_EPHEMERAL_WINDOWS=1`, invokes the inner runner, writes its result, and shuts down the sandbox. The host accepts success only when `result.json` has the expected run ID, architecture, installer SHA-256, `status: "passed"`, and no failure codes.

- [ ] **Step 4: Add host-state fingerprints**

Before Sandbox launch, record only these non-secret facts: existence and length/last-write time of the production control files, SHA-256 of `storage-location.json` when present, matching credential target names without values, installed executable path/hash, and running production PIDs. After Sandbox exit, require the fingerprint to match exactly. Never read credential values.

- [ ] **Step 5: Run syntax and source contracts**

Run: `powershell -NoProfile -Command "@('windows-installer-smoke.ps1','windows-installer-smoke-sandbox.ps1','windows-installer-smoke-guest.ps1','windows-installer-smoke-inner.ps1') | ForEach-Object { [scriptblock]::Create((Get-Content (Join-Path '.\scripts' $_) -Raw)) | Out-Null }"`

Expected: PASS.

Run: `pnpm test -- tests/windows-installer-smoke-selection.test.ts tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS.

- [ ] **Step 6: Perform one manual local Sandbox smoke**

Run: `.\scripts\windows-installer-smoke.ps1 -InstallerDirectory .\src-tauri\target\release\bundle\nsis -ExpectedArchitecture x86_64`

Expected: Windows Sandbox opens, runs the installed GUI twice, closes, and returns `Windows installer smoke passed`; no host `mistake-trainer-next.exe` PID, AppData file, credential target, installation directory, or Start Menu shortcut changes.

- [ ] **Step 7: Commit local Sandbox isolation**

```powershell
git add scripts/windows-installer-smoke.ps1 scripts/windows-installer-smoke-sandbox.ps1 scripts/windows-installer-smoke-guest.ps1 tests/windows-installer-smoke-isolation.test.ts
git commit -m "test: isolate local installer smoke in Windows Sandbox"
```

### Task 4: Recover only owned stale smoke directories

**Files:**
- Modify: `scripts/windows-installer-smoke-inner.ps1`
- Create: `scripts/windows-installer-smoke-cleanup.tests.ps1`
- Modify: `tests/windows-installer-smoke-isolation.test.ts`

**Interfaces:**
- Produces: `Remove-OwnedStaleSmokeRoot` with strict containment, ownership, age, and process-liveness checks.

- [ ] **Step 1: Write stale-cleanup rejection tests**

Create fixtures for a valid stale root, missing marker, mismatched run ID, uppercase/noncanonical name, symlink/reparse root, live owner PID with matching start time, reused owner PID with a different start time, age under 24 hours, path outside runner temp, and an unrelated sibling. Only the valid stale root and the reused-PID stale root may be removed.

- [ ] **Step 2: Run cleanup tests**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-installer-smoke-cleanup.tests.ps1`

Expected: FAIL because stale cleanup does not exist.

- [ ] **Step 3: Implement conservative scavenging**

At inner-runner startup, enumerate only immediate child directories of canonical `$runnerTemp` matching the fixed regex. For each, reject reparse points, require a regular ownership marker no larger than 4 KiB, require matching schema/run ID, require age greater than 24 hours, and require the recorded owner PID to be absent or to have a different process start time. Resolve the final absolute path and require its parent to equal canonical runner temp before `Remove-Item -LiteralPath <exact> -Recurse -Force`.

- [ ] **Step 4: Run cleanup and source contracts**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-installer-smoke-cleanup.tests.ps1`

Expected: PASS; every rejected fixture remains byte-for-byte unchanged.

Run: `pnpm test -- tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit owned cleanup**

```powershell
git add scripts/windows-installer-smoke-inner.ps1 scripts/windows-installer-smoke-cleanup.tests.ps1 tests/windows-installer-smoke-isolation.test.ts
git commit -m "test: clean only owned stale installer smoke roots"
```

### Task 5: Enforce ephemeral smoke in CI and signed release workflows

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/windows-signed-release.ps1`
- Modify: `scripts/windows-release-contract.test.ps1`
- Modify: `docs/windows-release-runbook.md`

**Interfaces:**
- Consumes: safe host/inner scripts.
- Produces: explicit workflow environment markers and documented local/CI execution paths.

- [ ] **Step 1: Add failing workflow contracts**

Require every direct inner-runner workflow step to set both `CI: 'true'` and `MISTAKE_TRAINER_EPHEMERAL_WINDOWS: '1'`. Require signed release automation to call only `windows-installer-smoke.ps1`, never the inner script. Require the runbook to forbid direct production-identity smoke on a developer desktop.

- [ ] **Step 2: Run release contracts**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-release-contract.test.ps1`

Expected: FAIL until workflow environment and runbook contracts are present.

- [ ] **Step 3: Update CI x64 and ARM64 jobs**

Set the two ephemeral variables on the smoke steps, retain the existing architecture-specific installer directories, and upload the inner `result.json` on failure without uploading AppData, credentials, database paths, or raw process command lines.

- [ ] **Step 4: Update signed-release orchestration and runbook**

The signed-release script calls the host wrapper. On CI it resolves to the inner runner; locally it resolves to Windows Sandbox. Document that closing Codex, a terminal, or the Sandbox host cannot leave the GUI alive because the guest Job Object owns it, and document the 24-hour owned stale-root scavenger.

- [ ] **Step 5: Run full safety gates**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-job-object.tests.ps1`

Expected: PASS.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-installer-smoke-cleanup.tests.ps1`

Expected: PASS.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows-release-contract.test.ps1`

Expected: PASS.

Run: `pnpm test -- tests/windows-installer-smoke-selection.test.ts tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit workflow enforcement**

```powershell
git add .github/workflows/ci.yml scripts/windows-signed-release.ps1 scripts/windows-release-contract.test.ps1 docs/windows-release-runbook.md
git commit -m "ci: enforce ephemeral Windows installer smoke"
```

## Self-Review

- Spec coverage: the plan prevents production-instance handoff, production AppData/credential access, orphaned GUI processes, unsafe broad process termination, and unowned temp deletion while retaining a real production-artifact smoke.
- Placeholder scan: host, guest, inner runner, Job Object flags, marker schema, cleanup predicate, CI markers, and expected commands are all explicit.
- Type consistency: `RunId`, `ResultDirectory`, `.mistake-trainer-installer-smoke.json`, `result.json`, `MISTAKE_TRAINER_EPHEMERAL_WINDOWS`, and the four Job Object helper names are consistent across tasks.
