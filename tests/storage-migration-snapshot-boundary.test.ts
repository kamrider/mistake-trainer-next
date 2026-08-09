import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/storage_migration.rs')
const snapshotPath = resolve(
  'src-tauri/src/modules/storage_migration_snapshot.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('storage migration snapshot boundary', () => {
  it('keeps migration lifecycle APIs behind a private snapshot child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "storage_migration_snapshot\.rs"\]\r?\nmod snapshot;/,
    )
    for (const name of [
      'stage_storage_migration',
      'stage_storage_migration_from_source',
      'storage_usage_bytes',
      'apply_pending_storage_migration',
      'storage_migration_pending',
      'read_storage_migration_receipt',
      'take_storage_migration_receipt',
    ]) {
      expect(facade).toMatch(new RegExp(`pub fn ${name}\\(`))
    }
    expect(facade).toContain('snapshot::stage_library_snapshot(')
    expect(facade).toContain('snapshot::storage_usage_bytes(')
    expect(facade).toContain('snapshot::validate_library_tree(')
  })

  it('isolates snapshot SQL, asset copying, and integrity verification', () => {
    const facade = readSource(facadePath)
    const snapshotExists = existsSync(snapshotPath)

    expect(snapshotExists).toBe(true)
    if (!snapshotExists) return

    const snapshot = readSource(snapshotPath)
    for (const name of [
      'stage_library_snapshot',
      'storage_usage_bytes',
      'validate_library_tree',
    ]) {
      expect(snapshot).toMatch(new RegExp(`pub\\(super\\) fn ${name}\\(`))
    }
    const exposedOperations = [
      ...snapshot.matchAll(/pub\(super\) fn ([a-z_]+)\(/g),
    ].map((match) => match[1])
    expect(exposedOperations).toEqual([
      'stage_library_snapshot',
      'storage_usage_bytes',
      'validate_library_tree',
    ])
    for (const name of [
      'create_database_snapshot',
      'query_assets',
      'copy_referenced_assets',
      'copy_and_verify',
      'validate_account_boundary',
      'ensure_database_budget',
      'pragma_u64',
    ]) {
      expect(snapshot).toMatch(new RegExp(`(?:^|\\n)fn ${name}\\(`))
      expect(facade).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }

    for (const token of [
      'backup::Backup',
      'decrypt_asset',
      'plaintext_sha256',
      'SELECT COUNT(*) FROM assets',
      'ORDER BY encrypted_path',
      'quick_check',
      'MAX_TOTAL_ASSET_BYTES',
      'ensure_no_reparse_components',
    ]) {
      expect(snapshot).toContain(token)
    }
    for (const lifecycleToken of [
      'write_storage_pointer',
      'remove_control_file',
      'initialize_application_library',
      'write_migration_receipt',
      'remove_committed_source',
    ]) {
      expect(facade).toContain(lifecycleToken)
      expect(snapshot).not.toContain(lifecycleToken)
    }
    expect(facade).not.toContain('backup::Backup')
    expect(facade).not.toContain('decrypt_asset')
    expect(facade).not.toContain('plaintext_sha256')
  })
})
