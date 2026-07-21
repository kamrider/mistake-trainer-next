import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('generated Tauri bindings', () => {
  it('contains the registered system command and public result contract', async () => {
    const path = resolve(process.cwd(), 'src/shared/api/bindings.ts')
    const bindings = await readFile(path, 'utf8')

    expect(bindings).toContain('export const commands')
    expect(bindings).toContain('systemStatus')
    expect(bindings).toContain('dashboardOverview')
    expect(bindings).toContain('DashboardOverview')
    expect(bindings).toContain('captureLanPreflight')
    expect(bindings).toContain('captureLanFirewallRepair')
    expect(bindings).not.toContain('captureLanOpenNetworkSettings')
    expect(bindings).not.toContain('CaptureLanSettingsPage')
    expect(bindings).toContain('CaptureLanPreflight')
    expect(bindings).toContain('captureItemStageRole')
    expect(bindings).toContain('captureCardMerge')
    expect(bindings).toContain('captureDraftDelete')
    expect(bindings).toContain('captureBatchAssignSubject')
    expect(bindings).toContain('subjectPreferencesGet')
    expect(bindings).toContain('subjectPreferencesSave')
    expect(bindings).toContain('SubjectPreferences')
    expect(bindings).toContain('reviewManualStart')
    expect(bindings).toContain('ReviewManualStartInput')
    expect(bindings).toContain('reviewQueue: () =>')
    expect(bindings).not.toContain('reviewQueue: (problemId:')
    expect(bindings).toContain('captureSoundEnabled')
    expect(bindings).toContain('newDraftSubject')
    expect(bindings).toContain('stagedRole')
    expect(bindings).not.toContain('captureDraftCreate')
    expect(bindings).toContain('AppResult')
    expect(bindings).toContain('diagnosticId')
    expect(bindings).not.toMatch(/\n\n$/)
  })
})
