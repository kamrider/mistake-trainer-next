import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const componentPath = 'src/modules/sync/components/SyncConflictCenter.vue'
const componentSource = readFileSync(resolve(componentPath), 'utf8')

function declarations(selector: string) {
  const compact = componentSource.replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('sync conflict center readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = [...componentSource.matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
      .filter(match => Number(match[1]) < 12)
      .map(match => match[0])

    expect(violations).toEqual([])
  })

  it('keeps every conflict decision action touch-safe', () => {
    expect(declarations('button')).toContain('min-height:44px')
  })

  it('keeps resolved-card transitions inside the list at every viewport width', () => {
    expect(declarations('.conflict-list')).toContain('position:relative')
    expect(declarations('.conflict-card-leave-active')).toContain('width:100%')
  })
})
