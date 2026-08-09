import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const editorPath = 'src/modules/capture/components/CaptureCropEditor.vue'

function source() {
  return readFileSync(resolve(editorPath), 'utf8')
}

function compactCss() {
  return source().replace(/\s+/g, ' ')
}

function ruleBody(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = css.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return match?.[1] ?? ''
}

describe('capture crop editor readability contract', () => {
  it('keeps explicit visible type at 12px or larger', () => {
    const violations = [...source().matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
      .filter(match => Number(match[1]) < 12)
      .map(match => match[0])

    expect(violations).toEqual([])
  })

  it('keeps every crop action target at least 44px', () => {
    const css = compactCss()

    expect(ruleBody(css, '.icon-button')).toMatch(/width:44px;height:44px/)
    expect(ruleBody(css, '.crop-toolbar button,.secondary,.primary')).toContain('min-height:44px')
    expect(ruleBody(css, '.region-select')).toContain('min-height:44px')
    expect(ruleBody(css, '.region-actions')).toContain('grid-template-columns:repeat(2,44px)')
    expect(ruleBody(css, '.region-actions button')).toMatch(/width:44px;height:44px/)
    expect(ruleBody(css, '.resize-handle')).toMatch(/width:44px;height:44px/)
    expect(css).not.toContain('.crop-toolbar button{min-height:40px}')
  })

  it('keeps a precise visible marker inside each enlarged resize target', () => {
    const css = compactCss()

    expect(ruleBody(css, '.resize-handle::after')).toMatch(/width:18px;height:18px/)
    expect(ruleBody(css, '.handle-n')).toContain('top:-22px')
    expect(ruleBody(css, '.handle-se')).toContain('bottom:-22px')
    expect(css).toContain('.resize-handle::after{border-color:CanvasText')
  })
})
