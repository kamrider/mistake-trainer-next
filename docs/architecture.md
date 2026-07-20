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

## Review session boundary

- Rust owns the active review session. A queue response includes its opaque session ID,
  original total, persisted completed count, and whether the session was resumed; Vue
  renders that state instead of reconstructing progress from the remaining cards.
- One profile has at most one active session. Opening the training room resumes that
  session regardless of whether it is a due queue or a user-selected deck; it creates a
  new due queue only when no active session exists.
- A manual deck contains 1 through 100 unique active problems. Rust validates account,
  profile, status, uniqueness, and caller order before replacing the current session.
  Any invalid selection leaves the previous session byte-for-byte unchanged.
- The library persists a manual deck through `review_manual_start` before navigation.
  The ordinary `review_queue` command accepts no entity IDs, so selected problem IDs do
  not enter routes, browser history, logs, or filenames. Leaving the room intentionally
  preserves unfinished progress for the next entry.
- A rating appends one `ReviewEvent`, updates the derived schedule, advances the active
  session, and writes the sync operation in one transaction. Failed submissions keep the
  current answer visible and retryable; the UI never advances optimistically.
- A problem's optional answer time limit is persisted with the problem and range-checked
  from 1 through 86,400 seconds. The training clock uses a monotonic source and freezes
  when the answer is revealed. Expiration is feedback only and never fabricates a rating.
- Review media is displayed in stored role and position order. The original encrypted
  asset is decrypted by Rust for the current detail DTO; Vue can enlarge it in a focused,
  keyboard-contained lightbox but receives no filesystem path or blob-store handle.
- Review session source (`due | manual`) is independent from its experience
  (`review | exam`). An exam persists an `answering` pass, its question position, a
  separate `grading` pass, and correct/wrong counters so either pass resumes after a crash.
- Vue cannot request an arbitrary problem while training. `review_current_problem` derives
  the current opaque problem from the active session. During an exam's answering pass Rust
  removes every answer asset before constructing the DTO; only the grading pass can receive
  answer media. Ratings submitted before grading are rejected transactionally.
- Exam grading reuses the normal FSRS transaction. The review event, schedule projection,
  sync outbox operation, session advance, and exam score counter either all commit or all
  roll back.
- The optional Schulte focus policy is profile scoped but snapshotted into each newly
  created ordinary session. Existing sessions retain their original policy, and exam
  sessions explicitly persist `off` so a settings change cannot interrupt an exam.
- Rust owns every focus-board transition. The deterministic 1–25 board, next number,
  elapsed time, and round index are stored on the active session. Current-problem reads
  and ratings fail closed while a board is active; a correct selection or skip commits
  before Vue changes the visible state.
- `session_start` inserts one warm-up before the first card. `every_10` inserts a break
  after cards 10, 20, and so on only when another card remains. The final card can never
  create a trailing board. A stale client selection reloads the authoritative queue.
- The board does not visually reveal the next tile. Wrong choices remain local feedback,
  while keyboard roving, 44 px targets, reduced-motion behavior, and a persistent skip
  affordance keep the optional exercise accessible and non-blocking.

## Export boundary

- Export candidate queries are profile-scoped, read-only projections. They never create,
  resume, cancel, or advance a review session. Due candidates use the same rule as the
  review queue; the latest-session source preserves its persisted problem order, and the
  all-active source has a deterministic updated-time order.
- Vue receives only opaque problem IDs and display metadata. It owns transient filtering
  and selection, while Rust revalidates account ownership, profile ownership, status,
  uniqueness, order, and the 500-problem limit when the immutable snapshot is created.
- A snapshot records the ordered selection and layout needed to reproduce an export.
  Generated DOCX files and image folders remain local artifacts and are not placed in the
  synchronization outbox.
- Generation prepares and validates the snapshot while the encrypted database is locked,
  releases that lock before showing the native folder picker, and performs filesystem work
  afterward. Vue receives a safe filename or cancellation result, never an output path.
- Deleted, foreign, corrupt, or oversized exports fail with stable user-facing error codes.
  Diagnostic details may be logged under a diagnostic ID but must not expose asset paths or
  database internals to the page.

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
- Restore preparation validates the selected package, copies only the manifest-listed files
  into an application-owned opaque candidate, and validates that copy again. Vue receives a
  UUID token and summary only; selected paths and application-data paths never enter page state.
- Confirmation writes a bounded pending marker and restarts the app. Before the first SQLCipher
  connection opens, startup revalidates the candidate and swaps the complete `library` directory
  on the same volume. The previous directory remains as an app-owned rollback child until the
  restored database has migrated and selected a valid active profile.
- Interrupted pre-swap, mid-swap, and post-swap states resume deterministically on the next
  launch. A damaged restored root is rolled back before it opens; a one-time bounded receipt
  reports success, automatic rollback, or final validation failure to the application shell.
- Backup packages cannot be created under the application-owned library root. Validation
  rejects SQLite journal/WAL sidecars and checks a private temporary copy of `library.db`,
  so SQL reads are bound to the same bytes whose ciphertext hash was accepted.

## Performance budgets

- Initial JavaScript: at most 300 KB gzip.
- Any lazy feature chunk: at most 120 KB gzip.
- Core interactions: no main-thread task over 50 ms.
- Target cold start: below 1.5 s on Windows 11, 4 cores, 8 GB RAM, SSD.
