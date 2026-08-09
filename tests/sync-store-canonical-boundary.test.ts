import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/sync_store.rs')
const repositoryPath = resolve('src-tauri/src/modules/sync_store_canonical.rs')

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('sync store canonical repository boundary', () => {
  it('delegates canonical push reconstruction to a private child repository', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "sync_store_canonical\.rs"\]\r?\nmod canonical;/,
    )
    expect(facade).toContain('canonical::select_due_operations')
    expect(facade).toContain('canonical::canonical_entity')
  })

  it('keeps entity loaders private and outside the orchestration facade', () => {
    const facade = readSource(facadePath)
    const repositoryExists = existsSync(repositoryPath)

    expect(repositoryExists).toBe(true)
    if (!repositoryExists) return

    const repository = readSource(repositoryPath)
    const loaders = [
      'load_profile',
      'load_asset',
      'load_problem',
      'load_review_event',
      'load_export_snapshot',
      'load_tombstone',
    ]

    for (const loader of loaders) {
      expect(facade).not.toMatch(new RegExp(`\\bfn ${loader}\\(`))
      expect(repository).toMatch(new RegExp(`(?:^|\\n)fn ${loader}\\(`))
      expect(repository).not.toMatch(new RegExp(`pub(?:\\([^)]*\\))? fn ${loader}\\(`))
    }

    expect(repository).toMatch(/pub\(super\) struct OperationRow/)
    expect(repository).toMatch(/pub\(super\) fn select_due_operations\(/)
    expect(repository).toMatch(/pub\(super\) fn canonical_entity\(/)
  })
})
