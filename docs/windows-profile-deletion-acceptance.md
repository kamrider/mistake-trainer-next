# Windows learner-profile deletion acceptance

Run this matrix in a packaged Windows build with a disposable encrypted library. Repeat the cloud
section on a development Supabase project after applying all migrations.

## Local and interaction checks

- [ ] Create three profiles with distinct questions, reviews, exports, and unfinished capture
  batches. Delete an inactive profile and confirm the current workspace does not change.
- [ ] Delete the active profile and confirm the app returns to the training desk using the oldest
  remaining profile. Restart the app and confirm the same profile remains active.
- [ ] Confirm no delete action appears when only one profile remains.
- [ ] Enter an incomplete, whitespace-modified, old, or case-modified profile name. The permanent
  delete action must remain unavailable and no data may change.
- [ ] Navigate the confirmation view by keyboard only. Escape must cancel and return focus to the
  profile trigger; reduced-motion mode must remove visible transforms.
- [ ] Start phone capture for the target profile, then delete it. The LAN page must stop responding
  before deletion commits, and no port/session may remain active.
- [ ] Use one image in problems from two profiles, plus one image used only by the target. After
  deletion, the shared image must still render and the orphan asset row/blob must be gone.
- [ ] Create and restore an encrypted backup before and after deletion. Both restored libraries
  must open with a valid active profile and complete referenced blobs.

## Offline and two-device checks

- [ ] Delete a profile while offline. Confirm one learner-profile delete and only true orphan-asset
  deletes remain pending; continue using the surviving profile and restart successfully.
- [ ] Reconnect and sync device A. On device B, sync and confirm the profile, questions, reviews,
  and exports disappear while shared assets remain. Local capture drafts remain device-only and
  are covered by the local deletion check above.
- [ ] Repeat sync on both devices. Counts and tombstones must remain stable and no profile may
  reappear.
- [ ] Attempt to delete the final profile through the UI and directly through the development RPC.
  Both must fail without removing the canonical profile.
- [ ] Attempt a foreign-account entity ID. RLS/RPC and the local ownership check must reject it
  without revealing names, paths, or row contents.

## Automated evidence

- Rust: `profile_store` (6), `profile_command` (5), and `sync_pull` (4) tests pass.
- Vue: profile switcher and App orchestration tests pass (9 total).
- Supabase: `0003_profile_deletion.test.sql` covers cascade, shared/orphan assets, and last-profile
  refusal. It requires Docker Engine or a disposable hosted development database.
- SQLCipher `VirtualLock` warning 1453 and OpenSSL static PDB linker warnings are known non-fatal
  development-machine warnings; any other failure blocks release.
