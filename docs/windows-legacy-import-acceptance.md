# Windows legacy import acceptance

Use a copy of legacy data for interactive checks. The implementation is read-only, but the
copy keeps acceptance independent from the user's only historical archive. Record the source
tree hash before and after every case.

## Safety contract

- The folder picker is the only place where a source path is accepted.
- Vue receives an opaque candidate ID, counts, and bounded issues; it never receives a path,
  database handle, account/profile identity, encryption key, or source filename.
- A candidate expires after 30 minutes and a later scan invalidates the previous candidate.
- Import reparses and fingerprints immediately before staging, then fingerprints again before
  commit. A changed source aborts and removes every new-side temporary/final blob.
- Limits are 512 members, 100,000 metadata records, 64 MiB per asset, 8 GiB total source
  bytes, 16 MiB metadata, 10,000 reported issues, and 12,000 px / 80 MP decoded images.
- Symbolic links and Windows junction/reparse points are rejected. Relative paths must contain
  normal components only and every canonical path must remain under the selected root.
- Rollback removes only unchanged, import-owned entities. Reused or subsequently referenced /
  edited data is preserved. Actual removals receive 30-day tombstones and delete outbox rows.

## Mapping checks

| Legacy input | Expected v1 result |
| --- | --- |
| member directory | uniquely named private learner profile |
| mistake + answers with one `pairId` | one problem, ordered question/answer assets |
| unpaired mistake | question-only incomplete problem |
| orphan answer | skipped with `orphan_answer` issue |
| subject, tags, notes, answer time limit | bounded problem fields |
| `success` / `fail` | `good` / `again` append-only review event |
| `nextTrainingDate` | initial schedule due time |
| frozen problem | profile-local `旧版冻结批次` export snapshot |
| duplicate plaintext image | reuse existing account asset by SHA-256 |

## Automated matrix

- [x] Normal multi-member and multi-image pair import.
- [x] Question-only and orphan-answer mapping.
- [x] Duplicate plaintext deduplication and encrypted-blob authentication.
- [x] Missing asset, corrupt metadata, corrupt image, oversized metadata/image, and truncated
  scan behavior.
- [x] Escaping relative path, external junction, and internal Windows reparse-point rejection.
- [x] Source fingerprint equality before/after successful import and rollback.
- [x] Source change between preflight and import rejection.
- [x] Injected final-blob move failure leaves zero imported rows and no staging files.
- [x] Duplicate import rejection, foreign/already-rolled-back ID rejection, and path-redacted
  command errors.
- [x] Ownership-safe rollback, changed-problem preservation, reusable-asset preservation,
  delete outbox compensation, and 30-day tombstones.
- [x] v9 to v10 schema migration and v1-v9/v10 backup validation behavior.
- [x] Candidate UUIDv7 opacity, replacement, expiry, successful consumption, and stale progress
  filtering.
- [x] Confirmation acknowledgement, cancel-first focus, Escape restoration, progress semantics,
  successful result, retry state, history, rollback confirmation, and reduced-motion CSS.

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm tauri build
```

## Interactive Windows matrix

- [ ] At 1280 × 820, scan a normal legacy copy, inspect all counts, keyboard-confirm import,
  switch to each new profile, open representative multi-image and incomplete problems, and
  confirm the library/review history/export snapshot mapping.
- [ ] At 390 × 844, confirm no horizontal overflow, 44 px targets, contained issue text,
  cancel-first dialog focus, focus return, Escape close, and immediate reduced-motion states.
- [ ] Restart after import and verify profiles, problems, assets, schedules, receipt history,
  and the rollback action remain available.
- [ ] Modify one imported problem and add a new review; rollback must preserve that problem,
  its imported/new history, referenced assets, and parent profile while removing untouched data.
- [ ] Disconnect/reconnect during future sync acceptance; rollback delete operations must replay
  idempotently and deleted imported entities must not reappear on a second device.
- [ ] Hash the legacy copy again. It must match the preflight hash byte-for-byte, including after
  failed import, retry, successful import, app restart, and rollback.

Do not mark the interactive boxes complete from browser preview or unit tests. They require the
packaged Tauri application and a representative anonymized legacy copy on Windows.
