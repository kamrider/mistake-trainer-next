import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const captureCardSources = [
  ['CaptureDraftCard.vue', 'src/modules/capture/components/CaptureDraftCard.vue'],
  ['CaptureThumbnail.vue', 'src/modules/capture/components/CaptureThumbnail.vue'],
] as const

function source(path: string) {
  return readFileSync(resolve(path), 'utf8')
}

function compactCss(path: string) {
  return source(path).replace(/\s+/g, ' ')
}

function ruleBody(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = css.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return match?.[1] ?? ''
}

describe('capture card editing readability contract', () => {
  it('keeps explicit visible type at 12px or larger', () => {
    const violations = captureCardSources.flatMap(([file, path]) =>
      [...source(path).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => ({ file, declaration: match[0] })),
    )

    expect(violations).toEqual([])
  })

  it('keeps card and thumbnail actions at least 44px', () => {
    const card = compactCss('src/modules/capture/components/CaptureDraftCard.vue')
    const thumbnail = compactCss('src/modules/capture/components/CaptureThumbnail.vue')

    expect(ruleBody(card, '.draft-target')).toContain('min-height:44px')
    expect(ruleBody(card, '.card-subject')).toContain('min-height:44px')
    expect(ruleBody(card, '.expand-image,.crop-image')).toContain('min-height:44px')
    expect(ruleBody(card, '.change-role')).toMatch(/width:44px;height:44px/)
    expect(ruleBody(card, '.flip-button')).toContain('min-height:44px')
    expect(ruleBody(card, '.return-image')).toContain('min-height:44px')
    expect(ruleBody(card, '.image-overlay button')).toMatch(/width:44px;height:44px/)
    expect(ruleBody(thumbnail, '.remove-button')).toMatch(/width: 44px; height: 44px/)
    expect(ruleBody(thumbnail, '.crop-button')).toMatch(/width:44px;height:44px/)
    expect(ruleBody(thumbnail, '.is-filmstrip .crop-button')).toMatch(/width:44px;height:44px/)
  })
})
