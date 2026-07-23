# Windows capture acceptance

Use this checklist on a trusted home Wi-Fi or phone personal hotspot. Mobile capture is
ordinary local HTTP protected by a short-lived high-entropy session token; do not use it
on an untrusted shared network.

## One-time Windows authorization

1. Remove the app's firewall rule, open an unfinished batch, and click **手机扫码**.
   Confirm Windows requests administrator authorization exactly once.
2. Approve the request. Confirm a QR code appears immediately without another setup step.
   Close and reopen the app, click **手机扫码**, and confirm it proceeds without UAC.
3. Cancel UAC. Confirm no listener or QR code is created. Click **手机扫码** again and
   confirm authorization is requested again.
4. In Windows Defender Firewall, verify **Mistake Trainer Next - Mobile Capture** is an
   enabled inbound allow rule scoped to the exact current executable, TCP, `LocalSubnet`,
   Domain/Private/Public profiles, and edge traversal disabled. The app must not disable
   Windows Firewall, change the network profile, or edit rules belonging to other apps.
5. If the legacy **Mistake Trainer Next - Mobile Capture (Private)** rule exists, approve
   authorization and confirm that legacy app-owned rule is removed.

## Desktop batch and study-card organizer

1. Open **设置 → 常用科目**. Confirm the nine built-ins are `语文、数学、英语、政治、历史、
   地理、物理、化学、生物`; disable several, add a custom subject, disable drop sound, save,
   restart, and confirm the same profile restores the selection. Switch profile and confirm it
   receives its own default configuration.
2. Create a batch and import images from phone, file picker, clipboard, and Explorer drag.
   Restart the app and confirm the encrypted batch returns unchanged.
3. Apply alternating, split, and questions-only templates. Unequal remainder images must
   stay in **待配对图片** and reapplying a template must require confirmation.
4. In the organizer's top **整批科目** row, choose one configured subject. Confirm every card
   immediately uses it, previous per-card overrides are cleared, and the batch revision advances
   once. Change one card from its header subject selector and confirm only that card is overridden.
5. Confirm unassigned images use a large, contained gallery preview. One click toggles an
   image between ink-green **题面** and cinnabar **答案**. There must be no double-click
   shortcut or double-click-only action.
6. Drag a loose **题面** image: the lifted card must shrink slightly, use ink-green, and make
   the hovered card/fusion lane scale gently with the same color. Repeat with **答案** and confirm
   cinnabar feedback. Release and confirm a single 240 ms settle motion happens only after the
   saved revision arrives. With sound enabled, confirm one short local tone; disable sound and
   confirm silence without changing drag behavior.
7. Drag a loose image to the empty fusion lane. Confirm one new card is created and the
   image is attached in one saved transaction. Drag another image onto that existing card
   and confirm it merges using the loose image's persisted role.
8. Drag an image from a card back to **素材牌库** and confirm it becomes unassigned rather
   than deleted. Restart the app and confirm its role color, grouping, and order persist.
9. Create a card with two images initially marked **题面**. Confirm the card explicitly says
   **缺答案**, use **转为答案** on one assigned image, and confirm the card becomes ready when
   it has a subject. Repeat with a blank subject and confirm it explicitly says **缺科目**.
10. Click **撤销这张卡**, approve the confirmation, and confirm every linked image returns
   to **素材牌库** while no original or encrypted asset is deleted. Create a new card again
   and confirm it inherits the currently selected card's subject.
11. Open a card containing two question images and two answer images. Confirm the initial
   face is the question, clicking the card or the labelled flip action performs a 240 ms
   `rotateY(180deg)` flip, and the filmstrip can select every image.
12. Click **大图** and confirm mathematical text is readable. A derived preview from a
   source over 960 px must have a 960 px longest edge; encrypted originals remain intact.
13. Enable Windows **Reduce motion** and confirm flip/list/lift/settle motion and drop sound are
   removed without hiding content, colors, or controls.
14. Confirm every grouping or metadata mutation shows **保存中** then **已自动保存**. Give
   ready cards a subject and click **加入题库**. Incomplete cards remain in the inbox and an
   injected persistence failure creates zero formal problems and produces no settle sound.
15. Scroll a reference batch containing 150 images. Lazy preview loading and the 40-item
   memory cache must prevent obvious stalls or unbounded memory growth.

## Phone session

1. On iPhone Safari and Android Chrome, tap **开始连续拍照**, accept one photo, and confirm
   it appears in the queue immediately while the next camera sheet is requested without
   waiting for normalization or upload. If the browser blocks an automatic file-picker
   reopen, the fixed **拍下一张** action must remain visible above the finish bar and work
   while earlier photos upload. **结束连拍** must leave queued uploads running. Repeat with
   **相册多选**. During normalization or upload the summary must say **处理中**, the finish
   button must remain disabled, and no item may be marked complete before its own upload
   succeeds.
2. Upload an image whose Unicode filename is at least 200 characters. Confirm the filename
   is ellipsized, the item remains inside the viewport, and the page never gains horizontal
   scrolling. Force a long failure message and confirm it wraps inside the same row.
3. Change the batch subject, delete one upload, refresh the page, and confirm session state
   recovers. Interrupt Wi-Fi during upload and confirm retry is idempotent.
4. Select HEIC/HEIF on iPhone. Conversion must load lazily, produce JPEG without external
   requests, and leave a visible failed item if decoding fails.
5. Finish shooting and confirm the desktop batch enters organizing. Stop a second session
   from Windows and confirm the old page can no longer call the API; repeat after expiry.

## Limits and release gates

- Reject source uploads over 50 MB and normalized images over 25 MB, 12,000 px per edge,
  or 80 million pixels. A batch permits at most 150 images and 1 GB.
- Reject forged media types, damaged images, invalid tokens/origins, expired sessions, and
  cross-account/profile IDs without creating capture items.
- Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, Rust all-target tests, binding drift check,
  frontend build, and `pnpm tauri build` before release.
