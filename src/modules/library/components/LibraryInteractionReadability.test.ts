import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const workspacePath = 'src/modules/library/components/LibraryWorkspace.vue'
const detailPath = 'src/modules/library/components/ProblemDetailDrawer.vue'
const tagEditorPath = 'src/modules/library/components/ProblemTagEditor.vue'
const componentPaths = [workspacePath, detailPath, tagEditorPath]

function source(componentPath: string) {
  return readFileSync(resolve(componentPath), 'utf8')
}

function declarations(componentPath: string, selector: string) {
  const compact = source(componentPath).replace(/\s+/g, ' ')
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = compact.match(new RegExp(`(?:^|})\\s*${escapedSelector}\\s*\\{([^}]*)\\}`))
  return (match?.[1] ?? '').replace(/\s+/g, '')
}

describe('library interaction readability contract', () => {
  it('keeps every explicit visible pixel font at 12px or larger', () => {
    const violations = componentPaths.flatMap(componentPath =>
      [...source(componentPath).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter(match => Number(match[1]) < 12)
        .map(match => `${componentPath}: ${match[0]}`),
    )

    expect(violations).toEqual([])
  })

  it('keeps library list controls touch-safe', () => {
    expect(declarations(workspacePath, '.select-all-action')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.batch-bar button')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.filter-tabs button')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.search-field input')).toContain('min-height:44px')
    expect(declarations(workspacePath, '.select-problem')).toContain('min-width:44px')
    expect(declarations(workspacePath, '.select-problem')).toContain('min-height:44px')
  })

  it('keeps drawer and tag-editor controls touch-safe', () => {
    expect(declarations(detailPath, '.icon-button')).toMatch(/width:44px;height:44px/)
    expect(declarations(detailPath, '.edit-header-button, .more-actions-button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.neighbor-actions button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.status-actions button')).toContain('min-height:44px')
    expect(declarations(detailPath, '.edit-paper input, .edit-paper textarea')).toContain('min-height:44px')
    expect(declarations(tagEditorPath, '.tag-chip')).toContain('min-height:44px')
    expect(declarations(tagEditorPath, '.tag-chip button')).toMatch(/width:44px;height:44px/)
    expect(declarations(tagEditorPath, '.tag-editor > input')).toContain('min-height:44px')
  })
})
