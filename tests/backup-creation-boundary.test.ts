import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/backup.rs')
const creationPath = resolve('src-tauri/src/modules/backup_creation.rs')
const restorePath = resolve('src-tauri/src/modules/backup_restore.rs')

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('backup creation boundary', () => {
  it('keeps the public API in the backup facade and creation in a private child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "backup_creation\.rs"\]\r?\nmod backup_creation;/,
    )
    expect(facade).toContain('pub use backup_creation::create_backup;')
    expect(facade).not.toMatch(/\bpub fn create_backup\(/)
    expect(facade).not.toContain('Backup::new')
    expect(facade).not.toContain('open_encrypted_database(&database_path')
    expect(facade).not.toContain(
      'if result.is_err() && temporary.parent() == Some(destination.as_path())',
    )

    expect(existsSync(creationPath)).toBe(true)
    if (!existsSync(creationPath)) return

    const creation = readSource(creationPath)
    expect(creation).toContain('pub fn create_backup(')
    expect(creation).toContain('Backup::new')
    expect(creation).toContain('open_encrypted_database(&database_path')
  })

  it('gives each unpublished backup one guarded linear filesystem owner', () => {
    expect(existsSync(creationPath)).toBe(true)
    if (!existsSync(creationPath)) return

    const creation = readSource(creationPath)
    const production = creation.split('#[cfg(test)]')[0]

    for (const token of [
      'struct StagedBackupPackage',
      'temporary_path: PathBuf',
      'final_path: PathBuf',
      'published: bool',
      'fn create(destination: &Path, label: &str)',
      'fn path(&self) -> &Path',
      'fn publish(&mut self)',
      'match fs::symlink_metadata(&self.final_path)',
      'error.kind() == io::ErrorKind::NotFound',
      'fs::rename(&self.temporary_path, &self.final_path)?',
      'self.published = true',
      'impl Drop for StagedBackupPackage',
      'if !self.published',
      'fs::remove_dir_all(&self.temporary_path)',
    ]) {
      expect(production).toContain(token)
    }

    expect(production).not.toMatch(
      /#\[derive\([^\]]*Clone[^\]]*\)\]\s*struct StagedBackupPackage/,
    )
    expect(production).not.toContain('fs::remove_dir_all(&self.final_path)')

    for (const testName of [
      'dropping_unpublished_backup_removes_only_its_temporary_directory',
      'publishing_rejects_and_preserves_a_preexisting_completed_package',
      'publishing_keeps_the_completed_backup_package',
    ]) {
      expect(creation).toContain(`fn ${testName}()`)
    }
  })

  it('keeps validation and restore transitions out of the creation child', () => {
    const facade = readSource(facadePath)
    expect(existsSync(creationPath)).toBe(true)
    expect(existsSync(restorePath)).toBe(true)
    if (!existsSync(creationPath) || !existsSync(restorePath)) return

    const creation = readSource(creationPath)
    const restore = readSource(restorePath)

    expect(facade).toContain('pub use backup_validation::validate_backup;')
    expect(creation).not.toContain('pub fn validate_backup(')
    expect(facade).toContain('pub use backup_restore::{')

    for (const name of [
      'prepare_backup_restore',
      'validate_restore_candidate',
      'schedule_backup_restore',
      'begin_pending_restore',
      'record_failed_restore',
      'take_restore_receipt',
    ]) {
      expect(facade).toContain(name)
      expect(facade).not.toContain(`pub fn ${name}(`)
      expect(restore).toContain(`pub fn ${name}(`)
      expect(creation).not.toContain(`pub fn ${name}(`)
    }

    for (const token of [
      'manifest_file_for_existing(',
      'ensure_no_reparse_components(',
      'copy_and_hash(',
      'ensure_database_budget(',
      'ensure_single_account(',
      'package.publish()?;',
    ]) {
      expect(creation).toContain(token)
    }
  })
})
