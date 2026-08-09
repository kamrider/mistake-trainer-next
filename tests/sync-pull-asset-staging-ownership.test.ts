import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/sync_pull.rs')
const stagingPath = resolve(
  'src-tauri/src/modules/sync_pull_asset_staging.rs',
)
const transactionPath = resolve(
  'src-tauri/src/modules/sync_pull_transaction.rs',
)
const integrationPath = resolve('src-tauri/tests/sync_pull.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('sync pull asset staging ownership', () => {
  it('moves local file lifecycle behind one private staging boundary', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "sync_pull_asset_staging\.rs"\]\r?\nmod asset_staging;/,
    )
    expect(facade).toMatch(
      /use asset_staging::\{[^}]*StagedAsset[^}]*cleanup_page[^}]*stage_encrypted_asset[^}]*\};/s,
    )
    expect(facade).not.toContain('struct StagedAsset')
    expect(facade).not.toContain('fn cleanup_staged(')
    expect(facade).not.toContain('fs::rename(')
    expect(facade).not.toContain('fs::write(')

    expect(existsSync(stagingPath)).toBe(true)
    if (!existsSync(stagingPath)) return

    const staging = readSource(stagingPath)
    const production = staging.split('#[cfg(test)]')[0]
    expect(production).toContain('pub(super) struct StagedAsset')
    expect(production).toContain('moved_to_final: bool')
    expect(production).toContain('moved_to_final: false')
    expect(production).toContain('pub(super) fn stage_encrypted_asset(')
    expect(production).toContain('pub(super) fn promote(&mut self)')
    expect(production).toContain('pub(super) fn cleanup_page(')
    expect(production).toContain('fs::remove_file(&staged_path)')
    expect(production).toContain('fs::remove_dir(&staged_root)')
  })

  it('tracks promotion ownership and deletes only files owned by this page', () => {
    if (!existsSync(stagingPath)) return
    const staging = readSource(stagingPath)

    for (const token of [
      'if self.final_path.exists()',
      'return Err(SyncPullError::AssetMismatch)',
      'fs::rename(&self.staged_path, &self.final_path)?',
      'self.moved_to_final = true',
      'rollback_final && asset.moved_to_final',
      'rollback_preserves_a_preexisting_unowned_final_file',
      'rollback_removes_a_final_file_promoted_by_this_page',
      'success_cleanup_keeps_the_promoted_final_file',
    ]) {
      expect(staging).toContain(token)
    }
    expect(staging).not.toContain(
      'rollback_final && asset.final_path.exists()',
    )
  })

  it('keeps transport and encryption in the facade and promotion in the transaction', () => {
    const facade = readSource(facadePath)
    if (!existsSync(stagingPath) || !existsSync(transactionPath)) return
    const staging = readSource(stagingPath)
    const production = staging.split('#[cfg(test)]')[0]
    const transaction = readSource(transactionPath)

    for (const forbidden of [
      'async fn',
      '.await',
      'CloudPullTransport',
      'download_object',
      'validate_download',
      'encrypt_asset',
      'Connection',
      'Transaction',
      'record_pull_success_tx',
      'rebuild_schedule_for_problem',
    ]) {
      expect(production).not.toContain(forbidden)
    }
    for (const token of [
      'pub async fn pull_until_current<T: CloudPullTransport>',
      '.download_object(access_token, &asset.storage_object)',
      'validate_download(asset, &downloaded)?',
      'encrypt_asset(&downloaded.bytes, asset_key)',
      'stage_encrypted_asset(',
      'cleanup_page(&staged_assets, false)',
      'cleanup_page(&staged_assets, true)',
    ]) {
      expect(facade).toContain(token)
    }
    for (const token of [
      'staged_assets: &mut [StagedAsset]',
      'staged: Option<&mut StagedAsset>',
      'staged.promote()?',
      'transaction.commit()?',
    ]) {
      expect(transaction).toContain(token)
      expect(facade).not.toContain(token)
    }
    expect(facade).toContain('match stage_remote_asset(')
    expect(
      facade.match(/cleanup_page\(&staged_assets, true\)/g),
    ).toHaveLength(2)

    const integration = readSource(integrationPath)
    expect(integration).toContain(
      'fn failed_pull_does_not_delete_a_preexisting_unowned_blob()',
    )
    expect(integration).toContain(
      'fn later_download_failure_cleans_assets_staged_earlier_in_the_page()',
    )
  })
})
