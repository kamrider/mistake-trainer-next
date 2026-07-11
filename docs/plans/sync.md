# Sync plan

1. Commit mutations and outbox operations atomically.
2. Push idempotent batches, upload immutable assets, then pull by change sequence.
3. Merge non-overlapping fields, union review events and surface true conflicts.
4. Test long offline periods, interrupted uploads, expired sessions and two-device races.

Exit: repeated push/pull is idempotent and no account can read another account through RLS.
