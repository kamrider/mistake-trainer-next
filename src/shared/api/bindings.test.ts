import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('generated Tauri bindings', () => {
  it('contains the registered system command and public result contract', async () => {
    const path = resolve(process.cwd(), 'src/shared/api/bindings.ts')
    const bindings = await readFile(path, 'utf8')

    expect(bindings).toContain('export const commands')
    expect(bindings).toContain('systemStatus')
    expect(bindings).toContain('captureLanPreflight')
    expect(bindings).toContain('captureLanFirewallRepair')
    expect(bindings).toContain('captureLanOpenNetworkSettings')
    expect(bindings).toContain('CaptureLanPreflight')
    expect(bindings).toContain('AppResult')
    expect(bindings).toContain('diagnosticId')
    expect(bindings).not.toMatch(/\n\n$/)
  })
})
