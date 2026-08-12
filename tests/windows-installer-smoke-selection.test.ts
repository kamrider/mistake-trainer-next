import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('Windows installer smoke candidate selection', () => {
  it('selects the manifest version and requested architecture instead of every old installer', () => {
    const script = readFileSync(resolve('scripts/windows-installer-smoke.ps1'), 'utf8')

    expect(script).toContain("'..\\src-tauri\\tauri.conf.json'")
    expect(script).toContain('$installerArchitecture = if ($ExpectedArchitecture -eq \'arm64\')')
    expect(script).toContain(
      '$expectedInstallerName = "$($tauriConfiguration.productName)_$($tauriConfiguration.version)_$installerArchitecture-setup.exe"',
    )
    expect(script).toContain('-Filter $expectedInstallerName')
    expect(script).not.toContain("-Filter '*-setup.exe'")
    expect(script).toContain("windows-installer-smoke-sandbox.ps1")
    expect(script).toContain("MISTAKE_TRAINER_EPHEMERAL_WINDOWS")
    expect(script).not.toContain('SkipSandbox')
    expect(script).not.toContain('ForceLocal')
  })
})
