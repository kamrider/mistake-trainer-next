import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const script = (name: string) => readFileSync(resolve('scripts', name), 'utf8')

describe('Windows installer smoke isolation', () => {
  it('requires an explicitly ephemeral inner runner and a kill-on-close job', () => {
    const inner = script('windows-installer-smoke-inner.ps1')
    const job = script('windows-job-object.ps1')

    expect(inner).toContain("$env:CI -ne 'true'")
    expect(inner).toContain("$env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS -ne '1'")
    expect(inner).toContain('New-KillOnCloseJob')
    expect(inner).toContain('Start-ProcessInJob')
    expect(inner).not.toContain('Stop-Process -Name')
    expect(job).toContain('JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE')
    expect(job).toContain('CREATE_SUSPENDED')
    expect(job).toContain('AssignProcessToJobObject')
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
    expect(inner).not.toContain("Get-Process -Name 'mistake-trainer-next'")
  })

  it('always uninstalls recorded binaries and verifies isolated data preservation', () => {
    const inner = script('windows-installer-smoke-inner.ps1')
    expect(inner).toContain('$script:launchedProcesses')
    expect(inner).toContain('if (-not $recordedProcess.HasExited)')
    expect(inner).toContain('Start-Sleep -Seconds 10')
    expect(inner).toContain('same-version reinstall')
    expect(inner).toContain('installer-preservation-sentinel.bin')
    expect(inner).toContain('uninstall_data_preservation_failed')
    expect(inner).toContain('$cleanupJob = New-KillOnCloseJob')
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
