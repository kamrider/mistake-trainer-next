# Export plan

## Implemented locally

- Users explicitly choose a source: the due queue, the latest review session, or all active
  problems. They can search, select a visible subset, and review missing-answer warnings
  before saving.
- The ordered selection and layout are persisted as immutable `ExportSnapshot` data. Rust
  transactionally revalidates every problem before committing the snapshot and outbox row.
- Rust can regenerate original-image folders, question/answer alternating DOCX files, and
  grouped question-then-answer DOCX files without exposing filesystem paths to Vue.
- Active snapshots and the 30-day recycle area have explicit generate, delete, and restore
  feedback. Cancelling the native folder picker is a neutral result rather than an error.
- Reports continue to come from local read models and remain usable offline.

## Remaining release acceptance

- Open representative generated DOCX fixtures in the supported Microsoft Word version and
  approve image clarity, question/answer order, page breaks, long titles, and multi-image
  questions using visual snapshots.
- Exercise folder export with duplicate filenames, unavailable/corrupt encrypted assets,
  insufficient disk space, and the maximum 500-problem snapshot on the Windows release
  candidate.
- Complete cloud synchronization contract tests for snapshot metadata. Generated files and
  folders must remain local and reproducible rather than entering cloud storage.

Exit: a selected export reproduces from its snapshot, survives delete/restore policy, opens
correctly in Microsoft Word, and never depends on synchronizing generated artifacts.
