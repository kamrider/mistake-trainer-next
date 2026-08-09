import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(path: string) {
  return readFileSync(resolve(path), 'utf8')
}

const scanPath = 'src-tauri/src/modules/legacy_scan.rs'
const filesystemPath = 'src-tauri/src/modules/legacy_scan_filesystem.rs'

describe('legacy scan filesystem ownership boundary', () => {
  it('isolates untrusted tree IO from legacy parsing and reporting', () => {
    const scan = source(scanPath)
    expect(scan).toContain('#[path = "legacy_scan_filesystem.rs"]')
    expect(scan).toContain('pub use legacy_scan_filesystem::legacy_tree_fingerprint;')
    expect(scan).toContain('pub(super) use legacy_scan_filesystem::{')
    expect(scan).not.toContain('pub fn legacy_tree_fingerprint(')
    expect(scan).not.toContain('fn collect_fingerprint_files(')
    expect(scan).not.toContain('pub(super) fn read_bounded(')
    expect(scan).not.toContain('pub(super) fn is_safe_relative_path(')

    const filesystem = source(filesystemPath)
    for (const definition of [
      'pub fn legacy_tree_fingerprint(',
      'fn collect_fingerprint_files(',
      'pub(in crate::modules::legacy) fn read_bounded(',
      'pub(in crate::modules::legacy) fn is_safe_relative_path(',
      'pub(super) fn sha256_file(',
      'fn is_windows_reparse_point(',
    ]) {
      expect(filesystem).toContain(definition)
    }
    expect(filesystem).not.toMatch(/serde|serde_json|LegacyScanReport|LegacyIssue/)
    expect(filesystem).not.toMatch(/OffsetDateTime|rusqlite|INSERT |UPDATE |DELETE /)
  })
})
