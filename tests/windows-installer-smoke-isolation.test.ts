import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const script = (name: string) => readFileSync(resolve('scripts', name), 'utf8')

describe('Windows installer smoke isolation', () => {
  it('requires an explicitly ephemeral inner runner and tracks only launched processes', () => {
    const inner = script('windows-installer-smoke-inner.ps1')

    expect(inner).toContain("$env:CI -ne 'true'")
    expect(inner).toContain("$env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS -ne '1'")
    expect(inner).toContain('function Start-SmokeProcess')
    expect(inner).toContain('$script:launchedProcesses.Add($process)')
    expect(inner).not.toContain('Start-ProcessInJob')
    expect(inner).not.toContain('New-KillOnCloseJob')
    expect(inner).not.toContain('Stop-Process -Name')
  })

  it('routes local execution through Sandbox with disabled integration surfaces', () => {
    const host = script('windows-installer-smoke.ps1')
    const sandbox = script('windows-installer-smoke-sandbox.ps1')
    const guest = script('windows-installer-smoke-guest.ps1')

    expect(host).toContain("$env:CI -eq 'true'")
    expect(host).toContain('windows-installer-smoke-sandbox.ps1')
    expect(sandbox).toContain('Containers-DisposableClientVM')
    expect(sandbox).toContain('<Networking>Disable</Networking>')
    expect(sandbox).toContain('<ClipboardRedirection>Disable</ClipboardRedirection>')
    expect(guest).toContain("$env:CI = 'true'")
    expect(guest).toContain("$env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS = '1'")
  })

  it('uses an exact owned run marker before recursive cleanup', () => {
    const inner = script('windows-installer-smoke-inner.ps1')
    const cleanup = script('windows-installer-smoke-cleanup.ps1')
    expect(inner).toContain('.mistake-trainer-installer-smoke.json')
    expect(inner).toContain('Remove-OwnedCurrentSmokeRoot')
    expect(inner).toContain('Remove-OwnedStaleSmokeRoot')
    expect(cleanup).toContain('TotalHours -le 24')
    expect(cleanup).toContain('[IO.FileAttributes]::ReparsePoint')
    expect(cleanup).toContain('(Split-Path -Parent $canonicalCandidate) -cne $canonicalRunner')
    expect(cleanup).toContain('function Test-OwnedSmokeProcessPresent')
    expect(cleanup).toContain('if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { continue }')
    expect(cleanup).toContain('if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { return $false }')
    expect(inner).not.toContain("Get-Process -Name 'mistake-trainer-next'")
  })

  it('always uninstalls recorded binaries and verifies isolated data preservation', () => {
    const inner = script('windows-installer-smoke-inner.ps1')
    expect(inner).toContain('$script:launchedProcesses')
    expect(inner).toContain('if (-not $recordedProcess.HasExited)')
    expect(inner).toContain('Stop-OwnedSmokeProcesses -InstallRoot $installRoot')
    expect(inner).toContain('Stop-Process -InputObject $process')
    expect(inner).not.toContain('Stop-Process -Name')
    expect(inner).toContain('Start-Sleep -Seconds 10')
    expect(inner).toContain('same-version reinstall')
    expect(inner).toContain('installer-preservation-sentinel.bin')
    expect(inner).toContain("Join-Path $libraryPath 'library.db'")
    expect(inner).toContain("Assert-Smoke (-not (Test-Path -LiteralPath $startupFailurePath")
    expect(inner).not.toContain('first run did not create the isolated encrypted library')
    expect(inner).toContain('uninstall_data_preservation_failed')
    expect(inner).toContain('$install = Start-SmokeProcess')
    expect(inner).toContain('$reinstall = Start-SmokeProcess')
    expect(inner).toContain('$uninstall = Start-SmokeProcess')
    expect(inner).not.toContain('$installerJob')
    expect(inner).not.toContain('$cleanupJob')
    expect(inner).toContain('$allowedFailureStages -ccontains $failureStage')
    expect(inner).toContain('installer_smoke_$boundedStage')
    expect(inner).toContain("if ($status -ne 'passed') { exit 1 }")
    expect(inner).toMatch(/Write-Output 'Windows installer smoke passed'\s+exit 0/)
    expect(inner).toContain("$failureStage = 'self_check_exit'")
    expect(inner).toContain("$failureStage = 'self_check_report'")
    expect(inner).toContain("$failureStage = 'product_check_exit'")
    expect(inner).toContain("$failureStage = 'product_check_report'")
    expect(inner.indexOf('$env:APPDATA = $isolatedAppData')).toBeGreaterThan(
      inner.indexOf("$failureStage = 'product_check_ready'"),
    )
    expect(inner).toContain('Wait-MainWindow $firstProcess 60')
    expect(inner).toContain('$firstProcess = Start-Process -FilePath $applicationPath -PassThru')
    expect(inner).toContain('$script:launchedProcesses.Add($firstProcess)')
    expect(inner).toContain('$second = Start-Process -FilePath $applicationPath -PassThru')
    expect(inner).toContain("{ 'gui_early_exit' } else { 'gui_window' }")
  })

  it('fingerprints host credentials and installed binaries even when the guest fails', () => {
    const sandbox = script('windows-installer-smoke-sandbox.ps1')
    const workflow = readFileSync(resolve('.github/workflows/ci.yml'), 'utf8')
    expect(sandbox).toContain('credentialTargets')
    expect(sandbox).toContain('executables')
    expect(sandbox).toContain('Read-BoundedSmokeResult')
    expect(sandbox).toContain('$hostUnchanged = (Get-ProductionFingerprint) -ceq $hostBefore')
    expect(workflow).toContain('Upload bounded installer smoke failure result')
    expect(workflow).toContain('mistake-trainer-installer-result-x86_64\\result.json')
  })
})
