import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const roomPath = 'src/modules/review/components/ReviewRoom.vue'
const focusPath = 'src/modules/review/components/SchulteFocus.vue'
const lightboxPath = 'src/modules/review/components/ReviewMediaLightbox.vue'
const componentPaths = [roomPath, focusPath, lightboxPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('review interaction readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps review-room and focus actions touch-safe', () => {
    expect(declarations(roomPath, '.icon-action')).toMatch(/width:44px;height:44px/)
    expect(declarations(roomPath, '.review-header')).toContain('grid-template-columns:44px')
    expect(source(roomPath)).not.toContain('grid-template-columns: 42px')
    expect(declarations(roomPath, '.mode-toggle')).toContain('min-height:44px')
    expect(declarations(focusPath, '.exit-focus')).toContain('min-height:44px')
    expect(declarations(focusPath, '.skip-focus')).toContain('min-height:44px')
  })

  it('keeps lightbox controls touch-safe', () => {
    expect(declarations(lightboxPath, '.lightbox-close')).toMatch(/width:44px;height:44px/)
    expect(declarations(lightboxPath, '.lightbox-controls button')).toContain('min-height:44px')
  })
})
