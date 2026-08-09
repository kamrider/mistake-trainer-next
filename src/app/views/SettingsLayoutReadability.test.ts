import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const settingsPath = 'src/app/views/SettingsView.vue'

function source() {
  return readFileSync(resolve(settingsPath), 'utf8')
}

function declarations(selector: string) {
  const compact = source().replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('settings page layout readability contract', () => {
  it('keeps the refresh action readable beside the heading on narrow screens', () => {
    expect(source()).toContain('class="settings-refresh"')
    expect(declarations('.settings-refresh')).toContain('flex:00auto')
    expect(declarations('.settings-refresh')).toContain('white-space:nowrap')
  })
})
