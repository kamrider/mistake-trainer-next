# Security policy

## Supported versions

Security fixes are provided for the newest published release. The published `v0.1.4` artifacts
predate the Rust dependency remediation prepared for `v0.1.5` and must not be used for a new
production rollout. Upgrade to `v0.1.5` after its immutable CI, release verification, and
acceptance gates complete.

## Report a vulnerability privately

Use the repository's **Security → Report a vulnerability** flow. Do not open a public issue for
a suspected vulnerability and do not include real student work, encrypted libraries, recovery
keys, access tokens, updater private keys, local paths, or account identifiers.

Include only the minimum safe reproduction information:

- affected application version and Windows version/architecture;
- the security boundary that was crossed;
- a synthetic reproduction or fixed public error code;
- whether local data, LAN capture, cloud sync, updates, or backup/restore are involved.

The project will validate the report, rotate or revoke exposed credentials when necessary, and
coordinate a fixed release before public disclosure. No response-time SLA is promised by this
repository.

## Release security gates

Every immutable release candidate must pass:

- full-history credential scanning;
- complete pnpm lockfile audit, including build and test tooling;
- RustSec audit of `src-tauri/Cargo.lock`;
- strict Rust lint and the complete test suite;
- installer and updater signature/checksum verification;
- privacy-safe diagnostic and startup-failure contracts.

The free Windows installer is intentionally not Authenticode-signed. This is a distribution
trust limitation, not an exemption from Tauri updater signature verification.
