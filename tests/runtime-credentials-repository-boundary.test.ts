import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/infrastructure/runtime.rs')
const repositoryPath = resolve(
  'src-tauri/src/infrastructure/runtime_credentials.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('runtime credentials repository boundary', () => {
  it('keeps stable runtime imports behind one private repository', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "runtime_credentials\.rs"\]\r?\nmod credentials;/,
    )
    expect(facade).toContain(
      'pub use credentials::{KeyringSecretStore, SecretStore};',
    )
    expect(facade).toContain(
      'pub(crate) use credentials::RestoreCredentials;',
    )
    expect(existsSync(repositoryPath)).toBe(true)
    if (!existsSync(repositoryPath)) return

    const repository = readSource(repositoryPath)
    expect(repository).toContain('pub trait SecretStore')
    expect(repository).toContain('pub struct KeyringSecretStore')
    expect(repository).toContain('pub(crate) struct RestoreCredentials')
    expect(repository.match(/pub\(super\) fn /g)).toHaveLength(4)
  })

  it('isolates credential policy while retaining runtime orchestration', () => {
    const facade = readSource(facadePath)
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const token of [
      'const DATABASE_KEY',
      'const ASSET_KEY',
      'const ACCOUNT_ID',
      'const DEVICE_ID',
      'pub const LIBRARY_LOCK_STATE',
      'fn load_required_secret(',
      'fn random_key_hex(',
      'fn decode_key(',
      'getrandom::fill',
      'keyring::Entry',
    ]) {
      expect(repository).toContain(token)
      expect(facade).not.toContain(token)
    }

    for (const token of [
      'pub struct LibraryRuntime',
      'pub struct ActiveProfile',
      'pub enum RuntimeError',
      'pub fn initialize_local_library(',
      'open_encrypted_database',
      'run_migrations',
      'create_profile(',
      'persist_active_profile(',
      'profile_transition:',
    ]) {
      expect(facade).toContain(token)
      expect(repository).not.toContain(token)
    }

    expect(facade).toContain(
      'credentials::load_or_create_local_credentials(secrets, existing_library)?',
    )
    expect(repository).toContain('struct LocalCredentials')
  })

  it('locks fail-closed credential and redaction invariants', () => {
    const facade = readSource(facadePath)
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const token of [
      'if existing_library',
      'RuntimeError::MissingCredentials',
      'RuntimeError::InvalidDatabaseKey',
      'RuntimeError::InvalidAssetKey',
      'RuntimeError::InvalidAccountId',
      'RuntimeError::InvalidDeviceId',
      'RuntimeError::InvalidLibraryLockState',
      'Uuid::now_v7().to_string()',
      'String::with_capacity(64)',
      '[0_u8; 32]',
    ]) {
      expect(repository).toContain(token)
    }
    for (const marker of [
      '.field("asset_key", &"<redacted>")',
      '.field("database_key", &"<redacted>")',
      '.field("account_id", &"<redacted>")',
      '.field("device_id", &"<redacted>")',
      '.field("active_profile", &"<redacted>")',
    ]) {
      expect(facade).toContain(marker)
    }
  })
})
