# Trusted Device Security Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give users an accurate view of this Windows device's cloud and offline-unlock protection without pretending that a Supabase session list can remotely destroy encryption keys already stored on another computer.

**Architecture:** Ship the feature in two security boundaries. The v1 boundary exposes only the current device, uses Supabase's local sign-out scope, reports whether the current Windows account can unlock the encrypted library, and keeps immediate local locking as the destructive action. Cross-device revocation is a later key-management feature: every installation must own a device key pair, receive a separately wrapped library-key envelope, prove possession when syncing, and stop receiving usable envelopes after revocation.

**Tech Stack:** Rust stable, Tauri 2.11, Windows Credential Manager, SQLCipher, Supabase Auth/Postgres/RLS, Vue 3, TypeScript, tauri-specta, Vitest, Testing Library.

## Security invariants

- “退出这台电脑” calls `/auth/v1/logout?scope=local`; it must never use Supabase's default global scope.
- Local refresh credentials are cleared even if the remote logout request is offline or times out.
- Raw device UUIDs, refresh tokens, database keys, asset keys, credential names, and filesystem paths never enter Vue state.
- The current Windows account may unlock only while its credential envelope is present and valid.
- A remote device action must not claim to revoke offline unlock until encrypted library keys are wrapped per device and are no longer stored as permanently reusable account-independent secrets.
- Revocation is fail-closed for future cloud access but cannot truthfully promise erasure of data already decrypted or copied by a compromised machine.
- All security copy distinguishes “lock this app now,” “sign out this cloud session,” and “revoke a different device.”

---

### Task 1: Scope desktop sign-out to the current Supabase session

**Files:**
- Modify: `src-tauri/src/infrastructure/supabase.rs`
- Modify: `src-tauri/tests/supabase_client.rs`
- Modify: `src/app/LibraryLockDialog.vue`
- Modify: `src/app/LibraryLockDialog.test.ts`

- [x] Add a protocol test that captures the exact logout request and expects `/auth/v1/logout?scope=local`.
- [x] Verify the test fails against the previous unscoped endpoint.
- [x] Add the local scope and document why the default global behavior is unsafe here.
- [x] Tell the user that other devices stay signed in and that offline cleanup applies to this computer.
- [x] Run the focused Rust and Vue regression suites.

---

### Task 2: Expose a redacted current-device security status

**Files:**
- Create: `src-tauri/src/commands/device_security.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src-tauri/src/infrastructure/runtime.rs`
- Modify: `src/shared/api/bindings.ts` through generation
- Create: `src-tauri/tests/device_security.rs`

**Interface:**

```rust
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CurrentDeviceSecurityStatus {
    pub platform_label: String,
    pub trusted_windows_account: bool,
    pub offline_unlock_available: bool,
    pub library_locked: bool,
    pub cloud_session: CurrentCloudSessionState,
}

current_device_security_status() -> AppResult<CurrentDeviceSecurityStatus>
```

- [ ] Add failing tests for unlocked, locked, missing-envelope, and credential-read-failure states.
- [ ] Derive the status from the existing lock marker, credential envelope, and in-memory auth state.
- [ ] Return a fixed platform label and booleans only; do not return the credential-store device UUID.
- [ ] Register the command, regenerate bindings, and run binding drift checks.

---

### Task 3: Replace the roadmap promise with a real current-device panel

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [ ] Replace “可信设备（未来）” with a live “当前 Windows 设备” panel.
- [ ] Show three explicit rows: encrypted local library, offline unlock by current Windows account, and current cloud-session state.
- [ ] Keep “立即锁定资料库” and “退出这台电脑并锁定” as separate actions with distinct confirmation copy.
- [ ] Explain that managing other computers requires a future device-key upgrade; do not render a disabled fake revoke button.
- [ ] Restore focus after dialogs, guard duplicate actions, and announce status changes through an `aria-live` region.
- [ ] Animate status changes with opacity/transform only, up to 240 ms, and remove motion under `prefers-reduced-motion`.
- [ ] Add keyboard, failure, cancellation, duplicate-click, and reduced-motion tests.

---

### Task 4: Record useful device activity without creating a tracking surface

**Files:**
- Add: `src-tauri/migrations/0014_local_device_activity.sql`
- Modify: `src-tauri/src/modules/auth_sync.rs`
- Modify: `src-tauri/src/modules/sync.rs`
- Modify: related Rust integration tests

**Data:**

```sql
CREATE TABLE local_device_activity (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  last_cloud_sign_in_at_utc_ms INTEGER,
  last_successful_sync_at_utc_ms INTEGER,
  last_cloud_sign_out_at_utc_ms INTEGER
);
```

- [ ] Store only local timestamps; do not store IP address, network name, hostname, Windows username, or hardware fingerprint.
- [ ] Update sign-in, successful-sync, and sign-out timestamps in their committed success paths.
- [ ] Expose the timestamps through the redacted status DTO with strict finite/range validation in Vue.
- [ ] Include the table in encrypted backup/restore validation.
- [ ] Add v13-to-v14 migration preservation tests and malformed-timestamp UI tests.

---

### Task 5: Design real cross-device revocation before exposing it

**Files:**
- Add: `supabase/migrations/0006_device_key_envelopes.sql`
- Create: `docs/architecture/device-key-envelopes.md`
- Create: contract and RLS tests under the existing Supabase test structure

**Cloud entities:**

```text
trusted_devices
  id, account_id, public_key, label, created_at, last_seen_at, revoked_at

library_key_envelopes
  account_id, device_id, key_version, wrapped_library_key, created_at, revoked_at
```

- [ ] Choose a Windows-backed non-exportable device private key mechanism and document recovery behavior.
- [ ] Wrap the library master key independently for each approved device.
- [ ] Require signed device proof for envelope download and sync authorization.
- [ ] Revoke future sync and envelope access atomically; keep an auditable tombstone.
- [ ] Add RLS tests for cross-account access, revoked devices, replay, forged device IDs, and envelope substitution.
- [ ] Define lost-all-devices recovery before implementation; recovery must not silently weaken encryption.
- [ ] Only after these tests pass, add an “其他设备” list and remote revoke UI.

---

### Task 6: Release acceptance

- [ ] `pnpm lint`
- [ ] `pnpm typecheck`
- [ ] `pnpm test`
- [ ] `pnpm bindings:check`
- [ ] `cargo fmt --check`
- [ ] `cargo test --all-targets`
- [ ] Supabase RLS and contract tests
- [ ] Windows keyboard and screen-reader smoke test
- [ ] Offline sign-out test: this device locks; other device remains signed in.
- [ ] Global-sign-out regression test: no desktop path calls unscoped `/auth/v1/logout`.
- [ ] `pnpm tauri build`

## Decision

The v1 UI will call this feature **“当前设备保护”**, not **“可信设备管理.”** It can accurately show and control the machine in front of the user. A multi-device list and remote offline-unlock revocation remain blocked on per-device key envelopes and signed device authorization; Supabase session metadata alone is insufficient.
