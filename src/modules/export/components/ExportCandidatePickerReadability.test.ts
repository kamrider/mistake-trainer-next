import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const pickerPath = 'src/modules/export/components/ExportCandidatePicker.vue'

function source() {
  return readFileSync(resolve(pickerPath), 'utf8')
}

function declarations(selector: string) {
  const compact = source().replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('export candidate picker readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = [...source().matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
      .filter(match => Number(match[1]) < 12)
      .map(match => match[0])

    expect(violations).toEqual([])
  })

  it('keeps every named picker interaction touch-safe', () => {
    expect(declarations('.source-card')).toContain('min-height:82px')
    expect(declarations('.search-field input')).toContain('min-height:44px')
    expect(declarations('.picker-toolbar button')).toContain('min-height:44px')
    expect(declarations('.candidate-row')).toContain('min-height:84px')
  })
})
