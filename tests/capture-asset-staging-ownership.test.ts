import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const repositoryPath = resolve(
  'src-tauri/src/modules/capture_asset_repository.rs',
)
const orchestratorPath = resolve('src-tauri/src/modules/capture_inbox.rs')
const integrationPath = resolve('src-tauri/tests/capture_inbox_store.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('capture asset staging ownership', () => {
  it('keeps one linear staged-asset owner in the filesystem repository', () => {
    const repository = readSource(repositoryPath)
    const production = repository.split('#[cfg(test)]')[0]

    for (const token of [
      'pub(crate) struct StagedCaptureAsset',
      'moved_to_final: bool',
      'committed: bool',
      'pub(crate) fn stage_encrypted_capture_asset(',
      'pub(crate) fn asset_id(&self)',
      'pub(crate) fn relative_path(&self)',
      'pub(crate) fn promote(&mut self)',
      'pub(crate) fn mark_committed(&mut self)',
      'impl Drop for StagedCaptureAsset',
      'if self.moved_to_final && !self.committed',
    ]) {
      expect(production).toContain(token)
    }
    expect(production).not.toMatch(/derive\([^)]*Clone/)
  })

  it('guards promotion and covers every cleanup transition', () => {
    const repository = readSource(repositoryPath)

    for (const token of [
      'if self.final_path.exists()',
      'fs::rename(&self.staged_path, &self.final_path)?',
      'self.moved_to_final = true',
      'self.committed = true',
      'fs::remove_file(&self.staged_path)',
      'fs::remove_file(&self.final_path)',
      'rollback_preserves_a_preexisting_unowned_capture_blob',
      'rollback_removes_a_capture_blob_promoted_by_this_owner',
      'commit_keeps_the_promoted_capture_blob',
      'dropping_before_promotion_removes_the_staged_capture_blob',
    ]) {
      expect(repository).toContain(token)
    }
    expect(repository).not.toContain(
      'if self.final_path.exists() { fs::remove_file',
    )
  })

  it('leaves encryption and SQL orchestration in the inbox use case', () => {
    const repository = readSource(repositoryPath)
    const production = repository.split('#[cfg(test)]')[0]
    const orchestrator = readSource(orchestratorPath)

    for (const forbidden of [
      'rusqlite',
      'Connection',
      'transaction(',
      'async fn',
      '.await',
      'inspect_capture_image',
      'plaintext_sha256',
      'encrypt_asset',
    ]) {
      expect(production).not.toContain(forbidden)
    }
    for (const token of [
      'inspect_capture_image(&input.bytes)?',
      'plaintext_sha256(&input.bytes)',
      'encrypt_asset(&input.bytes, asset_key)',
      'stage_encrypted_capture_asset(',
      'Option<StagedCaptureAsset>',
      '.promote()?',
      'transaction.commit()?;',
      '.mark_committed()',
    ]) {
      expect(orchestrator).toContain(token)
    }
    expect(orchestrator).toMatch(
      /transaction\.commit\(\)\?;\s*(?:if let Some\([^)]*\) = &mut staged_asset \{\s*)?[^}]*\.mark_committed\(\)/s,
    )
    expect(orchestrator).not.toContain(
      'Option<(String, PathBuf, PathBuf, String)>',
    )
    expect(orchestrator).not.toContain(
      'std::fs::rename(staged_path, final_path)',
    )
    expect(orchestrator).not.toContain('std::fs::remove_file(final_path)')

    const integration = readSource(integrationPath)
    expect(integration).toContain(
      'fn transaction_begin_failure_cleans_the_staged_capture_asset()',
    )
  })
})
