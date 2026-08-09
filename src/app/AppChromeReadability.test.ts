import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const componentPaths = [
  'src/app/App.vue',
  'src/app/AppShell.vue',
  'src/app/LibraryAccessScreen.vue',
  'src/app/views/NotFoundView.vue',
  'src/app/components/SettingsSectionNav.vue',
  'src/modules/profiles/components/ProfileSwitcher.vue',
]
const tokenPath = 'src/shared/styles/tokens.css'

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

function numericDeclaration(componentPath: string, selector: string, property: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`${escapedSelector}\\s*\\{[^}]*${property}:\\s*(-?\\d+)`))
  if (!match?.[1]) throw new Error(`Missing numeric ${property} for ${selector} in ${componentPath}`)
  return Number(match[1])
}

describe('application chrome readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps global navigation, notices, recovery, and access actions touch-safe', () => {
    expect(declarations('src/app/AppShell.vue', '.nav-item')).toContain('min-height:44px')
    expect(source('src/app/AppShell.vue')).toContain('activeNavigationIndex * 49')
    expect(declarations('src/app/App.vue', '.restore-notice button')).toMatch(/width:44px;height:44px/)
    expect(declarations('src/app/App.vue', '.route-error button')).toContain('min-height:44px')
    expect(declarations('src/app/LibraryAccessScreen.vue', '.access-primary, .access-secondary')).toContain('min-height:46px')
  })

  it('keeps the profile popover above page chrome with explicit action targets', () => {
    const pageChromeLayer = numericDeclaration('src/app/components/SettingsSectionNav.vue', '.settings-section-nav', 'z-index')
    const railLayer = numericDeclaration('src/app/AppShell.vue', '.side-rail', 'z-index')
    const noticeLayer = numericDeclaration('src/app/App.vue', '.global-notice-stack', 'z-index')
    const dialogLayer = numericDeclaration('src/app/BackupRestoreDialog.vue', '.restore-backdrop', 'z-index')

    expect(pageChromeLayer).toBeLessThan(railLayer)
    expect(railLayer).toBeLessThan(noticeLayer)
    expect(railLayer).toBeLessThan(dialogLayer)
    expect(declarations('src/modules/profiles/components/ProfileSwitcher.vue', '.rename-button, .delete-button'))
      .toMatch(/width:44px;height:44px/)
  })

  it('keeps remaining application-chrome actions at the 44px baseline', () => {
    expect(declarations('src/app/components/SettingsSectionNav.vue', '.directory-scroll'))
      .toMatch(/width:44px;height:44px/)
    expect(declarations('src/app/views/NotFoundView.vue', '.not-found button'))
      .toContain('min-height:44px')
  })

  it('keeps desktop settings and report actions at the 44px baseline', () => {
    const baselines = [
      ['src/app/views/SettingsView.vue', 'button'],
      ['src/app/components/SettingsBackupPanel.vue', 'button'],
      ['src/app/components/SettingsStoragePanel.vue', 'button'],
      ['src/app/components/SettingsUpdatePanel.vue', 'button'],
      ['src/app/views/ReportView.vue', '.page-heading button'],
    ] as const

    for (const [componentPath, selector] of baselines) {
      expect(declarations(componentPath, selector), `${componentPath} ${selector}`)
        .toContain('min-height:44px')
    }
  })

  it('stacks simultaneous global notices inside one bounded viewport layer', () => {
    const stack = declarations('src/app/App.vue', '.global-notice-stack')

    expect(stack).toContain('position:fixed')
    expect(stack).toContain('display:grid')
    expect(stack).toContain('gap:10px')
    expect(stack).toContain('max-height:calc(100vh-40px)')
    expect(stack).toContain('overflow:auto')
    expect(declarations('src/app/App.vue', '.restore-notice')).not.toContain('position:fixed')
  })

  it('keeps programmatic route context quiet without weakening interactive focus', () => {
    expect(declarations('src/app/App.vue', '.route-page:focus,.route-page h1[tabindex="-1"]:focus'))
      .toContain('outline:none')
    expect(source(tokenPath)).toContain(':focus-visible { outline: 3px solid')
  })
})
