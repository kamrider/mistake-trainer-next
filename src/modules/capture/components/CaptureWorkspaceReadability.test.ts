import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const componentPaths = [
  'src/modules/capture/components/CaptureWorkspace.vue',
  'src/modules/capture/components/CaptureLayoutTemplatePanel.vue',
]

function source() {
  return componentPaths
    .map(componentPath => readFileSync(resolve(componentPath), 'utf8'))
    .join('\n')
}

function compactCss() {
  return source().replace(/\s+/g, ' ')
}

function ruleBody(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = css.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return match?.[1] ?? ''
}

function declarations(css: string, selector: string) {
  return ruleBody(css, selector).replace(/\s+/g, '')
}

describe('capture workspace readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = [...source().matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
      .filter(match => Number(match[1]) < 12)
      .map(match => match[0])

    expect(violations).toEqual([])
  })

  it('provides a 44px baseline for workspace actions and fields', () => {
    const css = compactCss()

    expect(declarations(css, 'button')).toContain('min-height:44px')
    expect(declarations(css, 'input,select')).toContain('min-height:44px')
    expect(declarations(css, '.new-batch-card button, .capture-toolbar button, .collecting-panel>button, .commit-dock button')).toContain('min-height:44px')
    expect(declarations(css, '.layout-bar>button')).toContain('min-height:44px')
    expect(declarations(css, '.batch-menu-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(css, '.batch-card h3')).toContain('padding-right:56px')
    expect(declarations(css, '.batch-menu button')).toContain('min-height:44px')
    expect(declarations(css, '.back-button')).toContain('min-height:44px')
  })

  it('keeps dense organizing actions at least 44px', () => {
    const css = compactCss()

    for (const selector of [
      '.layout-impact button',
      '.recognition-result button',
      '.batch-subject-options button',
      '.batch-subject-confirm button',
      '.pair-suggestion-panel>header button,.pair-card-action',
      '.material-actions button',
    ]) {
      expect(declarations(css, selector), selector).toContain('min-height:44px')
    }

    expect(css).toContain('.commit-dock { bottom:82px; }')
  })
})
