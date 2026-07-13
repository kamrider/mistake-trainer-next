# Mistake Trainer Next Architecture

## Decision

Mistake Trainer Next is a Windows-first, offline-first modular monolith. Vue renders
the product experience. Rust owns use cases, storage, encryption, file operations,
exports, scheduling, migration, and synchronization. Supabase is a remote replica,
not the source of truth for an active local session.

## Dependency rule

```text
Vue feature -> generated typed command client -> Tauri command -> Rust use case
  -> repository/port -> SQLite, encrypted blobs, Windows Credential Manager, Supabase
```

- Vue never receives arbitrary filesystem access or database handles.
- Tauri commands validate DTOs and delegate; they do not contain domain policy.
- Domain modules do not depend on Tauri, SQL, HTTP, or the operating system.
- Infrastructure implements domain ports and is covered by integration tests.

## Modules

| Module | Owns |
| --- | --- |
| auth-sync | session, trusted-device unlock, outbox, conflicts, sync status |
| profiles | private learner profiles owned by one account |
| inbox-library | capture, asset deduplication, problem aggregate, trash |
| review | review queue, FSRS mapping, sessions, exam and focus modes |
| export-report | immutable export snapshots, DOCX/image export, reports |
| migration-settings | legacy import, backup, storage migration, diagnostics |

## Local invariants

- IDs are UUIDv7 and timestamps are UTC.
- Review events are append-only. Schedule state is derived and rebuildable.
- Assets are immutable, encrypted at rest, and deduplicated by plaintext SHA-256.
- Every cloud-visible mutation writes its entity change and outbox operation in one
  SQLite transaction.
- A sign-out revokes remote credentials and locks the local database.
- User-provided paths never cross a Tauri command boundary.
- One encrypted local library belongs to exactly one account. Account switching opens a
  different keyed library; backup creation and validation fail closed if foreign account
  rows are present.

## Conflict policy

Non-overlapping fields merge automatically. Review events form a set union. Assets
deduplicate by hash. Same-field edits with a common base revision create a visible
conflict. Deletes create tombstones retained for 30 days.

## Backup boundary

- A backup is a new directory containing one consistent SQLCipher snapshot, immutable
  encrypted asset blobs, and a plaintext manifest of relative paths, sizes, ciphertext
  hashes, schema version, creation time, and a one-way account hash.
- Creation holds the database lock while taking the SQLite online backup and collecting
  the immutable asset set. The package is written to a private temporary sibling and is
  renamed into place only after the manifest is durable; failures remove only that new
  temporary directory.
- Validation opens the snapshot read-only and checks schema/account ownership, bounded
  paths and sizes, ciphertext hashes, AES-GCM authentication, and plaintext length/hash
  against the SQLCipher-protected asset rows. It never opens the live library for writes.
- Version 1 packages rely on the current trusted Windows account/device credentials.
  Cross-device restore requires the future auth-sync key envelope; it must not be added
  by deriving encryption keys from a weak user password.
- Validation means “eligible for restore staging”, not “restored”. Atomic staging and
  next-start replacement are a separate operation and are not implemented yet.
- Backup packages cannot be created under the application-owned library root. Validation
  rejects SQLite journal/WAL sidecars and checks a private temporary copy of `library.db`,
  so SQL reads are bound to the same bytes whose ciphertext hash was accepted.

## Performance budgets

- Initial JavaScript: at most 300 KB gzip.
- Any lazy feature chunk: at most 120 KB gzip.
- Core interactions: no main-thread task over 50 ms.
- Target cold start: below 1.5 s on Windows 11, 4 cores, 8 GB RAM, SSD.
