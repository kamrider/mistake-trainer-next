# Sync plan

The cloud walking skeleton remains offline-first. Manual sync is always available, while
safe automatic triggers now reduce the chance that committed local work waits unnoticed:

1. Commit mutations and outbox operations atomically in the encrypted local library.
2. Bind one local library to one remote account; keep refresh credentials in Windows Credential Manager.
3. Push bounded, dependency-ordered batches through `push_sync_batch`, upload immutable assets first, then pull account changes by monotonic `change_seq`.
4. Apply a pull page and advance the local cursor in one SQLite transaction. Review events merge by set union; schedules are rebuilt locally from the complete event set.
5. Keep the cloud provider behind a transport boundary. A build without Supabase configuration remains local-only, so domestic or restricted networks do not block capture, library, review, or export.
6. Represent learner-profile and asset deletion with account-scoped tombstones. Remote and local
   apply both refuse deletion of the final profile, cascade profile-owned rows, preserve shared
   assets, and delete only newly orphaned asset metadata/blobs.
7. Restore the cloud session after the encrypted workspace unlocks, and sync on startup,
   actual network recovery, return to the foreground, and a successful cloud-visible local
   mutation.
8. Coalesce concurrent triggers through one Vue single-flight controller and one Rust
   process-wide coordinator. A short mutation debounce merges quick edits and ratings; a
   mutation committed during an older sync always receives a fresh later pass.
9. Keep local-only, signed-out, and offline sessions quiet. Local success never depends on
   cloud availability, and failed automatic sync never loops indefinitely.
10. Never stop an active LAN phone collection to make sync proceed. Sync returns a stable
    deferred result, keeps the upload session alive, and retries on a later eligible trigger.
11. Do not remount the training room after background upload. A page refresh occurs only
    after a non-mutation sync actually pulls remote changes, and is deferred while training.

The Supabase implementation uses only a project URL and publishable key at build time. It does not require a service-role key in the desktop app. If a hosted Supabase endpoint is unreachable, the app preserves the outbox and exposes a retryable sync error rather than making the local library unavailable.

Exit criteria:

- repeated push/pull is idempotent;
- no account can read another account through RLS;
- an interrupted asset transfer can resume or retry without corrupting the encrypted local blob;
- concurrent startup/online/manual/mutation triggers invoke at most one Rust sync at a time;
- a mutation committed during that request is not swallowed and receives a later pass;
- active phone capture is not terminated by sync;
- local-only and offline saves remain successful without repetitive cloud errors;
- a stale problem revision cannot erase newer remote question/answer links;
- deleting a profile on one device removes it on the next pull, selects a surviving local profile,
  preserves shared images, and cannot delete the account's last profile;
- the pgtap contract passes against a real local or hosted development database before enabling a production cloud project.
