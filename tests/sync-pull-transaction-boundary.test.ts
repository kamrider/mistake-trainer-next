import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/sync_pull.rs')
const transactionPath = resolve(
  'src-tauri/src/modules/sync_pull_transaction.rs',
)
const stagingPath = resolve(
  'src-tauri/src/modules/sync_pull_asset_staging.rs',
)
const decoderPath = resolve('src-tauri/src/modules/sync_pull_decoder.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('sync pull page transaction boundary', () => {
  it('moves page application behind one private child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "sync_pull_transaction\.rs"\]\r?\nmod sync_pull_transaction;/,
    )
    expect(facade).toContain('use sync_pull_transaction::apply_page;')
    expect(facade).not.toMatch(/^fn apply_page\(/m)
    expect(existsSync(transactionPath)).toBe(true)

    if (!existsSync(transactionPath)) return
    expect(readSource(transactionPath)).toMatch(
      /^pub\(super\) fn apply_page\(/m,
    )
  })

  it('keeps the complete SQLite page mutation atomic in the child', () => {
    const facade = readSource(facadePath)
    if (!existsSync(transactionPath)) return
    const transaction = readSource(transactionPath)

    for (const token of [
      'let transaction = connection.transaction()?',
      'staged_assets: &mut [StagedAsset]',
      'staged: Option<&mut StagedAsset>',
      'staged.promote()?',
      'fn apply_profile_merge(',
      'fn apply_problem_merge(',
      'fn apply_export_merge(',
      'fn apply_tombstone_merge(',
      'fn upsert_asset(',
      'rebuild_schedule_for_problem(',
      'record_pull_success_tx(',
      'transaction.commit()?',
      'remove_orphan_blob(blob_root, &relative_path)',
    ]) {
      expect(transaction).toContain(token)
    }

    for (const functionName of [
      'apply_page',
      'apply_profile_merge',
      'apply_problem_merge',
      'apply_export_merge',
      'apply_tombstone_merge',
      'upsert_asset',
    ]) {
      expect(facade).not.toMatch(
        new RegExp(`^fn ${functionName}\\(`, 'm'),
      )
    }

    expect(transaction.indexOf('record_pull_success_tx(')).toBeLessThan(
      transaction.indexOf('transaction.commit()?'),
    )
    expect(transaction.indexOf('transaction.commit()?')).toBeLessThan(
      transaction.indexOf('remove_orphan_blob(blob_root, &relative_path)'),
    )
  })

  it('leaves remote I/O in the facade and decoding and staging isolated', () => {
    const facade = readSource(facadePath)
    const staging = readSource(stagingPath).split('#[cfg(test)]')[0]
    const decoder = readSource(decoderPath)
    if (!existsSync(transactionPath)) return
    const transaction = readSource(transactionPath)

    for (const token of [
      'pub async fn pull_until_current<T: CloudPullTransport>',
      '.download_object(access_token, &asset.storage_object)',
      'validate_download(asset, &downloaded)?',
      'encrypt_asset(&downloaded.bytes, asset_key)',
      'stage_encrypted_asset(',
      'cleanup_page(&staged_assets, false)',
      'cleanup_page(&staged_assets, true)',
      'fn local_asset_matches(',
    ]) {
      expect(facade).toContain(token)
    }

    for (const forbidden of [
      'async fn',
      '.await',
      'CloudPullTransport',
      'download_object',
      'validate_download',
      'encrypt_asset',
      'cleanup_page',
    ]) {
      expect(transaction).not.toContain(forbidden)
    }

    expect(transaction).toContain('asset_staging::StagedAsset')
    expect(transaction).toContain('sync_pull_decoder::DecodedChange')
    expect(staging).not.toContain('Transaction')
    expect(staging).not.toContain('record_pull_success_tx')
    expect(decoder).not.toContain('Transaction')
    expect(decoder).not.toContain('CloudPullTransport')
  })
})
