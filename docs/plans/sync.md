# Sync plan

The first cloud walking skeleton is deliberately manual and offline-first:

1. Commit mutations and outbox operations atomically in the encrypted local library.
2. Bind one local library to one remote account; keep refresh credentials in Windows Credential Manager.
3. Push bounded, dependency-ordered batches through `push_sync_batch`, upload immutable assets first, then pull account changes by monotonic `change_seq`.
4. Apply a pull page and advance the local cursor in one SQLite transaction. Review events merge by set union; schedules are rebuilt locally from the complete event set.
5. Keep the cloud provider behind a transport boundary. A build without Supabase configuration remains local-only, so domestic or restricted networks do not block capture, library, review, or export.

The Supabase implementation uses only a project URL and publishable key at build time. It does not require a service-role key in the desktop app. If a hosted Supabase endpoint is unreachable, the app preserves the outbox and exposes a retryable sync error rather than making the local library unavailable.

Exit criteria:

- repeated push/pull is idempotent;
- no account can read another account through RLS;
- an interrupted asset transfer can resume or retry without corrupting the encrypted local blob;
- a stale problem revision cannot erase newer remote question/answer links;
- the pgtap contract passes against a real local or hosted development database before enabling a production cloud project.
