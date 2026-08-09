import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/legacy.rs')
const importPath = resolve(
  'src-tauri/src/modules/legacy_import_transaction.rs',
)
const rollbackPath = resolve(
  'src-tauri/src/modules/legacy_rollback_transaction.rs',
)
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('legacy mutation boundaries', () => {
  it('keeps stable public functions behind two private children', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "legacy_import_transaction\.rs"\]\r?\nmod legacy_import_transaction;/,
    )
    expect(facade).toMatch(
      /#\[path = "legacy_rollback_transaction\.rs"\]\r?\nmod legacy_rollback_transaction;/,
    )
    expect(facade).toContain(
      'pub use legacy_import_transaction::import_legacy_plan;',
    )
    expect(facade).toContain(
      'pub use legacy_rollback_transaction::rollback_legacy_import;',
    )
    expect(facade).not.toMatch(/^pub fn import_legacy_plan\(/m)
    expect(facade).not.toMatch(/^pub fn rollback_legacy_import\(/m)
    expect(existsSync(importPath)).toBe(true)
    expect(existsSync(rollbackPath)).toBe(true)
  })

  it('gives import validation, staging, persistence, and progress one owner', () => {
    expect(existsSync(importPath)).toBe(true)
    if (!existsSync(importPath)) return
    const source = readSource(importPath)

    for (const token of [
      'pub fn import_legacy_plan(',
      'struct StagedLegacyAsset',
      'fn persist_legacy_import(',
      'fn insert_import_sync_operation(',
      'fn record_import_entity(',
      'fn unique_profile_name(',
      'fn validate_import_image(',
      'fn plaintext_digest(',
      'fn cleanup_legacy_staging(',
      'fs::rename(&asset.staged_path, &asset.final_path)?',
      'transaction.commit()?',
      'LegacyImportPhase::Validating',
      'LegacyImportPhase::Encrypting',
      'LegacyImportPhase::Writing',
      'LegacyImportPhase::Verifying',
      'LegacyImportPhase::Completed',
    ]) {
      expect(source).toContain(token)
    }
    expect(
      source.match(/legacy_tree_fingerprint\(&plan\.source_root\)/g),
    ).toHaveLength(1)
    expect(source).toContain('build_legacy_import_plan(&plan.source_root)?')
    expect(source).not.toContain('.legacy-rollback')
    expect(source).not.toContain('enqueue_legacy_rollback_deletion')
    expect(source).not.toContain("'delete'")
  })

  it('gives guarded rollback, quarantine, and deletion sync one owner', () => {
    expect(existsSync(rollbackPath)).toBe(true)
    if (!existsSync(rollbackPath)) return
    const source = readSource(rollbackPath)

    for (const token of [
      'pub fn rollback_legacy_import(',
      'struct RemovedLegacyEntity',
      'fn enqueue_legacy_rollback_deletion(',
      'fn import_entity_ids(',
      'fn restore_quarantined_assets(',
      'revision != 1',
      'has_non_import_review',
      'has_snapshot_reference',
      '.legacy-rollback',
      'restore_quarantined_assets(&quarantined)',
      'INSERT INTO tombstones(',
      "'delete'",
      'transaction.commit()?',
    ]) {
      expect(source).toContain(token)
    }
    expect(source.indexOf('fs::rename(original, &staged)?')).toBeLessThan(
      source.indexOf('transaction.commit()?'),
    )
    expect(source).not.toContain('encrypt_asset')
    expect(source).not.toContain('StagedLegacyAsset')
    expect(source).not.toContain('LegacyImportPhase')
  })

  it('leaves public models, candidate state, history, and errors in the facade', () => {
    const facade = readSource(facadePath)

    for (const token of [
      'pub struct LegacyImportPlan',
      'pub enum LegacyImportPhase',
      'pub struct LegacyImportReceipt',
      'pub struct LegacyRollbackReceipt',
      'pub struct LegacyImportCandidate',
      'pub struct LegacyImportSummary',
      'pub struct LegacyImportManager',
      'pub fn list_legacy_imports(',
      'pub enum LegacyImportError',
      'pub use legacy_scan::{',
    ]) {
      expect(facade).toContain(token)
    }
    for (const forbidden of [
      'fs::rename(',
      'fs::write(',
      'transaction.commit()?',
      'struct StagedLegacyAsset',
      'struct RemovedLegacyEntity',
      'INSERT INTO ',
    ]) {
      expect(facade).not.toContain(forbidden)
    }
    expect(facade).not.toMatch(/\bTransaction\b/)
  })
})
