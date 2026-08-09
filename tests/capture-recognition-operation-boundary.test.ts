import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(path: string) {
  return readFileSync(resolve(path), 'utf8')
}

const facadePath = 'src-tauri/src/modules/capture_recognition.rs'
const applyPath = 'src-tauri/src/modules/capture_recognition_transaction.rs'
const revertPath = 'src-tauri/src/modules/capture_recognition_revert.rs'
const ledgerPath = 'src-tauri/src/modules/capture_recognition_operation_ledger.rs'

describe('capture recognition operation ownership boundary', () => {
  it('keeps apply, revert/query, and serialization ownership separate', () => {
    const facade = source(facadePath)
    expect(facade).toContain('#[path = "capture_recognition_operation_ledger.rs"]')
    expect(facade).toContain('#[path = "capture_recognition_revert.rs"]')
    expect(facade).toContain('pub use capture_recognition_transaction::apply_capture_recognition;')
    expect(facade).toContain('pub use capture_recognition_revert::{')

    const apply = source(applyPath)
    expect(apply).toContain('pub fn apply_capture_recognition(')
    expect(apply).not.toContain('pub fn revert_capture_recognition(')
    expect(apply).not.toContain('pub fn latest_capture_recognition_operation(')

    const revert = source(revertPath)
    expect(revert).toContain('pub fn revert_capture_recognition(')
    expect(revert).toContain('pub fn latest_capture_recognition_operation(')
    expect(revert).not.toContain('pub fn apply_capture_recognition(')

    const ledger = source(ledgerPath)
    for (const typeName of [
      'RecognitionOperationLedger',
      'RecognitionLedgerSource',
      'RecognitionLedgerItem',
      'RecognitionLedgerDraft',
    ]) {
      expect(ledger).toMatch(
        new RegExp(
          `#\\[serde\\(rename_all = "camelCase"\\)\\]\\s+pub\\(super\\) struct ${typeName}`,
        ),
      )
    }
    for (const field of [
      'source_items: Vec<RecognitionLedgerSource>',
      'created_items: Vec<RecognitionLedgerItem>',
      'created_drafts: Vec<RecognitionLedgerDraft>',
      'superseded_by_derivation_id: String',
      'derivation_id: String',
      'source_sequence: i64',
      'draft_id: Option<String>',
      'role: Option<String>',
      'position: Option<i64>',
      'position: i64',
    ]) {
      expect(ledger).toContain(`pub(super) ${field}`)
    }
    expect(ledger).not.toMatch(/\b(?:select|insert|update|delete)\b/i)
    expect(ledger).not.toMatch(/\b(?:rusqlite|std::fs|std::path|File|Connection)\b/i)
  })
})
