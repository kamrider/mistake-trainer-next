import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/diagnostics.rs')
const builderPath = resolve(
  'src-tauri/src/modules/diagnostic_report_builder.rs',
)
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('diagnostic report builder boundary', () => {
  it('keeps report construction behind one private builder operation', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "diagnostic_report_builder\.rs"\]\r?\nmod report_builder;/,
    )
    expect(facade).toMatch(/pub fn export_diagnostic_report\(/)
    expect(facade).toContain('report_builder::build_report(&connection, &report_id, context)?')
    expect(facade).toContain('report.warning_count()')

    expect(existsSync(builderPath)).toBe(true)
    if (!existsSync(builderPath)) return

    const builder = readSource(builderPath)
    expect(builder).toMatch(/^pub\(super\) fn build_report\(/m)
    expect(builder.match(/^pub\(super\) fn /gm)).toHaveLength(1)
    expect(builder).toContain('pub(super) struct DiagnosticReport')
    expect(builder).toContain('pub(super) fn warning_count(&self) -> u32')
  })

  it('isolates the aggregate schema and fixed warning projection', () => {
    const facade = readSource(facadePath)
    if (!existsSync(builderPath)) return
    const builder = readSource(builderPath)

    for (const token of [
      'const REPORT_SCHEMA_VERSION',
      'const APPLICATION_NAME',
      'struct DiagnosticApplication',
      'struct DiagnosticLibrary',
      'enum DiagnosticIntegrity',
      'struct DiagnosticSync',
      'struct DiagnosticWarning',
      'fn count(',
      'fn non_negative(',
      'PRAGMA quick_check(1)',
      'library_integrity_check_failed',
      'windows_release_unsupported',
      'windows_extended_support_only',
      'webview2_runtime_not_detected',
      'previous_startup_failure_detected',
    ]) {
      expect(builder).toContain(token)
      expect(facade).not.toContain(token)
    }

    for (const table of [
      'learner_profiles',
      'problems',
      'assets',
      'capture_batches',
      'review_events',
      'export_snapshots',
      'sync_operations',
      'sync_conflicts',
    ]) {
      expect(builder).toContain(`"${table}"`)
    }
  })

  it('keeps user content out and atomic file delivery in the facade', () => {
    const facade = readSource(facadePath)
    if (!existsSync(builderPath)) return
    const builder = readSource(builderPath)

    for (const forbidden of [
      'SELECT *',
      'account_id',
      'profile_id',
      'device_id',
      'subject',
      'note',
      'tags_json',
      'encrypted_path',
      'source_name',
      'payload_json',
      'local_value_json',
      'remote_value_json',
    ]) {
      expect(builder).not.toContain(forbidden)
    }
    for (const token of [
      'fn ensure_destination_directory(',
      'fn write_report_atomically(',
      'OpenOptions::new()',
      '.create_new(true)',
      'output.sync_all()',
      'fs::rename(&temporary_path, &final_path)',
      'fs::remove_file(&temporary_path)',
      'an_existing_final_report_is_never_overwritten_and_temp_is_removed',
    ]) {
      expect(facade).toContain(token)
      expect(builder).not.toContain(token)
    }
  })
})
