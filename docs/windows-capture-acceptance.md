# Windows capture acceptance

Use this checklist on a trusted home Wi-Fi or a phone personal hotspot. Do not use a
public or shared network: the temporary collector is ordinary local HTTP. The app never
opens a listening port until its preflight passes. Firewall repair is an explicit button,
uses one Windows administrator confirmation, and creates an app-scoped TCP rule for
private local-subnet traffic only. It never enables public-network access.

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

1. With Windows currently on a public network, click **手机扫码**. Confirm no QR code or
   listener appears. Confirm the dialog contains four numbered steps, warns against
   changing a school/cafe network to Private, and contains no terminal commands.
2. Choose **Wi-Fi**, then click **打开 Wi-Fi 设置**. Confirm Windows opens its Wi-Fi page.
   Follow the in-app text: click the connected network or **Properties**, find **Network
   profile type**, and select **Private network**. Repeat with **网线 / 扩展坞** and
   confirm **打开以太网设置** opens the Ethernet page.
3. After a Settings page opens, leave the guide visible for up to 90 seconds. Confirm it
   displays automatic-detection status and advances to firewall repair without requiring
   **设置完成，立即检测**. Also confirm that button performs an immediate manual refresh.
4. Repeat with simultaneous Private and Public adapters. The conservative gate must
   still block QR generation, and the expanded troubleshooting section must suggest
   disconnecting unneeded VPN, virtual, company, or extra wired connections. On a managed
   school/work device where **Private network** is unavailable, verify the guide recommends
   an administrator, personal computer, or personal hotspot instead of bypass instructions.
5. If the app rule is absent or invalid, confirm **修复手机连接** is shown instead of a
   QR code. Cancel the Windows administrator prompt once: no listener should start and
   the repair button must remain available. Expand the administrator-help section and
   confirm it explains the password and cancellation cases. Click repair again, approve
   **Yes** in UAC, and verify the dialog advances automatically. In Windows
   Defender Firewall, verify the rule is bound to the current executable, inbound TCP,
   Private profile, `LocalSubnet`, and has edge traversal disabled. Public remains blocked.
6. Once both checks pass, select the private IPv4 address for the Wi-Fi or hotspot and
   generate the QR code. Restart the app and confirm the valid rule is recognized without
   another administrator prompt. If the executable path changes, the app must request an
   explicit repair instead of trusting the stale rule.
7. On iPhone Safari and Android Chrome, test **连续拍照** and **从相册批量选择**. Change
   the batch subject, delete one upload, refresh the page, and confirm state recovers.
8. Interrupt Wi-Fi during an upload, restore it, and confirm the item retries without
   duplicating. Uploads run at most two at a time and one failed item must not block the
   rest of the queue.
9. Select a HEIC/HEIF image on iPhone. Confirm conversion happens only after selection,
   uploads as JPEG, and no external network request is made. If conversion fails, the
   item must stay visibly failed rather than uploading damaged bytes. Apple users can
   switch Camera > Formats to **Most Compatible** as a fallback.
10. Click **完成拍摄** and confirm the desktop batch changes to organizing. Start another
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
