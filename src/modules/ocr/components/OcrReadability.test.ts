import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const ocrSources = [
  ['OcrCapabilityPanel.vue', 'src/modules/ocr/components/OcrCapabilityPanel.vue'],
  ['CaptureRecognitionEntry.vue', 'src/modules/ocr/components/CaptureRecognitionEntry.vue'],
  ['CaptureRecognitionReview.vue', 'src/modules/ocr/components/CaptureRecognitionReview.vue'],
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

describe('OCR readability and touch contract', () => {
  it('keeps explicit visible type at 12px or larger', () => {
    const violations = ocrSources.flatMap(([file, path]) =>
      [...source(path).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => ({ file, declaration: match[0] })),
    )

    expect(violations).toEqual([])
  })

  it('keeps every OCR action target at least 44px', () => {
    const capability = compactCss('src/modules/ocr/components/OcrCapabilityPanel.vue')
    const entry = compactCss('src/modules/ocr/components/CaptureRecognitionEntry.vue')
    const review = compactCss('src/modules/ocr/components/CaptureRecognitionReview.vue')

    expect(ruleBody(capability, '.enhancement-control button')).toContain('min-height: 44px')
    expect(ruleBody(entry, '.primary-action, .secondary-action, .text-action')).toContain('min-height: 44px')
    expect(ruleBody(entry, '.secondary-link')).toContain('min-height: 44px')
    expect(ruleBody(review, '.icon-button')).toMatch(/width: 44px; height: 44px/)
    expect(ruleBody(review, '.preview-placeholder button')).toContain('min-height: 44px')
    expect(ruleBody(review, '.review-position button')).toMatch(/width: 44px; height: 44px/)
    expect(ruleBody(review, 'button')).toContain('min-height: 44px')
  })
})
