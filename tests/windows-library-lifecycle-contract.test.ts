import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const source = (path: string) => readFileSync(resolve(path), 'utf8')

describe('Windows library lifecycle contract', () => {
  it('keeps first run behind an exhaustive evidence inventory', () => {
    const inventory = source('src-tauri/src/application/library_inventory.rs')
    expect(inventory).toContain('CredentialEnvelopeState::Absent, LibraryArtifactState::Absent, false')
    expect(inventory).toContain('LibraryRecoveryReason::LocalDataMissing')
    expect(inventory).toContain('LibraryRecoveryReason::SetupInterrupted')
    expect(inventory).toContain('LibraryRecoveryReason::ResetIncomplete')
    expect(inventory).toContain('LibraryRecoveryReason::MigrationInterrupted')
    expect(inventory).toContain('LibraryRecoveryReason::RestoreInterrupted')
  })

  it('requires journaled exact-confirmation abandonment', () => {
    const access = source('src-tauri/src/commands/access.rs')
    const reset = source('src-tauri/src/modules/library_reset.rs')
    expect(access).toContain('永久放弃原资料库')
    expect(access).toContain('fresh_start_preflight')
    expect(access).toContain('validate_existing_library')
    expect(reset).toContain('RESET_PENDING_FILE')
    expect(reset).toContain('delete_local_credential_envelope')
    expect(reset).toContain('remove_control_file(control_root, RESET_PENDING_FILE)')
  })

  it('never converts unreadable operation evidence into absence', () => {
    const startup = source('src-tauri/src/application/startup.rs')
    expect(startup).not.toContain('control_file_present(application_root, RESTORE_PENDING_FILE).unwrap_or(false)')
    expect(startup).not.toContain('marker_path.exists()')
    expect(startup).toContain('LibraryRecoveryReason::RestoreInterrupted')
  })

  it('never treats a status retry as an in-process startup recheck', () => {
    const lifecycle = source('src/app/composables/useLibraryAccessLifecycle.ts')
    expect(lifecycle).toContain('options.retry()')
    expect(lifecycle).toContain("phase.value = 'restarting'")
  })
})
