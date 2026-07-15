# Windows capture acceptance

Use this checklist on a trusted home Wi-Fi or a phone personal hotspot. Do not use a
public or shared network: the temporary collector is ordinary local HTTP. The app never
opens Windows Firewall automatically; if Windows asks, permit private networks only.

## Desktop batch

1. Start the Tauri app, choose a learner profile, open **采集整理**, and create a batch.
2. Import several images with **电脑批量选择**, paste from the clipboard, and drag files
   from Explorer. Restart the app and confirm the batch and assignments return.
3. Apply `题答交替`, `前题后答`, and `仅题图` to separate test batches. Confirm unequal
   remainders stay unassigned and reapplying a template requires confirmation.
4. Drag images between question, answer, and unassigned areas; then repeat the same
   moves with the keyboard actions. Confirm new phone uploads do not replace this work.
5. Give ready drafts a subject and commit them. Confirm incomplete drafts remain and a
   deliberately injected storage failure produces zero formal problems.

## Phone session

1. Put the phone and Windows computer on the same trusted private network. Click
   **手机扫码**. If multiple private IPv4 addresses appear, choose the address for that
   Wi-Fi or hotspot, then scan the QR code.
2. On iPhone Safari and Android Chrome, test **连续拍照** and **从相册批量选择**. Change
   the batch subject, delete one upload, refresh the page, and confirm state recovers.
3. Interrupt Wi-Fi during an upload, restore it, and confirm the item retries without
   duplicating. Uploads run at most two at a time and one failed item must not block the
   rest of the queue.
4. Select a HEIC/HEIF image on iPhone. Confirm conversion happens only after selection,
   uploads as JPEG, and no external network request is made. If conversion fails, the
   item must stay visibly failed rather than uploading damaged bytes. Apple users can
   switch Camera > Formats to **Most Compatible** as a fallback.
5. Click **完成拍摄** and confirm the desktop batch changes to organizing. Start another
   session, stop it from Windows, and confirm the old page can no longer call the API.
   Repeat after 30 minutes idle to confirm expiry.

## Limits and safety

- A source upload over 50 MB is rejected. Mobile normalization uses a 4096-pixel longest
  edge and the server rejects normalized content over 25 MB, 12,000 pixels per edge, or
  80 million pixels.
- A batch allows at most 150 images and 1 GB of encrypted temporary data. Verify a full
  reference batch survives restart, then discard it and confirm only orphan assets are
  removed.
- JPEG, PNG, and WebP are accepted after decode validation; forged content types,
  malformed files, wrong tokens, wrong origins, expired sessions, and cross-profile IDs
  must fail without creating a capture item.
- Finish by running `pnpm lint`, `pnpm typecheck`, `pnpm test`, the Rust all-target test
  suite, the binding drift check, and `pnpm tauri build`.
