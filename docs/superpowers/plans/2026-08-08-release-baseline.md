# Windows 0.1.0-rc.1 Release Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce one reviewable `0.1.0-rc.1` Windows release-candidate baseline whose exact commit passes repository gates, builds an installable x64 package locally, and is ready for a protected GitHub signed-release run.

**Architecture:** Freeze the current feature surface and treat the accumulated commercial-quality work as one release candidate. First close the interrupted legacy filesystem extraction, then establish a versioned release ledger and run every deterministic local gate against the exact snapshot; only after that snapshot is committed may it be pushed and tagged for the protected x64/ARM64 signing workflow.

**Tech Stack:** Vue 3, TypeScript 5.9, Vitest 4, Rust 1.97, Tauri 2.11, PowerShell, GitHub Actions, NSIS

## Global Constraints

- Release-candidate version is exactly `0.1.0-rc.1` in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Do not add product features, dependencies, telemetry, cloud requirements, or automatic update endpoints to ordinary builds.
- Keep capture recognition review-required; do not claim unverified automatic OCR accuracy.
- Do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`; inspect and test its existing working-tree change without rewriting it.
- Preserve encrypted library, backup, restore, legacy import, sync, and updater safety behavior.
- Do not place certificates, passwords, updater private keys, tokens, account identifiers, user content, or local private paths in Git.
- A local unsigned installer is release-candidate evidence only. Public paid release remains blocked until the protected GitHub workflow produces valid signed x64 and ARM64 artifacts and a human completes the required manual matrix.

---

### Task 1: Close the interrupted legacy filesystem boundary

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`
- Modify mechanically: `src-tauri/src/modules/legacy_scan.rs`
- Modify mechanically: `src-tauri/src/modules/legacy_scan_filesystem.rs`
- Modify: `docs/superpowers/plans/2026-08-08-legacy-scan-filesystem-boundary.md`

**Interfaces:**
- Consumes: `legacy_scan_filesystem::{legacy_tree_fingerprint, MAX_ASSET_BYTES, is_safe_relative_path, read_bounded}`.
- Produces: a boundary contract that rejects actual SQL mutation statements without rejecting `sha2::Digest::update`.

- [x] **Step 1: Narrow the false-positive SQL mutation contract**

Replace the final SQL portion of the filesystem-child rejection pattern with statement-shaped SQL tokens:

```powershell
'(?im)serde|serde_json|LegacyScanReport|LegacyIssue|OffsetDateTime|rusqlite|^\s*(?:INSERT\s+INTO|UPDATE\s+\S+\s+SET|DELETE\s+FROM)\b'
```

- [x] **Step 2: Format the extracted Rust files**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml
```

Expected: exit code `0`, with only rustfmt layout changes.

- [x] **Step 3: Verify the boundary and focused behavior**

Run:

```powershell
pnpm exec vitest run tests/legacy-scan-filesystem-boundary.test.ts
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: ownership contract passes, PowerShell contract prints `Rust architecture boundary contract passed.`, and all 16 legacy tests pass.

- [x] **Step 4: Mark the interrupted plan complete**

Change Steps 7 and 8 in `docs/superpowers/plans/2026-08-08-legacy-scan-filesystem-boundary.md` from unchecked to checked only after the full gates in Task 3 pass.

### Task 2: Version and document the release candidate

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Create: `CHANGELOG.md`
- Create: `docs/releases/0.1.0-rc.1.md`

**Interfaces:**
- Consumes: the semantic-version equality checks in `scripts/windows-release-contract.ps1` and `scripts/windows-signed-release.ps1`.
- Produces: release version `0.1.0-rc.1` and an auditable candidate ledger.

- [x] **Step 1: Set the exact release-candidate version**

Change each first-party manifest version from `0.1.0` to:

```text
0.1.0-rc.1
```

Run `cargo metadata --manifest-path src-tauri\Cargo.toml --no-deps --format-version 1` so Cargo refreshes the first-party lockfile entry.

- [x] **Step 2: Add a customer-readable changelog**

Create `CHANGELOG.md` with an `0.1.0-rc.1` section dated `2026-08-08`. State that this candidate includes offline-first encrypted storage, capture/import/review, backup/restore, optional cloud sync, safe diagnostics, Windows installer hardening, and review-required local question splitting. State that signed x64/ARM64 artifacts and real-device acceptance are release promotion requirements.

- [x] **Step 3: Add the release evidence ledger**

Create `docs/releases/0.1.0-rc.1.md` with these fixed sections:

```markdown
# 0.1.0-rc.1 release evidence

## Candidate identity
## Deterministic repository gates
## Local Windows package evidence
## Protected GitHub release evidence
## Manual acceptance matrix
## Known limitations
## Promotion decision
```

Use `PASS`, `FAIL`, `PENDING`, or `NOT APPLICABLE` for every item. Record command names and artifact hashes, never secrets or private paths.

- [x] **Step 4: Verify version contracts**

Run:

```powershell
.\scripts\windows-release-contract.ps1
.\scripts\windows-signed-release.ps1 -ReleaseTag v0.1.0-rc.1 -Architecture x64
```

Expected: the ordinary release contract passes. The signed-release command may stop only at missing protected credentials; it must not report tag/version drift.

### Task 3: Prove the deterministic quality baseline

**Files:**
- Modify with results: `docs/releases/0.1.0-rc.1.md`
- Regenerate only if required: `src/shared/api/bindings.ts`

**Interfaces:**
- Consumes: repository scripts and CI-equivalent commands.
- Produces: a green deterministic gate ledger tied to the candidate snapshot.

- [x] **Step 1: Run frontend static and contract gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm mobile:vendor:check
pnpm contract:rust-boundaries
.\scripts\github-actions-pin-contract.ps1
.\scripts\windows-release-contract.ps1
```

Expected: every command exits `0` with no warnings promoted to failures.

- [x] **Step 2: Verify generated bindings without relying on a dirty HEAD diff**

Hash `src/shared/api/bindings.ts`, run `pnpm bindings:generate`, and hash it again.

Expected: both SHA-256 hashes are identical.

- [x] **Step 3: Run frontend tests and production build**

Run:

```powershell
pnpm test:coverage
pnpm build
```

Expected: all tests pass, coverage thresholds pass, and Vite creates `dist`.

- [x] **Step 4: Run Rust static and regression gates**

Run:

```powershell
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

Expected: every command exits `0`; environment-dependent real-corpus/runtime tests remain explicitly ignored.

- [x] **Step 5: Run repository hygiene and secret checks**

Run:

```powershell
git diff --check
git status --short
rg -n --hidden -g '!node_modules' -g '!src-tauri/target' -g '!.git' '(BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY|TAURI_SIGNING_PRIVATE_KEY\s*=|WINDOWS_CERTIFICATE_PASSWORD\s*=|SUPABASE_SERVICE_ROLE_KEY\s*=|ghp_[A-Za-z0-9]{20,})'
```

Expected: whitespace check exits `0`; secret scan returns no tracked secret material.

### Task 4: Build and inspect the local Windows candidate

**Files:**
- Modify with results: `docs/releases/0.1.0-rc.1.md`
- Generated artifact: `src-tauri/target/release/bundle/nsis/Mistake Trainer Next_0.1.0-rc.1_x64-setup.exe`

**Interfaces:**
- Consumes: Tauri ordinary configuration with updater networking disabled.
- Produces: an unsigned x64 installation candidate, SHA-256 evidence, and installed-product smoke evidence.

- [x] **Step 1: Build the x64 NSIS package**

Run:

```powershell
pnpm tauri build
```

Expected: Tauri produces the versioned x64 NSIS installer.

- [x] **Step 2: Run installed-product smoke**

Run:

```powershell
.\scripts\windows-installer-smoke.ps1
```

Expected: install, runtime/product self-check, WebView2 GUI launch, ten-second keepalive, single-instance handoff, clean shutdown, and uninstall all pass.

- [x] **Step 3: Record artifact identity**

Run `Get-FileHash -Algorithm SHA256` and `Get-AuthenticodeSignature` for the installer. Record the hash and expected local `NotSigned` status in the ledger; never describe this local file as the paid production installer.

### Task 5: Create the Git baseline and connect GitHub

**Files:**
- Modify with final commit identity: `docs/releases/0.1.0-rc.1.md`

**Interfaces:**
- Consumes: a deterministic green snapshot and local package evidence.
- Produces: one reviewable branch commit, a remote branch, and—after protected settings exist—annotated tag `v0.1.0-rc.1` plus a draft signed release.

- [x] **Step 1: Review the exact commit set**

Inspect `git diff --stat`, untracked files, generated resources, and existing `recognition_visual_split.rs` changes. Exclude build output, local corpora, credentials, diagnostic reports, and machine-specific files. Do not discard unrelated user work.

- [ ] **Step 2: Commit the verified baseline**

Stage only the reviewed source, tests, documentation, workflows, licenses, and release metadata. Commit with:

```text
chore(release): prepare 0.1.0-rc.1 baseline
```

- [ ] **Step 3: Re-run HEAD-relative checks**

Run `pnpm bindings:check`, `git diff --check HEAD`, and `git status --short`.

Expected: bindings match committed generated output, no whitespace failure, and no unexplained source changes remain.

- [x] **Step 4: Inspect GitHub connectivity**

Run `git remote -v` and `gh auth status`. If the repository has no intended GitHub remote or authentication is unavailable, ask the user for the account/repository decision at this point.

- [ ] **Step 5: Push the candidate branch**

Push `codex/windows-runtime-resilience` to its GitHub upstream. Do not force-push.

- [ ] **Step 6: Run protected signed release after user configuration**

After the `production` environment contains every secret and variable listed in `docs/windows-release-runbook.md`, merge the reviewed commit to `main`, create annotated tag `v0.1.0-rc.1`, push the tag, and wait for `Signed Windows Release` to create the draft release. Record signed artifact hashes, signature publisher, workflow URL, and manual-matrix results before promotion.

## Self-Review

- Spec coverage: the plan produces a release-candidate baseline, deterministic gates, local installer evidence, a commit, GitHub synchronization, and an explicit credential handoff for protected signing.
- Placeholder scan: every implementation or verification action names its exact files, commands, expected state, and allowed stopping condition.
- Type consistency: the exact version and tag are `0.1.0-rc.1` and `v0.1.0-rc.1`; local ordinary builds remain unsigned and update-network-free, while protected builds own Authenticode and updater signing.
