import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/exports.rs')
const generatorPath = resolve('src-tauri/src/modules/exports_generation.rs')

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('export generation boundary', () => {
  it('keeps the existing exports API behind a private generation child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "exports_generation\.rs"\]\r?\nmod generation;/,
    )
    expect(facade).toContain('pub use generation::generate_export;')
    expect(facade).toContain(
      'pub(crate) struct PreparedExport(generation::PreparedExport);',
    )
    expect(facade).toContain('generation::prepare_export(')
    expect(facade).toContain('generation::write_prepared_export(')
  })

  it('separates snapshot persistence from filesystem and DOCX generation', () => {
    const facade = readSource(facadePath)
    const generatorExists = existsSync(generatorPath)

    expect(generatorExists).toBe(true)
    if (!generatorExists) return

    const generator = readSource(generatorPath)
    const facadeRepositoryFunctions = [
      'create_export_snapshot',
      'list_export_snapshots',
      'list_export_candidates',
      'list_deleted_export_snapshots',
      'delete_export_snapshot',
      'restore_export_snapshot',
    ]
    const generatorOnlyFunctions = [
      'generate_export',
      'load_snapshot',
      'load_export_problems',
      'read_decrypted_export_asset',
      'generate_original_folder',
      'generate_docx',
      'validate_docx_assets',
      'safe_output_stem',
    ]
    const safetyLimits = [
      'MAX_ENCRYPTED_ASSET_BYTES',
      'MAX_EXPORT_PLAINTEXT_BYTES',
      'MAX_EXPORT_ASSETS',
      'MAX_DOCX_IMAGE_PIXELS',
      'MAX_DOCX_IMAGE_DIMENSION',
      'MAX_DOCX_TOTAL_PIXELS',
    ]

    for (const name of facadeRepositoryFunctions) {
      expect(facade).toMatch(new RegExp(`\\bfn ${name}\\(`))
      expect(generator).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }
    for (const name of generatorOnlyFunctions) {
      expect(generator).toMatch(new RegExp(`\\bfn ${name}\\(`))
      expect(facade).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }
    for (const name of ['prepare_export', 'write_prepared_export']) {
      expect(generator).toMatch(new RegExp(`pub\\(super\\) fn ${name}\\(`))
      expect(facade).toMatch(new RegExp(`pub\\(crate\\) fn ${name}\\(`))
    }
    for (const limit of safetyLimits) {
      expect(generator).toContain(`const ${limit}`)
      expect(facade).not.toContain(`const ${limit}`)
    }

    expect(generator).toContain('pub(super) struct PreparedExport')
    expect(generator).toContain('use docx_rs::')
    expect(generator).toContain('AssetDecryptor')
    expect(generator).toContain('.decrypt(&encrypted)')
    expect(generator).not.toContain('infrastructure::assets')
    expect(facade).not.toContain('docx_rs')
    expect(facade).not.toContain('decrypt_asset')
  })
})
