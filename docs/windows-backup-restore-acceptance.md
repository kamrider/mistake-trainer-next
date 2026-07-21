# Windows encrypted backup restore acceptance

Use a disposable test library. Version 1 restores only on the same trusted Windows account and
device because the SQLCipher and asset keys remain in Windows secure credential storage.

## Normal restore

1. Add a uniquely named profile, one formal problem, and one unfinished capture batch. Create an
   encrypted backup in **Settings → Backup and restore**.
2. Change or delete those records after the backup. Choose **Select backup and prepare restore**,
   select the package directory, and confirm the candidate summary contains no local path.
3. Confirm the current library is still unchanged. Open **Review risks and confirm restore**;
   the final button must remain disabled until the acknowledgement checkbox is selected.
4. Confirm restore. Any active phone-capture listener must stop, the app must restart, and the
   backed-up profile, problem, encrypted asset, and unfinished capture batch must return.
5. Confirm the global **Library restored successfully** notice appears once and can be dismissed.
   Close and reopen the app; the consumed notice must not appear again.

## Cancel and validation failures

1. Cancel the native folder picker. No candidate card, restart, or data mutation should occur.
2. Prepare a valid candidate, close the confirmation dialog with Escape and by clicking Cancel,
   and confirm the current library remains usable.
3. Modify one byte in a copied test package asset, remove a manifest entry, and try a package from
   another local account. Each case must fail without changing the current database or assets and
   without displaying an internal path.
4. Fill the destination disk during preparation. Only the new app-owned temporary candidate may
   be cleaned; the selected backup package and live library hashes must remain unchanged.

## Interrupted restart and rollback

For each boundary below, force-close the process and relaunch it. The pending marker is bounded and
the directory names must match the exact app-owned UUID prefixes.

1. After confirmation but before the live directory is renamed: the next launch should perform
   the restore normally.
2. After live → rollback but before candidate → live: the next launch should finish the candidate
   move, validate startup, and then remove only the verified rollback directory.
3. After candidate → live but before runtime initialization: the next launch should validate the
   restored root and continue.
4. Corrupt the restored database or asset after step 3 while retaining the rollback directory.
   The next launch must replace it with the exact old library before opening SQLCipher and show
   **Automatically returned to the original library**.
5. Repeat with an injected runtime initialization failure. The original profile and assets must
   be byte-for-byte available and no restore candidate may be reported as successful.

## Release gates

- Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `cargo test --all-targets`, binding drift check,
  frontend build, and `pnpm tauri build`.
- Verify no restore DTO, error message, diagnostic payload, or frontend state contains a selected
  backup path, application-data path, database key, asset key, account ID, or marker contents.
