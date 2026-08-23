# Privacy and data handling notice

Mistake Trainer Next is offline-first. The core capture, library, review, reporting, export, and
backup workflows operate on the user's Windows device without requiring an account.

## Data stored on the device

The application can store an encrypted library database, encrypted question and answer assets,
review history, capture drafts, profile preferences, backup metadata, and fixed diagnostic state.
Database content is protected with SQLCipher and asset files are encrypted separately. Optional
account refresh credentials are kept in the Windows credential store rather than in the library.

## Network activity

Network access is limited by feature and build configuration:

- a production build can check the public GitHub updater manifest no more than once per 24 hours;
- a user can explicitly start a temporary LAN phone-capture session on a selected local address;
- optional OCR model installation performs a user-initiated HTTPS download and verifies the
  pinned files before use;
- optional cloud authentication and synchronization occur only in a build with a configured
  backend and after the user selects that functionality.

Ordinary local builds without updater or cloud configuration do not silently enable those
services. Local OCR text is used transiently for layout analysis and is not stored in the
database, UI state, logs, events, or diagnostic report.

## Diagnostics and support

The Safe diagnostic report contains fixed system compatibility facts and aggregate counts. It is
designed not to contain question content, answers, names, tags, filenames, local paths, account
identifiers, tokens, keys, panic text, or stack traces. Export is user initiated and the user
chooses where to save the file.

Do not post private learning content or credentials in public support issues. Follow `SUPPORT.md`
and use the private process in `SECURITY.md` for vulnerabilities.

## Deletion, uninstall, and backups

Profile deletion and an explicitly confirmed fresh start remove application-owned data within
their validated scope. Uninstalling the application is intentionally designed to preserve the
encrypted library so that an accidental uninstall is not data loss. Users should create and
verify a backup before deleting data, moving storage, upgrading, or uninstalling.

This repository notice documents the application's technical behavior. A distributor offering
the application commercially must add its legal identity, jurisdiction-specific disclosures,
and privacy contact before claiming complete legal compliance.
