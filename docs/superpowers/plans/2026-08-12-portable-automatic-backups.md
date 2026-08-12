# Portable Automatic Backups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cryptographically portable credential envelope to backups and a bounded automatic-backup policy without weakening the existing encrypted-at-rest model.

**Architecture:** Encrypt the database key, asset key, and account id with a randomly generated 256-bit recovery key using AES-256-GCM. Store only the authenticated envelope beside the existing ciphertext manifest; show the recovery key once and never persist it locally. Keep automatic backups on the existing device-key path first, with an explicit destination, due interval, retention count, and safe owned-directory pruning.

**Tech Stack:** Rust aes-gcm/getrandom/base64/serde, Tauri Specta bindings, Vue 3, Vitest, rusqlite.

## Global Constraints

- Recovery keys are generated from the operating-system random source and contain at least 256 bits of entropy.
- Recovery keys, database keys, asset keys, and raw account identifiers never enter logs, diagnostics, manifests, or test snapshots.
- Automatic retention may delete only validated directories created under the configured destination with the owned backup label prefix.
- Restoring portable credentials into another device remains disabled until credential replacement is crash-transactional across keyring and startup restore.

---

### Task 1: Portable credential envelope core

**Files:**
- Create: `src-tauri/src/modules/backup_portability.rs`
- Modify: `src-tauri/src/modules/backup.rs`
- Modify: `src-tauri/src/modules/backup_creation.rs`
- Test: `src-tauri/tests/backup_portability.rs`

**Interfaces:**
- Produces: `PortableBackupReceipt { summary, recovery_key }`, `create_portable_backup(...)`, and `open_portable_credentials(...)`.

- [x] **Step 1: Write tamper, wrong-key, round-trip, and secrecy tests**

```rust
let receipt = create_portable_backup(/* existing runtime credentials */)?;
let opened = open_portable_credentials(&package, &receipt.recovery_key)?;
assert_eq!(opened.database_key, DATABASE_KEY);
assert!(!fs::read_to_string(package.join("recovery-envelope.json"))?.contains(DATABASE_KEY));
assert!(open_portable_credentials(&package, OTHER_KEY).is_err());
```

- [x] **Step 2: Implement authenticated envelope creation**

```rust
let cipher = Aes256Gcm::new_from_slice(&recovery_key)?;
let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), Payload {
    msg: &serde_json::to_vec(&credentials)?,
    aad: manifest_sha256.as_bytes(),
})?;
```

- [x] **Step 3: Verify core backup tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test backup_portability --test backup_store`
Expected: PASS.

### Task 2: Portable backup command and explicit UI

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/app/components/SettingsBackupPanel.vue`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/components/SettingsBackupPanel.test.ts`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Produces: `backup_create_portable` command; a one-time recovery-key receipt with copy confirmation and cross-device limitation copy.

- [x] **Step 1: Add failing UI tests for explicit creation and one-time recovery-key rendering**

```ts
await userEvent.click(screen.getByRole('button', { name: '创建便携加密备份' }))
expect(await screen.findByText(/恢复密钥只显示这一次/)).toBeVisible()
expect(screen.queryByText(/跨设备恢复已经完成/)).not.toBeInTheDocument()
```

- [x] **Step 2: Expose the command and wire the settings operation**

```rust
pub async fn backup_create_portable(state: State<'_, LibraryRuntime>)
  -> Result<AppResult<Option<PortableBackupReceipt>>, ()>;
```

- [x] **Step 3: Generate bindings and run focused UI tests**

Run: `pnpm bindings:generate`
Expected: `backupCreatePortable` and `PortableBackupReceipt` appear in generated bindings.

Run: `pnpm test -- src/app/components/SettingsBackupPanel.test.ts src/app/views/SettingsView.test.ts`
Expected: PASS.

### Task 3: Automatic backup policy boundary

**Files:**
- Create: `src-tauri/src/modules/automatic_backup.rs`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/commands/backup.rs`
- Test: `src-tauri/tests/automatic_backup.rs`

**Interfaces:**
- Produces: `AutomaticBackupPolicy { enabled, interval_days, retention_count, destination }`, due calculation, and owned-package retention pruning.

- [x] **Step 1: Write due-time and safe-retention tests**

```rust
assert!(backup_is_due(last_success, now, 7));
assert_eq!(owned_packages_to_prune(root, 3)?.len(), 2);
assert!(foreign_directory.exists());
```

- [x] **Step 2: Implement bounded policy validation**

```rust
if !(1..=30).contains(&interval_days) || !(1..=20).contains(&retention_count) {
    return Err(AutomaticBackupError::InvalidPolicy);
}
```

- [x] **Step 3: Run automatic-backup and schema tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test automatic_backup --test database_schema`
Expected: PASS.
