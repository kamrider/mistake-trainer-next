# Library plan

## Delivered

1. Persistent encrypted capture batches, drafts, assignments, restart recovery, and
   orphan-safe discard.
2. Desktop multi-select, clipboard paste, file drop, lazy previews, deterministic
   organizing templates, keyboard moves, and atomic commit of every ready draft.
3. Explicit LAN phone sessions with QR handoff, upload idempotency, progress events,
   trusted-private-network selection, expiration, deletion, and finish-to-organize.
4. Mobile continuous camera and album selection, metadata-stripping normalization,
   two-file concurrency, retry/backoff, and lazily loaded HEIC/HEIF conversion.

## Remaining release acceptance

1. Run the real-device matrix in `docs/windows-capture-acceptance.md` on iPhone Safari
   and Android Chrome.
2. Record the 150-image scroll/drag profile and the 1 GB restart-recovery fixture on
   the Windows reference machine.
3. Complete signed installer and update-channel work in the Release plan; bundle
   configuration already includes all mobile decoder notices and license texts.

Exit: normal and damaged legacy fixtures import without mutating their source hashes,
and the documented phone/desktop capture matrix passes on real hardware.
