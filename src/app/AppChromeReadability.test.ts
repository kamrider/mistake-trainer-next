import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const componentPaths = [
  'src/app/App.vue',
  'src/app/AppShell.vue',
  'src/app/LibraryAccessScreen.vue',
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
