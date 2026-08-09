import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const backupPath = 'src/app/BackupRestoreDialog.vue'
const legacyDialogPath = 'src/modules/legacy/components/LegacyImportDialog.vue'
const legacyPanelPath = 'src/modules/legacy/components/LegacyImportPanel.vue'
const legacyResultPath = 'src/modules/legacy/components/LegacyImportResult.vue'
const componentPaths = [backupPath, legacyDialogPath, legacyPanelPath, legacyResultPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('backup and legacy safety readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps backup restore controls touch-safe', () => {
    expect(declarations(backupPath, '.close-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(backupPath, '.dialog-actions button')).toContain('min-height:44px')
  })

  it('keeps legacy migration actions touch-safe', () => {
    expect(declarations(legacyDialogPath, '.close-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(legacyDialogPath, '.dialog-actions button')).toContain('min-height:44px')
    expect(declarations(legacyPanelPath, 'button')).toContain('min-height:44px')
    expect(declarations(legacyResultPath, '.result-actions a,.result-actions button')).toContain('min-height:44px')
  })
})
