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

## Capture inbox boundary

- Capture batches, drafts, and their item assignments live in the encrypted local
  library. They survive restart, but never enter the sync outbox before a draft is
  atomically committed as a `Problem`.
- Imported images pass through Rust validation, plaintext SHA-256 deduplication, and
  AES-GCM blob encryption. Discard only removes assets proven to be orphaned from both
  capture drafts and formal problems.
- The phone collector is an explicitly started, single-session HTTP server bound to a
  selected RFC1918 address. Its 256-bit bearer token stays in the QR URL fragment and
  memory; it is removed from browser history and is never logged or persisted.
- LAN capture is for a trusted home Wi-Fi or personal hotspot only. After explicit
  elevation, the app installs one persistent firewall rule scoped to its own executable
  and all Windows network profiles; a successful rule is reused, while a failed or
  missing rule is requested again on the next start. The app does not present ordinary
  HTTP as protection on public networks. Sessions expire after 30 minutes idle or two
  hours absolute time.
- HEIC/HEIF conversion is a separate same-origin decoder loaded only after such a file
  is selected. The mobile page has no CDN or third-party requests. The decoder is a
  codec exception to the Vue feature-chunk budget and does not enter the desktop's
  initial JavaScript bundle.

## Dashboard read boundary

- The training dashboard is a local, profile-scoped read model derived from the encrypted
  library. It remains useful offline and does not depend on Supabase availability.
- Due counts, current streak, today's reviews, 30-day remembered rate, and unfinished
  capture work are queried from persisted records. A read failure is shown explicitly;
  the UI never substitutes demo statistics or retains stale values as if they were live.
- Calendar-day metrics use the Windows browser's current UTC offset. The offset is
  range-checked in Rust, and a streak may begin today or yesterday so a user does not
  lose it before completing the current day's review.

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
