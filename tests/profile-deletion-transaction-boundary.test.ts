import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/profiles.rs')
const transactionPath = resolve('src-tauri/src/modules/profile_deletion.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('profile deletion transaction boundary', () => {
  it('keeps the public delete API behind one private transaction', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "profile_deletion\.rs"\]\r?\nmod deletion;/,
    )
    expect(facade).toMatch(/pub fn delete_profile\(/)
    expect(facade).toContain('deletion::delete_profile(connection, input)')
    expect(existsSync(transactionPath)).toBe(true)
    if (!existsSync(transactionPath)) return

    const transaction = readSource(transactionPath)
    expect(transaction).toMatch(/pub\(super\) fn delete_profile\(/)
    expect(transaction.match(/pub\(super\) fn /g)).toHaveLength(1)
  })

  it('isolates destructive persistence from ordinary profile CRUD', () => {
    const facade = readSource(facadePath)
    if (!existsSync(transactionPath)) return
    const transaction = readSource(transactionPath)

    for (const token of [
      'const DELETION_RETENTION_MILLIS',
      'let candidate_assets',
      'DELETE FROM sync_conflicts',
      'DELETE FROM tombstones',
      'DELETE FROM learner_profiles',
      "'learner_profile', ?3, 'delete'",
      "'asset', ?3, 'delete'",
      'let asset_operation_time',
    ]) {
      expect(transaction).toContain(token)
      expect(facade).not.toContain(token)
    }

    for (const operation of [
      'create_profile',
      'list_profiles',
      'rename_profile',
      'persist_active_profile',
      'profile_name_exists',
      'learner_profile_from_row',
    ]) {
      expect(facade).toMatch(new RegExp(`fn ${operation}\\(`))
    }
    for (const operation of [
      'create_profile',
      'list_profiles',
      'rename_profile',
      'persist_active_profile',
      'profile_name_exists',
    ]) {
      expect(transaction).not.toMatch(new RegExp(`fn ${operation}\\(`))
    }
  })

  it('locks tenant scope, shared-asset guards, and write ordering', () => {
    if (!existsSync(transactionPath)) return
    const transaction = readSource(transactionPath)

    for (const token of [
      'WHERE account_id = ?1 AND id = ?2',
      'problem.account_id = ?1 AND problem.profile_id = ?2',
      'batch.account_id = ?1 AND batch.profile_id = ?2',
      'NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)',
      'NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)',
      'input.now_utc_ms.saturating_add(DELETION_RETENTION_MILLIS)',
      'target.revision.saturating_add(1)',
      'input.now_utc_ms.saturating_add(1)',
    ]) {
      expect(transaction).toContain(token)
    }

    const preference = transaction.indexOf('INSERT INTO account_preferences')
    const cleanup = transaction.indexOf('DELETE FROM sync_operations')
    const profileDelete = transaction.indexOf('DELETE FROM learner_profiles')
    const profileTombstone = transaction.indexOf("'learner_profile', ?3")
    const orphanLoop = transaction.indexOf('for candidate in candidate_assets')
    const commit = transaction.indexOf('transaction.commit()?')
    expect(preference).toBeLessThan(cleanup)
    expect(cleanup).toBeLessThan(profileDelete)
    expect(profileDelete).toBeLessThan(profileTombstone)
    expect(profileTombstone).toBeLessThan(orphanLoop)
    expect(orphanLoop).toBeLessThan(commit)
  })
})
