import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const subjectPath = 'src/app/components/SettingsSubjectPanel.vue'
const reviewPath = 'src/app/components/SettingsReviewPanel.vue'
const diagnosticsPath = 'src/app/components/SettingsDiagnosticsPanel.vue'
const componentPaths = [subjectPath, reviewPath, diagnosticsPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('settings learning and diagnostics readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps subject settings touch-safe at every viewport width', () => {
    expect(declarations(subjectPath, 'button')).toContain('min-height:44px')
    expect(declarations(subjectPath, '.builtin-subjects span')).toContain('min-height:44px')
    expect(declarations(subjectPath, '.custom-subjects button')).toContain('min-width:44px')
    expect(declarations(subjectPath, '.custom-subjects button')).toContain('min-height:44px')
    expect(declarations(subjectPath, '.subject-controls form input')).toContain('min-height:44px')
    expect(declarations(subjectPath, '.sound-toggle')).toContain('min-height:44px')
  })

  it('keeps review and diagnostic actions touch-safe at every viewport width', () => {
    expect(declarations(reviewPath, 'button')).toContain('min-height:44px')
    expect(declarations(diagnosticsPath, 'button')).toContain('min-height:44px')
  })
})
