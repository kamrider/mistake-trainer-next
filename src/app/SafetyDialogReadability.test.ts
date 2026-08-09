import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const lockPath = 'src/app/LibraryLockDialog.vue'
const confirmPath = 'src/app/components/ActionConfirmDialog.vue'
const componentPaths = [lockPath, confirmPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('safety dialog readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps the library-lock close and actions touch-safe', () => {
    expect(declarations(lockPath, '.close-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(lockPath, '.dialog-actions button')).toContain('min-height:44px')
  })

  it('keeps reusable confirmation actions touch-safe', () => {
    expect(declarations(confirmPath, '.dialog-actions button')).toContain('min-height:44px')
  })
})
