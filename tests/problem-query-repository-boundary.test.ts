import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/problems.rs')
const repositoryPath = resolve(
  'src-tauri/src/modules/problem_query_repository.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('problem query repository boundary', () => {
  it('keeps the public problem query API behind a private child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "problem_query_repository\.rs"\]\r?\nmod query_repository;/,
    )
    for (const name of [
      'list_problem_summaries',
      'list_problem_summaries_with_previews',
      'get_problem_detail',
    ]) {
      expect(facade).toMatch(new RegExp(`pub fn ${name}\\(`))
      expect(facade).toContain(`query_repository::${name}(`)
    }
  })

  it('isolates query SQL and preview reconstruction from mutations', () => {
    const facade = readSource(facadePath)
    const repositoryExists = existsSync(repositoryPath)

    expect(repositoryExists).toBe(true)
    if (!repositoryExists) return

    const repository = readSource(repositoryPath)
    const queryEntries = [
      'list_problem_summaries',
      'list_problem_summaries_with_previews',
      'get_problem_detail',
    ]
    const queryHelpers = [
      'list_problem_summaries_internal',
      'read_decrypted_asset',
      'make_preview',
      'make_preview_with_dimension',
      'media_type_for',
    ]
    const queryLimits = [
      'MAX_ENCRYPTED_ASSET_BYTES',
      'PREVIEW_MAX_DIMENSION',
      'LIST_PREVIEW_MAX_DIMENSION',
      'MAX_SOURCE_DIMENSION',
      'MAX_SOURCE_PIXELS',
    ]
    const mutations = [
      'create_problem',
      'update_problem',
      'change_problem_status',
      'persist_problem',
      'cleanup_new_assets',
    ]

    for (const name of queryEntries) {
      expect(repository).toMatch(new RegExp(`pub\\(super\\) fn ${name}\\(`))
    }
    for (const name of queryHelpers) {
      expect(repository).toMatch(new RegExp(`(?:^|\\n)(?:const )?fn ${name}\\(`))
      expect(facade).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }
    for (const limit of queryLimits) {
      expect(repository).toContain(`const ${limit}`)
      expect(facade).not.toContain(`const ${limit}`)
    }
    for (const name of mutations) {
      expect(facade).toMatch(new RegExp(`\\bfn ${name}\\(`))
      expect(repository).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }

    expect(repository).toContain('use base64::')
    expect(repository).toContain('AssetDecryptor')
    expect(repository).toContain('.decrypt(&encrypted)')
    expect(repository).not.toContain('infrastructure::assets')
    expect(repository).toContain(".replace('%', \"\\\\%\")")
    expect(repository).toContain('FROM problems p')
    expect(repository).toContain('FROM problem_assets pa')
    expect(repository).toContain('data:{preview_media_type};base64')
    expect(facade).not.toContain('base64::')
    expect(facade).not.toContain('decrypt_asset')
    for (const queryDependency of ['Cursor', 'Read', 'Component']) {
      expect(repository).toContain(queryDependency)
      expect(facade).not.toContain(queryDependency)
    }
  })
})
