import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/backup.rs')
const validationPath = resolve('src-tauri/src/modules/backup_validation.rs')
const restorePath = resolve('src-tauri/src/modules/backup_restore.rs')

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('backup package validation boundary', () => {
  it('keeps the validation API on the facade through a private child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "backup_validation\.rs"\]\r?\nmod backup_validation;/,
    )
    expect(facade).toContain('pub use backup_validation::validate_backup;')
    expect(facade).not.toMatch(/\bpub fn validate_backup\(/)
    expect(facade).not.toContain('open_encrypted_database_read_only')
    expect(facade).not.toContain('decrypt_asset')
    expect(facade).not.toContain('plaintext_sha256')

    expect(existsSync(validationPath)).toBe(true)
    if (!existsSync(validationPath)) return
    expect(readSource(validationPath)).toContain('pub fn validate_backup(')
  })

  it('owns every private validation workspace through one RAII object', () => {
    expect(existsSync(validationPath)).toBe(true)
    if (!existsSync(validationPath)) return

    const validation = readSource(validationPath)
    const production = validation.split('#[cfg(test)]')[0]

    for (const token of [
      'struct ValidationWorkspace',
      'parent: PathBuf',
      'path: PathBuf',
      'fn create(parent: PathBuf)',
      'fn path(&self) -> &Path',
      'impl Drop for ValidationWorkspace',
      'if self.path.parent() == Some(self.parent.as_path())',
      'fs::remove_dir_all(&self.path)',
      'let workspace = ValidationWorkspace::create(validation_parent)?;',
      'workspace.path().join(DATABASE_FILE)',
    ]) {
      expect(production).toContain(token)
    }

    expect(production).not.toMatch(
      /#\[derive\([^\]]*Clone[^\]]*\)\]\s*struct ValidationWorkspace/,
    )
    expect(production).not.toContain('fs::remove_dir_all(&validation_directory)')

    for (const testName of [
      'dropping_validation_workspace_removes_only_its_owned_directory',
      'validation_workspaces_use_distinct_private_names',
    ]) {
      expect(validation).toContain(`fn ${testName}()`)
    }
  })

  it('contains package verification without absorbing creation or restore', () => {
    const facade = readSource(facadePath)
    expect(existsSync(validationPath)).toBe(true)
    expect(existsSync(restorePath)).toBe(true)
    if (!existsSync(validationPath) || !existsSync(restorePath)) return

    const validation = readSource(validationPath)
    const restore = readSource(restorePath)

    for (const token of [
      'reject_sqlite_sidecars(&source)?',
      'copy_and_hash(&source_database, &staged_database, MAX_DATABASE_BYTES)',
      'open_encrypted_database_read_only(&staged_database, database_key)',
      'ensure_single_account(&database, account_id, schema_version)?',
      'read_verified_manifest_file(&source, asset, MAX_ASSET_BYTES)?',
      'decrypt_asset(&encrypted, asset_key)',
      'plaintext_sha256(&plaintext)',
    ]) {
      expect(validation).toContain(token)
      expect(facade).not.toContain(token)
    }

    for (const forbidden of [
      'pub fn create_backup(',
      'pub fn prepare_backup_restore(',
      'pub fn validate_restore_candidate(',
      'pub fn schedule_backup_restore(',
      'pub fn begin_pending_restore(',
      'pub fn record_failed_restore(',
      'pub fn take_restore_receipt(',
    ]) {
      expect(validation).not.toContain(forbidden)
    }

    expect(facade).toContain('pub use backup_restore::{')
    for (const restoreEntry of [
      'prepare_backup_restore',
      'validate_restore_candidate',
      'schedule_backup_restore',
      'begin_pending_restore',
      'record_failed_restore',
      'take_restore_receipt',
    ]) {
      expect(facade).toContain(restoreEntry)
      expect(facade).not.toContain(`pub fn ${restoreEntry}(`)
      expect(restore).toContain(`pub fn ${restoreEntry}(`)
    }
  })
})
