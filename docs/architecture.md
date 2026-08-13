# Mistake Trainer Next Architecture

## Decision

Mistake Trainer Next is a Windows-first, offline-first modular monolith. Vue renders
the product experience. Rust owns use cases, storage, encryption, file operations,
exports, scheduling, migration, and synchronization. Cloud synchronization is an
optional provider behind a Rust boundary; the local encrypted database is always
the source of truth for an active session. The default provider is `local-only`.

## Dependency rule

```text
Vue feature -> generated typed command client -> Tauri command -> Rust use case
  -> repository/port -> SQLite, encrypted blobs, Windows Credential Manager, cloud backend
```

- Vue never receives arbitrary filesystem access or database handles.
- Tauri commands validate DTOs and delegate; they do not contain domain policy.
- Domain modules do not depend on Tauri, SQL, HTTP, or the operating system.
- Infrastructure implements domain ports and is covered by integration tests.
- Frontend feature modules consume app-wide capabilities through contracts in `src/shared`;
  they do not import `src/app` implementations.
- Reusable modal, focus, menu, and confirmation mechanics live in `src/shared/ui`.
- Every consumed frontend feature exposes one `src/modules/<feature>/index.ts` public API.
  Production consumers outside that feature import the module root and never traverse
  `components`, `composables`, `domain`, or `services`.
- `App.vue` composes command adapters and providers. `useApplicationSyncLifecycle` owns
  cloud restore, sync phase transitions, browser triggers, cooldown, and success callbacks;
  `createSyncController` owns request single-flight and mutation debounce.
- `useLibraryAccessLifecycle` owns access phases and initialization admission.
  `useLibraryRecoveryController` owns reconnect, recovery restore, fresh-start, recovery
  dialog state, and cross-action single-flight; `App.vue` only adapts generated commands.
- `useProfileManagement` owns profile loading, shell projection, mutation admission and
  single-flight, workspace-transition policy, dashboard refresh policy, and profile mutation
  sync scheduling. `App.vue` only adapts commands and supplies router, guard, and sync capabilities.
- `useSettingsStorageLifecycle` owns settings-side storage status, migration receipt projection,
  migration dialog lifecycle, safe cancellation/failure handling, and restart transition.
  `SettingsView.vue` only adapts commands and supplies app capabilities.
- `useSettingsBackupOperations` is the complete settings backup controller: it owns manual,
  portable, restore, and automatic-backup admission, state, failure copy, restore-dialog focus,
  and navigation busy. `SettingsView.vue` only adapts commands and supplies focus/page callbacks.
- `useSettingsWindowsUpdate` owns the explicit settings compatibility/status probes, update
  check/install single-flight, exact-version policy, stale-report invalidation, and focus return;
  `useStartupUpdate` separately owns silent throttled startup checks.
- `useSettingsDiagnosticsExport` owns diagnostic export single-flight, native cancellation,
  privacy-safe failure copy, stale-receipt clearing, and focus restoration. The generated receipt
  exposed to Vue contains no absolute path.
- `useSettingsCloudSession` owns the coupled cloud-session and local-device boundary: credential
  restore/authentication, device access status, ordered sign-out-then-lock, restart admission,
  lock dialog state, and mode-aware focus restoration.
- `useCaptureCropPresentation` owns ordinary, development-preview, and recognition crop modal
  presentation, launcher/successor lookup, context-safe focus restoration, and pending-focus
  disposal. `CaptureView.vue` only adapts crop and recognition operations.
- `useCaptureQualityAnalysis` owns quality-check admission and caching, per-item reports/errors,
  dismissal, stale-result protection, batch-context reset, and crop-seed projection.
  `CaptureView.vue` only adapts the generated quality command and binds presentation state.
- `useCaptureBatchData` owns batch list/detail state, requested-detail identity, stale-response
  rejection, route projection, external detail replacement, clear invalidation, and development
  hydration. OCR preloading is registered after OCR composition, avoiding setup-order coupling.
- `useCaptureOrganizerActions` owns revision-aware organizer transactions, including pair-
  suggestion application, save/busy transitions, stale-result rejection, conflict refresh,
  input-invalid recovery copy, and success notices. `CaptureView.vue` only adapts commands.
- `useCaptureDraftPersistence` composes the lower-level save queue and owns update inputs,
  revision-conflict classification and retry, safe detail replacement, failure copy, blocked
  flushing, batch retention, readonly unsaved/busy projections, and disposal.
- Rust application policy is infrastructure-independent. `application/startup.rs` is the
  explicit composition root that may coordinate infrastructure and module operations.
- Pure privacy policy such as stable email-hint redaction lives in `domain::privacy`; both
  authentication state and Supabase debug formatting depend inward on that policy.
- Asset-reading use cases depend on the application-owned `AssetDecryptor` port. Capture-quality
  analysis, export generation, and problem preview/detail queries receive a decryptor from their
  command/composition boundary; sync push uses the same port before upload. Only
  `infrastructure::assets::KeyedAssetDecryptor` knows the concrete asset cipher and key. Stable
  plaintext content identity is the pure `domain::assets::plaintext_sha256` policy. Sync pull
  likewise receives the application-owned `AssetEncryptor` port before staging downloaded blobs;
  problem creation and capture ingestion use the same write capability.
- Derivation transactions that must decrypt a source and encrypt derived assets depend on the
  asset crypto ports: crop accepts the combined `AssetCipher`, while recognition uses the read
  and write capabilities on its bounded job input. Neither imports the concrete cipher.
- Sync pull commits database tombstones before invoking the application-owned
  `AssetBlobRemover`; infrastructure implements containment-safe filesystem deletion.
- Missing-library reset is an infrastructure recovery operation: it journals and removes only
  product-owned control files and the local credential envelope, never the abandoned data tree.
- Legacy import exposes feature-level error categories instead of infrastructure crypto errors;
  the import transaction consumes `AssetEncryptor` and maps encryption failures to that category.
- `pnpm contract:architecture` enforces dependency direction and ratchets remaining Rust
  module-to-infrastructure exceptions.

### Remaining Rust boundary batches

The ratchet is intentionally not presented as zero debt. The remaining seven files belong to
four larger migrations that must preserve filesystem and credential transaction semantics:

- `auth_sync.rs`: split provider/runtime construction and secret persistence behind auth ports.
- `backup_creation.rs`, `backup_portability.rs`, and `backup_validation.rs`: separate backup
  policy from encrypted-database opening, package I/O, and cross-key asset transformation.
- `storage_migration.rs` and `storage_migration_snapshot.rs`: separate migration policy from
  journal/control-file I/O, database snapshot creation, and runtime reopen.
- `product_check.rs`: relocate the installed-product smoke orchestrator to an explicit outer
  composition boundary; it intentionally exercises infrastructure and multiple use cases.

Each batch must remove its files from the exact architecture ratchet; adding a new exception is
always a contract failure.

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

## Local access lock boundary

- `library-lock-state` is stored beside the database and asset keys in Windows Credential
  Manager. Existing installations without the marker start unlocked; only the exact values
  `locked` and `unlocked` are accepted.
- Tauri reads the marker before library startup can open SQLCipher or load an asset key and
  keeps that exact decision in a process-scoped access gate. A later credential-store read
  can never make a process that started fail-closed claim to have a live runtime. A locked,
  unreadable, or malformed marker deliberately omits `LibraryRuntime`, so profile, problem,
  review, export, backup, and capture commands cannot acquire database state even if invoked
  outside Vue.
- Manual lock stops the temporary LAN phone collector, persists the marker, moves Vue into
  the root restarting boundary, and then restarts the process. Unlock first verifies the
  database key, asset key, and account identity through the current Windows account's
  credential access, clears the marker, and also restarts; the application never hot-loads
  encryption keys into a process that began in the locked state.
- Vue asks only for the typed access status before it mounts `AppShell`. Checking, locked,
  credential-error, unlocking, and restarting states cannot start profile or library reads.
  Browser preview remains an unlocked, command-free design surface.
- Cloud sign-out is local-first: it clears the refresh token and in-memory session before a
  bounded, best-effort remote revocation attempt. A slow or unreachable endpoint cannot
  preserve local credentials or block locking indefinitely. The settings flow never claims
  success after a credential-store or marker-write failure.
- The current signed-in Windows account is the offline trust boundary. No weak application
  password is derived, and locking never deletes, moves, or rewrites encrypted user data.

## Cloud backends

The `CloudBackend` port is provider-neutral and selected by Rust configuration:

| Mode | Intended use | Network requirement |
| --- | --- | --- |
| `local-only` | Default and offline-first operation | None |
| `supabase` | Development or overseas deployments | Supabase Cloud reachable |
| `tencent` | Mainland-China deployment | Domestic API/storage endpoint |

Vue must not import a vendor SDK or construct a cloud URL. A backend that is not
configured fails closed and leaves the local outbox intact for a later retry.

## Conflict policy

Non-overlapping fields merge automatically. Review events form a set union. Assets
deduplicate by hash. Same-field edits with a common base revision create a visible
conflict. Deletes create tombstones retained for 30 days.

## Background sync boundary

- The encrypted local transaction is authoritative. Profile, problem, review, export,
  migration, and conflict-resolution commands report success as soon as local state and
  its outbox operation commit; a later cloud failure cannot undo or relabel that save.
- Vue restores the cloud session only after the local library unlocks. Startup, actual
  network recovery, return to the foreground, explicit manual action, and successful
  cloud-visible local mutations are finite triggers; there is no polling interval.
- Mutation notifications are debounced. The app controller coalesces concurrent triggers,
  waits for an older request, and always starts a fresh pass for a mutation that may have
  missed that request's leased outbox batch. Failed automatic requests do not self-loop.
- Rust owns a process-wide sync permit in addition to the Vue single-flight guard, so
  forged or concurrent command invocations cannot overlap push/pull transactions.
- Local-only, signed-out, and offline phases do not schedule mutation sync. Their outbox
  remains intact for manual, startup, or network-recovery retry.
- Active LAN phone capture takes priority. Sync reads the LAN session state and returns a
  stable deferred result instead of stopping the server or interrupting an upload.
- Upload-only completion updates profiles and the global sync indicator without remounting
  the active route. A non-mutation sync may refresh a page only after it actually pulled
  changes, and never while the training room is active.

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
- Capture quality analysis is an explicit, item-scoped local command. Rust verifies batch/item
  ownership, decrypts the active asset only for the bounded command, downsizes analysis input to
  at most 1,024 px, and returns scores and advisory issue codes without persistence or outbox work.
- Quality suggestions never block capture commit and never auto-apply. A suggested crop may seed
  the existing editor as a reversible recipe; a small-angle skew is shown as guidance to use the
  explicit perspective tool because the persisted rotation contract remains quarter-turn only.
- Perspective correction accepts four normalized points only after convexity, clockwise order,
  minimum area, range, and output-size validation. Rust rectifies with bounded inverse bilinear
  sampling before the normal crop/rotation/encode pipeline. The derivation ledger remains the undo
  boundary and encrypted source assets stay immutable.

## Library retrieval and metadata boundary

- The canonical quick-reason catalog is represented by ordinary normalized tags prefixed with
  `错因·`; the six supplied chips reuse the same 20-tag, 30-character persistence contract as
  free-form tags and do not create a second classification store.
- Library reads accept one typed query. Search is limited to 100 characters and treats SQLite
  wildcard characters literally; subject/tag arrays contain at most 20 values of at most 30
  characters. Rust builds fixed predicates, applies OR within a filter group and AND across groups,
  and always injects runtime account/profile identity.
- Bulk metadata edits accept 1 through 100 unique active problem IDs. Rust validates the complete
  selection before changing a row, then applies normalized subject/tag changes, revision increments,
  and canonical outbox upserts atomically. Foreign, duplicate, archived, or invalid input changes
  neither problems nor outbox state.

## Dashboard read boundary

- The training dashboard is a local, profile-scoped read model derived from the encrypted
  library. It remains useful offline and does not depend on Supabase availability.
- Due counts, current streak, today's reviews, 30-day remembered rate, and unfinished
  capture work are queried from persisted records. A read failure is shown explicitly;
  the UI never substitutes demo statistics or retains stale values as if they were live.
- Calendar-day metrics use the Windows browser's current UTC offset. The offset is
  range-checked in Rust, and a streak may begin today or yesterday so a user does not
  lose it before completing the current day's review.
- Learning reports expose weak areas only after at least two matching review events. They combine
  subjects with canonical `错因·` tags, use 30-day lapses, cap the ordered result at five, and return
  an empty list with sparse-evidence guidance rather than manufacturing a conclusion.
- The due forecast contains today plus the next six local calendar dates. Rust range-checks the
  supplied UTC offset, assigns future due work to local-day buckets, and reports already-overdue
  work separately on today.

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
- Quick review presets are Rust-owned manual sessions: five minutes selects at most eight cards,
  ten problems at most ten, and recently forgotten at most twenty. Optional subject/tag filters are
  validated server-side; every preset selects due cards before new cards with deterministic
  `(due_at, updated_at, id)` ordering and persists the session before Vue navigates.
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

## Review history boundary

- Review history is a profile-scoped read model over append-only `review_events`. Every list,
  count, subject, and detail query receives the account and active profile from
  `LibraryRuntime`; neither identity can be supplied by Vue. Joins back to `problems` also
  require the event and problem account/profile pair to match, so corrupt cross-profile links
  fail closed instead of exposing problem metadata.
- Ordering is deterministic on `(occurred_at_utc_ms DESC, id DESC)`. Pagination uses a
  bounded URL-safe base64 cursor containing only that pair, so equal-time events are neither
  skipped nor duplicated. Malformed or oversized cursors fail closed.
- Search accepts at most 80 characters and escapes SQLite `LIKE` wildcards. List DTOs contain
  an opaque event ID for detail selection, but never contain a problem ID, raw device ID,
  filesystem path, database handle, or image bytes.
- Event algorithm and parameter versions are immutable audit facts. The detail command emits
  only `isCurrentDevice`; raw device identity stays in Rust. Missing and foreign events share
  the same not-found response.
- Schedule values in a detail response are the current derived projection, not a historical
  snapshot. The UI labels this boundary explicitly and shows current/history badges against
  the running FSRS and parameter versions.
- Event IDs stay out of routes and browser history. The lazy master-detail page uses explicit
  pagination, preserves already visible rows after a failed read, rejects stale async detail
  responses, restores focus after the mobile sheet closes, and respects reduced motion.

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

## Legacy migration boundary

- The native folder picker is the only source of a legacy path. Rust keeps that path in a
  single in-memory candidate behind an opaque UUIDv7 token; a new scan replaces the old
  candidate, and every candidate expires after 30 minutes. Vue receives counts, bounded
  issue descriptions, and opaque IDs only.
- Candidate construction rejects truncated input, escaping paths, symbolic links, Windows
  junctions/reparse points, oversized metadata, oversized images, and trees above the
  documented limits. Import reparses the source, compares a complete SHA-256 tree
  fingerprint, then compares the fingerprint again immediately before commit.
- Records sharing a legacy `pairId` form one ordered problem with every mistake image as a
  question asset and every answer image as an answer asset. A question without an answer is
  retained as an incomplete question-only problem; an orphan answer is reported and skipped.
- Legacy `success`/`fail` records become append-only `good`/`again` events under
  `legacy-proficiency-v1` and `legacy-import-v1`. `nextTrainingDate` seeds the first due time;
  frozen questions become members of a per-profile `旧版冻结批次` export snapshot.
- New image plaintext is decoded, hashed, AES-GCM encrypted into a private staging area, and
  moved to its final shard only inside the compensated import transaction. Existing
  account-owned plaintext hashes are reused. Any parsing, encryption, file, fingerprint, or
  SQL failure removes new blobs and leaves both databases unchanged.
- The v10 ownership ledger distinguishes entities created by an import from reused assets.
  Rollback deletes only unchanged import-owned entities. A problem changed or reviewed after
  import is preserved with its coherent imported/new review history; referenced assets and
  profiles are preserved with it. Every actual deletion writes a 30-day tombstone and an
  idempotent delete outbox operation so an already-synced entity cannot reappear remotely.
- Import and rollback hold the profile-transition lock before the database lock. Public error
  messages are fixed and diagnostic-ID based; source paths, SQL text, encryption keys, account
  identity, and image filenames never enter page state or serialized errors.

## Learner profile deletion

- An account always retains at least one learner profile. Deleting any other profile requires
  typing its exact current name; forged IDs and stale names fail without changing the library.
- The SQLite transaction selects the oldest remaining profile as a deterministic fallback,
  switches `account_preferences` only when necessary, cascades profile-owned data, records an
  account-scoped 30-day tombstone, and queues deletion before orphan-asset operations.
- Assets are account-wide and deduplicated. Profile deletion removes only assets no longer
  referenced by `problem_assets` or `capture_items`; encrypted blob removal happens after commit
  through a validated relative path and is safe to retry.
- The profile transition lock serializes deletion with profile switching and LAN capture. An
  active phone session is stopped before mutation, and `LibraryRuntime` changes its active
  profile only after the database transaction commits.
- Supabase profile tombstones have no profile foreign key, so they survive remote cascade. The
  database rejects deletion of the final remote profile, preserves shared assets, creates asset
  tombstones for new orphans, and exposes every tombstone through the ordered account feed.
- Pull applies the same last-profile and ownership invariants locally, advances the cursor in the
  same transaction, removes committed orphan blobs afterward, and refreshes the in-memory active
  profile before reporting sync success.

## Smart image modes

- `本地智能识图` is the shipped, review-first mode. On Windows x64 it prefers a separately
  installed and hash-verified PP-OCRv6 small component to locate question-number anchors; if the
  model is absent or unusable, it falls back to the deterministic Rust visual-layout splitter.
  Neither path performs inference over the network.
- OCR strings and question numbers are transient worker data. They are used only to assemble crop
  boundaries and same-number question/answer groups, then discarded. They never enter SQLite,
  logs, Tauri events, generated bindings, or Vue state.
- A source item still has an explicit user-controlled `question` or `answer` role. Matching means
  equal recognized number plus deterministic reading order inside the same recognition operation;
  it is not semantic answer understanding.
- `语义识题` remains a future mode for subject inference, question/answer meaning, full text
  transcription, automatic judgment, solving, and export. The UI marks that capability as
  unavailable. PP-OCRv6 medium is not selected for crop splitting.

## Local smart-image splitting boundary

- Schema v14 adds account/profile/batch-scoped recognition jobs, item snapshots, review
  suggestions, and a reversible-operation ledger. Schema v15 raises derivation positions to the
  150-item batch limit. Schema v16 adds persistent recognition pairs and their role-specific item
  links. These rows remain local and never enter the sync outbox.
- The capture workbench treats splitting as a suggestion layer over unassigned, active items in
  an `organizing` batch. Existing drafts and manually assigned items remain canonical. A source
  marked as a question produces question materials; an answer source produces answer materials.
- The production worker selects `ProductRecognitionEngine`. A lazily initialized, verified small
  OCR component produces numbered anchor regions when available. Weak anchors, a detector box
  crossing one proposed boundary, or any runtime failure remain reviewable through a conservative
  whole-page/visual fallback. Multi-row watermark overlays are excluded without weakening the
  single-boundary content-cut guard.
- A single managed worker decrypts one scoped asset at a time into an application-owned
  private temporary directory, runs no more than one engine call concurrently, emits
  only job/batch IDs and bounded progress, and removes plaintext on success, per-item failure,
  cancellation, batch discard, and app shutdown. Startup deletes interrupted suggestions and
  replays the abandoned job; a corrupt encrypted source still fails only that item.
- Review separates high-confidence, needs-review, insufficient, and stale results.
  Low-confidence and stale results cannot be accepted, and proposal crop editing does
  not create assets.
- Applying suggestions is a separate atomic boundary. All derived blobs are prepared under
  `.staging`, then encrypted assets, ordinary crop derivations, unassigned `capture_items`,
  source supersession, complete question/answer pair rows, and the operation ledger commit in one
  compensated transaction. It creates no draft, Problem, or sync outbox row. The source is
  retained; unmatched question or answer regions remain ordinary unassigned materials.
- The material deck renders complete pairs as grouped question and answer previews. Applying one
  or many pairs creates capture drafts and draft-item links in one SQLite transaction. Any stale,
  moved, role-changed, already-assigned, or missing item aborts the whole request. Formal Problems
  and sync outbox operations are still created only by the existing explicit ready-draft commit.
- The operation ledger supports a persistent, revision-scoped undo. Undo succeeds only
  while every generated item still matches the applied state and no generated asset is
  referenced by a formal problem or later derivation. Any later edit returns a conflict without
  deleting user work.
- The Windows x64 installer bundles only the pinned Microsoft ONNX Runtime 1.20.1 CPU libraries
  and notices. The small model is an explicit optional install with fixed HTTPS sources, lengths,
  and hashes. Startup does not load it; first recognition lazily initializes it with at most four
  CPU threads while leaving one logical processor free when possible.
- Backups at schema v17 require the recognition and pair tables, run SQLite foreign-key
  validation, and reject cross-account, cross-profile, cross-batch, or inconsistent pair-state
  references. Schema v16 backups remain readable and migrate forward on restore. Optional model
  caches, runtime caches, OCR strings, decrypted images, and absolute paths remain outside backup
  scope.

## Performance budgets

- Initial JavaScript: at most 300 KB gzip.
- Any lazy feature chunk: at most 120 KB gzip.
- Core interactions: no main-thread task over 50 ms.
- Target cold start: below 1.5 s on Windows 11, 4 cores, 8 GB RAM, SSD.
