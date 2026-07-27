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
5. Guided legacy migration with bounded read-only preflight, 30-minute opaque candidates,
   pair-aware parsing, atomic encrypted import, progress events, import history, and
   ownership-safe rollback with sync deletion compensation.
6. End-to-end mistake tags: normalized 20-tag/30-character validation, subject/note/tag
   search, compact card chips, accessible keyboard editing, detail display, and atomic
   revision/outbox persistence.

## Remaining release acceptance

1. Run the real-device matrix in `docs/windows-capture-acceptance.md` on iPhone Safari
   and Android Chrome.
2. Record the 150-image scroll/drag profile and the 1 GB restart-recovery fixture on
   the Windows reference machine.
3. Complete signed installer and update-channel work in the Release plan; bundle
   configuration already includes all mobile decoder notices and license texts.

## Gated OCR and question reconstruction roadmap

OCR remains a later, evidence-gated derivative workflow; it is not part of the canonical
capture record or the current Windows v1 release gate.

1. The implemented isolated comparison is documented in
   `docs/superpowers/plans/2026-07-23-ppocrv6-question-content-bakeoff.md`.
2. Safe question-region suggestions and their 60/300-image gates are documented in
   `docs/superpowers/plans/2026-07-22-automatic-question-region-suggestions.md`.
3. The post-gate product cascade—conditional OpenCV/UVDoc, PP-OCRv6 small anchors,
   PP-DocLayout-M block protection, on-demand PP-OCRv6 medium/PP-FormulaNet,
   optional PaddleOCR-VL-1.6, editable Markdown/LaTeX/image blocks, and future
   consented RT-DETR training—is locked in
   `docs/superpowers/plans/2026-07-24-verified-question-content-pipeline.md`.

Until those gates pass, keep the encrypted color source, manual crop, and image-based
question/answer workflow as the trustworthy default.

### Smart organizing delivery boundary

The first recognition-facing slice is local smart organizing, not content extraction.
It persists resumable item-scoped suggestion jobs, provides conservative confidence
bands and keyboard review, and reuses the crop editor in proposal-only mode. The
production start action stays closed until the real-image benchmark and the atomic
apply/revert implementation pass the Windows acceptance checklist.

Recognized prose, formulas, Markdown/LaTeX, handwriting removal, subject inference, and
automatic Problem creation remain out of this slice. Formal `Problem` and outbox rows
continue to originate only from the existing explicit capture commit.

Exit: normal and damaged legacy fixtures import without mutating their source hashes,
and the documented phone/desktop capture matrix passes on real hardware.
