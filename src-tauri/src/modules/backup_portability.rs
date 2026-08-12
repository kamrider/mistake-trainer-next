use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::infrastructure::{
    assets::{decrypt_asset, encrypt_asset},
    database::open_encrypted_database,
};

use super::{
    ASSETS_DIRECTORY, BackupError, BackupManifest, BackupRestoreCandidate, BackupSummary,
    DATABASE_FILE, MANIFEST_FILE, MAX_ASSET_BYTES, MAX_DATABASE_BYTES, MAX_MANIFEST_BYTES,
    backup_package_repository::{
        canonical_contained_file, copy_and_hash, ensure_no_reparse_components,
        manifest_file_for_existing, read_bounded, read_verified_manifest_file, safe_relative_path,
        sha256_bytes, write_new_synced,
    },
    create_backup, prepare_backup_restore, validate_backup,
};

const ENVELOPE_FILE: &str = "recovery-envelope.json";
const ENVELOPE_SCHEMA_VERSION: u32 = 1;
const ENVELOPE_ALGORITHM: &str = "AES-256-GCM";
const MAX_ENVELOPE_BYTES: u64 = 16 * 1024;
const AAD_PREFIX: &[u8] = b"mistake-trainer-portable-backup-v1:";

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PortableBackupReceipt {
    pub summary: BackupSummary,
    pub recovery_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortableCredentials {
    pub database_key: String,
    pub asset_key: [u8; 32],
    pub account_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryEnvelope {
    schema_version: u32,
    algorithm: String,
    manifest_sha256: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EncodedCredentials {
    database_key: String,
    asset_key: String,
    account_id: String,
}

pub fn create_portable_backup(
    connection: &Mutex<Connection>,
    blob_root: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    destination: &Path,
    now_utc_ms: i64,
) -> Result<PortableBackupReceipt, BackupError> {
    let summary = create_backup(
        connection,
        blob_root,
        database_key,
        account_id,
        destination,
        now_utc_ms,
    )?;
    let package = destination.join(&summary.label);
    let credentials = PortableCredentials {
        database_key: database_key.to_owned(),
        asset_key: *asset_key,
        account_id: account_id.to_owned(),
    };
    let recovery_key = match write_portable_envelope(&package, &credentials) {
        Ok(key) => key,
        Err(error) => {
            remove_incomplete_portable_package(destination, &package, &summary.label);
            return Err(error);
        }
    };
    Ok(PortableBackupReceipt {
        summary,
        recovery_key,
    })
}

pub fn open_portable_credentials(
    package: &Path,
    recovery_key: &str,
) -> Result<PortableCredentials, BackupError> {
    let package = package
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    if !package.is_dir() {
        return Err(BackupError::InvalidPackage);
    }
    let manifest = read_bounded(&package.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let manifest_sha256 = sha256_bytes(&manifest);
    let envelope_bytes = read_bounded(&package.join(ENVELOPE_FILE), MAX_ENVELOPE_BYTES)?;
    let envelope: RecoveryEnvelope =
        serde_json::from_slice(&envelope_bytes).map_err(|_| BackupError::InvalidPackage)?;
    if envelope.schema_version != ENVELOPE_SCHEMA_VERSION
        || envelope.algorithm != ENVELOPE_ALGORITHM
        || envelope.manifest_sha256 != manifest_sha256
    {
        return Err(BackupError::InvalidPackage);
    }
    let key = decode_fixed::<32>(recovery_key).ok_or(BackupError::InvalidRecoveryKey)?;
    let nonce = decode_fixed::<12>(&envelope.nonce).ok_or(BackupError::InvalidPackage)?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(envelope.ciphertext)
        .map_err(|_| BackupError::InvalidPackage)?;
    if ciphertext.len() > usize::try_from(MAX_ENVELOPE_BYTES).unwrap_or(usize::MAX) {
        return Err(BackupError::TooLarge);
    }
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| BackupError::InvalidRecoveryKey)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &associated_data(&manifest_sha256),
            },
        )
        .map_err(|_| BackupError::InvalidRecoveryKey)?;
    let encoded: EncodedCredentials =
        serde_json::from_slice(&plaintext).map_err(|_| BackupError::InvalidPackage)?;
    let asset_key = decode_hex_key(&encoded.asset_key).ok_or(BackupError::InvalidPackage)?;
    if decode_hex_key(&encoded.database_key).is_none()
        || uuid::Uuid::parse_str(&encoded.account_id).is_err()
    {
        return Err(BackupError::InvalidPackage);
    }
    Ok(PortableCredentials {
        database_key: encoded.database_key,
        asset_key,
        account_id: encoded.account_id,
    })
}

/// Prepares a portable backup for restoration on this device without changing
/// the current library. The source database and assets remain encrypted on
/// disk while a short-lived package is re-keyed to the current device before
/// entering the normal restore-candidate pipeline.
pub fn prepare_portable_backup_restore(
    source: &Path,
    application_root: &Path,
    recovery_key: &str,
    target_database_key: &str,
    target_asset_key: &[u8; 32],
    target_account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupRestoreCandidate, BackupError> {
    let credentials = open_portable_credentials(source, recovery_key)?;
    validate_backup(
        source,
        &credentials.database_key,
        &credentials.asset_key,
        &credentials.account_id,
    )?;

    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    if !application_root.is_dir() {
        return Err(BackupError::InvalidDestination);
    }
    let source = source
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    let workspace = PortableConversionWorkspace::create(&application_root)?;
    convert_package(
        &source,
        workspace.path(),
        &credentials,
        target_database_key,
        target_asset_key,
        target_account_id,
    )?;
    prepare_backup_restore(
        workspace.path(),
        &application_root,
        target_database_key,
        target_asset_key,
        target_account_id,
        now_utc_ms,
    )
}

struct PortableConversionWorkspace {
    parent: PathBuf,
    path: PathBuf,
}

impl PortableConversionWorkspace {
    fn create(parent: &Path) -> Result<Self, BackupError> {
        let path = parent.join(format!(
            ".mistake-trainer-portable-convert-{}.tmp",
            uuid::Uuid::now_v7().simple()
        ));
        fs::create_dir(&path)?;
        Ok(Self {
            parent: parent.to_owned(),
            path,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PortableConversionWorkspace {
    fn drop(&mut self) {
        if self.path.parent() == Some(self.parent.as_path()) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn convert_package(
    source: &Path,
    destination: &Path,
    source_credentials: &PortableCredentials,
    target_database_key: &str,
    target_asset_key: &[u8; 32],
    target_account_id: &str,
) -> Result<(), BackupError> {
    uuid::Uuid::parse_str(target_account_id).map_err(|_| BackupError::InvalidPackage)?;
    if decode_hex_key(target_database_key).is_none() {
        return Err(BackupError::InvalidPackage);
    }
    let manifest_bytes = read_bounded(&source.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let source_manifest: BackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BackupError::InvalidPackage)?;

    fs::create_dir(destination.join(ASSETS_DIRECTORY))?;
    let relative_database = safe_relative_path(DATABASE_FILE)?;
    ensure_no_reparse_components(source, &relative_database)?;
    let source_database = canonical_contained_file(source, &relative_database)?;
    let target_database = destination.join(DATABASE_FILE);
    let (database_bytes, database_hash) =
        copy_and_hash(&source_database, &target_database, MAX_DATABASE_BYTES)?;
    if database_bytes != source_manifest.database.encrypted_bytes
        || database_hash != source_manifest.database.ciphertext_sha256
    {
        return Err(BackupError::Integrity);
    }
    rekey_database(
        &target_database,
        &source_credentials.database_key,
        &source_credentials.account_id,
        target_database_key,
        target_account_id,
    )?;

    let mut assets = Vec::with_capacity(source_manifest.assets.len());
    for entry in &source_manifest.assets {
        let (_, encrypted) = read_verified_manifest_file(source, entry, MAX_ASSET_BYTES)?;
        let plaintext = decrypt_asset(&encrypted, &source_credentials.asset_key)
            .map_err(|_| BackupError::Integrity)?;
        let reencrypted =
            encrypt_asset(&plaintext, target_asset_key).map_err(|_| BackupError::Crypto)?;
        if u64::try_from(reencrypted.len()).unwrap_or(u64::MAX) > MAX_ASSET_BYTES {
            return Err(BackupError::TooLarge);
        }
        let relative = safe_relative_path(&entry.relative_path)?;
        let output = destination.join(&relative);
        fs::create_dir_all(output.parent().ok_or(BackupError::InvalidPackage)?)?;
        write_new_synced(&output, &reencrypted)?;
        assets.push(manifest_file_for_existing(
            &output,
            entry.relative_path.clone(),
            MAX_ASSET_BYTES,
        )?);
    }

    let manifest = BackupManifest {
        format_version: source_manifest.format_version,
        created_at_utc_ms: source_manifest.created_at_utc_ms,
        schema_version: source_manifest.schema_version,
        account_hash: sha256_bytes(target_account_id.as_bytes()),
        database: manifest_file_for_existing(
            &target_database,
            DATABASE_FILE.to_owned(),
            MAX_DATABASE_BYTES,
        )?,
        assets,
    };
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(manifest.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(BackupError::TooLarge);
    }
    write_new_synced(&destination.join(MANIFEST_FILE), &manifest)
}

fn rekey_database(
    database_path: &Path,
    source_database_key: &str,
    source_account_id: &str,
    target_database_key: &str,
    target_account_id: &str,
) -> Result<(), BackupError> {
    let mut database = open_encrypted_database(database_path, source_database_key)
        .map_err(|_| BackupError::Integrity)?;
    database.pragma_update(None, "foreign_keys", "OFF")?;
    let tables = account_scoped_tables(&database)?;
    let transaction = database.transaction()?;
    for table in tables {
        let quoted = table.replace('"', "\"\"");
        transaction.execute(
            &format!("UPDATE \"{quoted}\" SET account_id = ?1 WHERE account_id = ?2"),
            rusqlite::params![target_account_id, source_account_id],
        )?;
    }
    transaction.commit()?;
    database.pragma_update(None, "journal_mode", "DELETE")?;
    database.pragma_update(None, "rekey", target_database_key)?;
    let quick_check: String = database.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(BackupError::Integrity);
    }
    Ok(())
}

fn account_scoped_tables(database: &Connection) -> Result<Vec<String>, BackupError> {
    let mut statement = database.prepare(
        "SELECT m.name
         FROM sqlite_master AS m
         WHERE m.type = 'table'
           AND m.name NOT LIKE 'sqlite_%'
           AND EXISTS (
             SELECT 1 FROM pragma_table_info(m.name) WHERE name = 'account_id'
           )
         ORDER BY m.name",
    )?;
    Ok(statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn write_portable_envelope(
    package: &Path,
    credentials: &PortableCredentials,
) -> Result<String, BackupError> {
    let manifest = read_bounded(&package.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let manifest_sha256 = sha256_bytes(&manifest);
    let mut recovery_key = [0_u8; 32];
    let mut nonce = [0_u8; 12];
    getrandom::fill(&mut recovery_key).map_err(|_| BackupError::Crypto)?;
    getrandom::fill(&mut nonce).map_err(|_| BackupError::Crypto)?;
    let encoded = EncodedCredentials {
        database_key: credentials.database_key.clone(),
        asset_key: encode_hex(&credentials.asset_key),
        account_id: credentials.account_id.clone(),
    };
    let plaintext = serde_json::to_vec(&encoded).map_err(|_| BackupError::InvalidPackage)?;
    let cipher = Aes256Gcm::new_from_slice(&recovery_key).map_err(|_| BackupError::Crypto)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &associated_data(&manifest_sha256),
            },
        )
        .map_err(|_| BackupError::Crypto)?;
    let envelope = RecoveryEnvelope {
        schema_version: ENVELOPE_SCHEMA_VERSION,
        algorithm: ENVELOPE_ALGORITHM.to_owned(),
        manifest_sha256,
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    };
    let bytes = serde_json::to_vec_pretty(&envelope).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_ENVELOPE_BYTES {
        return Err(BackupError::TooLarge);
    }
    write_new_synced(&package.join(ENVELOPE_FILE), &bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(recovery_key))
}

fn associated_data(manifest_sha256: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + manifest_sha256.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(manifest_sha256.as_bytes());
    aad
}

fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    let decoded = URL_SAFE_NO_PAD.decode(value).ok()?;
    decoded.try_into().ok()
}

fn decode_hex_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(key)
}

fn encode_hex(value: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in value {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn remove_incomplete_portable_package(destination: &Path, package: &Path, label: &str) {
    if label.starts_with("mistake-trainer-backup-") && package.parent() == Some(destination) {
        let _ = fs::remove_dir_all(package);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATABASE_KEY: &str = "0101010101010101010101010101010101010101010101010101010101010101";
    const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";

    fn fixture() -> (tempfile::TempDir, PortableCredentials, String) {
        let root = tempfile::tempdir().expect("portable package");
        fs::write(root.path().join(MANIFEST_FILE), br#"{"formatVersion":1}"#).expect("manifest");
        let credentials = PortableCredentials {
            database_key: DATABASE_KEY.to_owned(),
            asset_key: [7_u8; 32],
            account_id: ACCOUNT_ID.to_owned(),
        };
        let recovery_key =
            write_portable_envelope(root.path(), &credentials).expect("write envelope");
        (root, credentials, recovery_key)
    }

    #[test]
    fn portable_credentials_round_trip_without_plaintext_in_the_envelope() {
        let (root, credentials, recovery_key) = fixture();

        let opened = open_portable_credentials(root.path(), &recovery_key).expect("open envelope");
        let envelope = fs::read_to_string(root.path().join(ENVELOPE_FILE)).expect("envelope");

        assert_eq!(opened, credentials);
        assert!(!envelope.contains(DATABASE_KEY));
        assert!(!envelope.contains(ACCOUNT_ID));
        assert!(!envelope.contains(&encode_hex(&[7_u8; 32])));
    }

    #[test]
    fn wrong_key_and_manifest_or_envelope_tampering_are_rejected() {
        let (root, _credentials, recovery_key) = fixture();
        let wrong_key = URL_SAFE_NO_PAD.encode([9_u8; 32]);
        assert!(matches!(
            open_portable_credentials(root.path(), &wrong_key),
            Err(BackupError::InvalidRecoveryKey)
        ));

        fs::write(root.path().join(MANIFEST_FILE), b"changed").expect("tamper manifest");
        assert!(matches!(
            open_portable_credentials(root.path(), &recovery_key),
            Err(BackupError::InvalidPackage)
        ));

        let (root, _credentials, recovery_key) = fixture();
        let envelope_path = root.path().join(ENVELOPE_FILE);
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(&envelope_path).expect("envelope")).expect("json");
        envelope["ciphertext"] = serde_json::json!(URL_SAFE_NO_PAD.encode([1_u8; 32]));
        fs::write(&envelope_path, serde_json::to_vec(&envelope).expect("json"))
            .expect("tamper envelope");
        assert!(matches!(
            open_portable_credentials(root.path(), &recovery_key),
            Err(BackupError::InvalidRecoveryKey)
        ));
    }

    #[test]
    fn malformed_recovery_keys_are_rejected_without_reading_secrets() {
        let (root, _credentials, _recovery_key) = fixture();
        for key in ["", "not-base64", &URL_SAFE_NO_PAD.encode([1_u8; 31])] {
            assert!(matches!(
                open_portable_credentials(root.path(), key),
                Err(BackupError::InvalidRecoveryKey)
            ));
        }
    }
}
