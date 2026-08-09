import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const cloudPath = 'src/app/components/SettingsCloudAuthPanel.vue'
const backendPath = 'src/app/components/SettingsSyncBackendPanel.vue'
const devicePath = 'src/app/components/SettingsDeviceOverviewPanel.vue'
const componentPaths = [cloudPath, backendPath, devicePath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('settings connectivity readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps cloud account controls touch-safe at every viewport width', () => {
    expect(declarations(cloudPath, 'button')).toContain('min-height:44px')
    expect(declarations(cloudPath, '.cloud-auth-form input')).toContain('min-height:44px')
    expect(declarations(cloudPath, '.auth-mode-toggle')).toContain('min-height:44px')
  })

  it('keeps backend and device controls touch-safe at every viewport width', () => {
    expect(declarations(backendPath, 'button')).toContain('min-height:44px')
    expect(declarations(devicePath, 'button')).toContain('min-height:44px')
  })
})
