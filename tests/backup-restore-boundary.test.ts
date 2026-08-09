import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/backup.rs')
const lifecyclePath = resolve('src-tauri/src/modules/backup_restore.rs')
const repositoryPath = resolve(
  'src-tauri/src/modules/backup_restore_repository.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

const restoreEntries = [
  'prepare_backup_restore',
  'validate_restore_candidate',
  'schedule_backup_restore',
  'begin_pending_restore',
  'record_failed_restore',
  'take_restore_receipt',
] as const

describe('backup restore lifecycle boundary', () => {
  it('keeps stable restore APIs on the facade through one private child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "backup_restore\.rs"\]\r?\nmod backup_restore;/,
    )
    expect(facade).toContain('pub use backup_restore::{')
    expect(existsSync(lifecyclePath)).toBe(true)

    for (const name of [...restoreEntries, 'RestoreSwap']) {
      expect(facade).toContain(name)
    }
  })

  it('moves every restore transition and verified candidate copy out of the facade', () => {
    expect(existsSync(lifecyclePath)).toBe(true)
    if (!existsSync(lifecyclePath)) return

    const facade = readSource(facadePath)
    const lifecycle = readSource(lifecyclePath)

    for (const token of [
      'pub struct RestoreSwap',
      'pub fn prepare_backup_restore(',
      'pub fn validate_restore_candidate(',
      'pub fn schedule_backup_restore(',
      'pub fn begin_pending_restore(',
      'fn rollback_interrupted_restore(',
      'impl RestoreSwap',
      'pub fn record_failed_restore(',
      'pub fn take_restore_receipt(',
      'fn copy_verified_manifest_entry(',
    ]) {
      expect(lifecycle).toContain(token)
      expect(facade).not.toContain(token)
    }
  })

  it('keeps persistence in the repository and unrelated backup work outside restore', () => {
    expect(existsSync(lifecyclePath)).toBe(true)
    if (!existsSync(lifecyclePath)) return

    const lifecycle = readSource(lifecyclePath)
    const repository = readSource(repositoryPath)

    for (const token of [
      'read_pending_marker',
      'write_control_file',
      'ensure_owned_directory_if_present',
      'restore_directory_name',
    ]) {
      expect(repository).toContain(`fn ${token}`)
      expect(lifecycle).not.toMatch(
        new RegExp(`(?:pub(?:\\(super\\))? )?fn ${token}\\(`),
      )
    }

    expect(lifecycle).not.toContain('pub fn create_backup(')
    expect(lifecycle).not.toContain('pub fn validate_backup(')
    expect(lifecycle).not.toContain('open_encrypted_database_read_only')
  })
})
