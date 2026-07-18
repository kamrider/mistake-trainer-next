use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use rusqlite::{Connection, backup::Backup};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::infrastructure::{
    assets::{decrypt_asset, plaintext_sha256},
    database::{DatabaseError, open_encrypted_database, open_encrypted_database_read_only},
};

const FORMAT_VERSION: i32 = 1;
const CURRENT_SCHEMA_VERSION: i64 = 6;
const DATABASE_FILE: &str = "library.db";
const MANIFEST_FILE: &str = "manifest.json";
const ASSETS_DIRECTORY: &str = "assets";
const MAX_DATABASE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 65 * 1024 * 1024;
const MAX_TOTAL_ASSET_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ASSETS: usize = 50_000;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub format_version: i32,
    pub created_at_utc_ms: f64,
    pub asset_count: i32,
    pub encrypted_bytes: f64,
    pub label: String,
    pub ready_for_restore: bool,
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("backup destination is invalid")]
    InvalidDestination,
    #[error("backup package is invalid")]
    InvalidPackage,
    #[error("backup belongs to another local account")]
    AccountMismatch,
    #[error("local library contains foreign account data")]
    ForeignAccountData,
    #[error("backup schema is unsupported")]
    UnsupportedSchema,
    #[error("backup exceeds the safety budget")]
    TooLarge,
    #[error("backup integrity check failed")]
    Integrity,
    #[error("local library is busy")]
    Lock,
    #[error("backup file operation failed")]
    Io(#[from] io::Error),
    #[error("backup database operation failed")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format_version: i32,
    created_at_utc_ms: i64,
    schema_version: i64,
    account_hash: String,
    database: ManifestFile,
    assets: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    relative_path: String,
    encrypted_bytes: u64,
    ciphertext_sha256: String,
}

pub fn create_backup(
    connection: &Mutex<Connection>,
    blob_root: &Path,
    database_key: &str,
    account_id: &str,
    destination: &Path,
    now_utc_ms: i64,
) -> Result<BackupSummary, BackupError> {
    let destination = destination
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    if !destination.is_dir() {
        return Err(BackupError::InvalidDestination);
    }
    let library_root = blob_root
        .parent()
        .ok_or(BackupError::InvalidDestination)?
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    if destination.starts_with(&library_root) {
        return Err(BackupError::InvalidDestination);
    }

    let suffix = Uuid::now_v7().simple().to_string();
    let label = format!("mistake-trainer-backup-{suffix}");
    let temporary = destination.join(format!(".{label}.tmp"));
    let final_path = destination.join(&label);
    fs::create_dir(&temporary)?;

    let result = (|| {
        fs::create_dir(temporary.join(ASSETS_DIRECTORY))?;
        let database_path = temporary.join(DATABASE_FILE);
        let (stored_assets, schema_version) = {
            let source = connection.lock().map_err(|_| BackupError::Lock)?;
            let source_schema_version: i64 =
                source.pragma_query_value(None, "user_version", |row| row.get(0))?;
            if !(1..=CURRENT_SCHEMA_VERSION).contains(&source_schema_version) {
                return Err(BackupError::UnsupportedSchema);
            }
            ensure_single_account(&source, account_id, source_schema_version)?;
            ensure_database_budget(&source)?;
            let asset_count: i64 = source.query_row(
                "SELECT COUNT(*) FROM assets WHERE account_id = ?1",
                [account_id],
                |row| row.get(0),
            )?;
            if usize::try_from(asset_count).map_or(true, |count| count > MAX_ASSETS) {
                return Err(BackupError::TooLarge);
            }
            let mut target = open_encrypted_database(&database_path, database_key)
                .map_err(map_target_database_error)?;
            {
                let backup = Backup::new(&source, &mut target)?;
                backup.run_to_completion(128, Duration::from_millis(5), None)?;
            }
            target.pragma_update(None, "journal_mode", "DELETE")?;
            let quick_check: String =
                target.pragma_query_value(None, "quick_check", |row| row.get(0))?;
            if quick_check != "ok" {
                return Err(BackupError::Integrity);
            }
            let schema_version: i64 =
                target.pragma_query_value(None, "user_version", |row| row.get(0))?;
            if schema_version != source_schema_version {
                return Err(BackupError::Integrity);
            }
            drop(target);

            let mut statement = source.prepare(
                "SELECT encrypted_path FROM assets WHERE account_id = ?1
                 ORDER BY encrypted_path LIMIT ?2",
            )?;
            let assets = statement
                .query_map(
                    rusqlite::params![account_id, i64::try_from(MAX_ASSETS).unwrap_or(i64::MAX)],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            (assets, schema_version)
        };

        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(BackupError::UnsupportedSchema);
        }
        if stored_assets.len() > MAX_ASSETS {
            return Err(BackupError::TooLarge);
        }

        let database = manifest_file_for_existing(
            &database_path,
            DATABASE_FILE.to_owned(),
            MAX_DATABASE_BYTES,
        )?;
        let canonical_blob_root = if stored_assets.is_empty() {
            None
        } else {
            Some(
                blob_root
                    .canonicalize()
                    .map_err(|_| BackupError::Integrity)?,
            )
        };
        let mut total_asset_bytes = 0_u64;
        let mut assets = Vec::with_capacity(stored_assets.len());
        let mut canonical_sources = HashSet::with_capacity(stored_assets.len());
        for stored_path in stored_assets {
            let relative = safe_relative_path(&stored_path)?;
            let root = canonical_blob_root.as_ref().ok_or(BackupError::Integrity)?;
            ensure_no_reparse_components(root, &relative)?;
            let source = root.join(&relative);
            let canonical_source = source.canonicalize().map_err(|_| BackupError::Integrity)?;
            if !canonical_source.starts_with(root) || !canonical_source.is_file() {
                return Err(BackupError::Integrity);
            }
            if !canonical_sources.insert(canonical_source.clone()) {
                return Err(BackupError::Integrity);
            }
            let output = temporary.join(ASSETS_DIRECTORY).join(&relative);
            let parent = output.parent().ok_or(BackupError::InvalidPackage)?;
            fs::create_dir_all(parent)?;
            let (encrypted_bytes, ciphertext_sha256) =
                copy_and_hash(&canonical_source, &output, MAX_ASSET_BYTES)?;
            total_asset_bytes = total_asset_bytes
                .checked_add(encrypted_bytes)
                .filter(|total| *total <= MAX_TOTAL_ASSET_BYTES)
                .ok_or(BackupError::TooLarge)?;
            assets.push(ManifestFile {
                relative_path: format!("{ASSETS_DIRECTORY}/{}", normalize_relative(&relative)),
                encrypted_bytes,
                ciphertext_sha256,
            });
        }

        let manifest = BackupManifest {
            format_version: FORMAT_VERSION,
            created_at_utc_ms: now_utc_ms,
            schema_version,
            account_hash: sha256_bytes(account_id.as_bytes()),
            database,
            assets,
        };
        write_manifest(&temporary.join(MANIFEST_FILE), &manifest)?;
        fs::rename(&temporary, &final_path)?;

        Ok(BackupSummary {
            format_version: FORMAT_VERSION,
            created_at_utc_ms: now_utc_ms as f64,
            asset_count: i32::try_from(manifest.assets.len()).unwrap_or(i32::MAX),
            encrypted_bytes: manifest
                .database
                .encrypted_bytes
                .saturating_add(total_asset_bytes) as f64,
            label,
            ready_for_restore: false,
        })
    })();

    if result.is_err() && temporary.parent() == Some(destination.as_path()) {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

pub fn validate_backup(
    source: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
) -> Result<BackupSummary, BackupError> {
    let source = source
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    if !source.is_dir() {
        return Err(BackupError::InvalidPackage);
    }
    let relative_manifest = safe_relative_path(MANIFEST_FILE)?;
    ensure_no_reparse_components(&source, &relative_manifest)?;
    let manifest_path = canonical_contained_file(&source, &relative_manifest)?;
    let manifest_bytes = read_bounded(&manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: BackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BackupError::InvalidPackage)?;
    if manifest.format_version != FORMAT_VERSION {
        return Err(BackupError::InvalidPackage);
    }
    if !(1..=CURRENT_SCHEMA_VERSION).contains(&manifest.schema_version) {
        return Err(BackupError::UnsupportedSchema);
    }
    if manifest.account_hash != sha256_bytes(account_id.as_bytes()) {
        return Err(BackupError::AccountMismatch);
    }
    if manifest.assets.len() > MAX_ASSETS {
        return Err(BackupError::TooLarge);
    }

    if manifest.database.relative_path != DATABASE_FILE {
        return Err(BackupError::InvalidPackage);
    }
    let label = source
        .file_name()
        .and_then(|name| name.to_str())
        .map(safe_label)
        .unwrap_or_else(|| "backup".to_owned());
    reject_sqlite_sidecars(&source)?;

    let validation_parent = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    let validation_directory = validation_parent.join(format!(
        ".mistake-trainer-validate-{}.tmp",
        Uuid::now_v7().simple()
    ));
    fs::create_dir(&validation_directory)?;
    let result = (|| {
        let relative_database = safe_relative_path(DATABASE_FILE)?;
        ensure_no_reparse_components(&source, &relative_database)?;
        let source_database = canonical_contained_file(&source, &relative_database)?;
        let staged_database = validation_directory.join(DATABASE_FILE);
        let (database_bytes, database_hash) =
            copy_and_hash(&source_database, &staged_database, MAX_DATABASE_BYTES)?;
        if database_bytes != manifest.database.encrypted_bytes
            || database_hash != manifest.database.ciphertext_sha256
        {
            return Err(BackupError::Integrity);
        }

        let database = open_encrypted_database_read_only(&staged_database, database_key)
            .map_err(|_| BackupError::Integrity)?;
        let journal_mode: String =
            database.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        let quick_check: String =
            database.pragma_query_value(None, "quick_check", |row| row.get(0))?;
        let schema_version: i64 =
            database.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let account_exists: i64 = database.query_row(
            "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE account_id = ?1)",
            [account_id],
            |row| row.get(0),
        )?;
        if !journal_mode.eq_ignore_ascii_case("delete")
            || quick_check != "ok"
            || schema_version != manifest.schema_version
            || account_exists != 1
        {
            return Err(BackupError::Integrity);
        }
        ensure_single_account(&database, account_id, schema_version)?;
        let database_asset_count: i64 = database.query_row(
            "SELECT COUNT(*) FROM assets WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        let database_asset_count =
            usize::try_from(database_asset_count).map_err(|_| BackupError::TooLarge)?;
        if database_asset_count > MAX_ASSETS {
            return Err(BackupError::TooLarge);
        }
        if database_asset_count != manifest.assets.len() {
            return Err(BackupError::Integrity);
        }
        let mut statement = database.prepare(
            "SELECT encrypted_path, plaintext_sha256, byte_length
             FROM assets WHERE account_id = ?1 ORDER BY encrypted_path LIMIT ?2",
        )?;
        let database_assets = statement
            .query_map(
                rusqlite::params![account_id, i64::try_from(MAX_ASSETS).unwrap_or(i64::MAX)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        (row.get::<_, String>(1)?, row.get::<_, i64>(2)?),
                    ))
                },
            )?
            .collect::<Result<HashMap<_, _>, _>>()?;
        drop(statement);
        drop(database);
        let mut manifest_assets = HashSet::with_capacity(manifest.assets.len());
        let mut canonical_assets = HashSet::with_capacity(manifest.assets.len());
        let mut total_asset_bytes = 0_u64;
        for asset in &manifest.assets {
            let relative = safe_relative_path(&asset.relative_path)?;
            if relative.components().next()
                != Some(Component::Normal(std::ffi::OsStr::new(ASSETS_DIRECTORY)))
            {
                return Err(BackupError::InvalidPackage);
            }
            let stored = relative
                .strip_prefix(ASSETS_DIRECTORY)
                .map_err(|_| BackupError::InvalidPackage)?;
            let stored = normalize_relative(stored);
            if !manifest_assets.insert(stored.clone()) {
                return Err(BackupError::InvalidPackage);
            }
            let (canonical_asset, encrypted) =
                read_verified_manifest_file(&source, asset, MAX_ASSET_BYTES)?;
            if !canonical_assets.insert(canonical_asset) {
                return Err(BackupError::InvalidPackage);
            }
            let (expected_plaintext_sha256, expected_plaintext_bytes) =
                database_assets.get(&stored).ok_or(BackupError::Integrity)?;
            let plaintext =
                decrypt_asset(&encrypted, asset_key).map_err(|_| BackupError::Integrity)?;
            if i64::try_from(plaintext.len()).ok() != Some(*expected_plaintext_bytes)
                || plaintext_sha256(&plaintext) != *expected_plaintext_sha256
            {
                return Err(BackupError::Integrity);
            }
            total_asset_bytes = total_asset_bytes
                .checked_add(asset.encrypted_bytes)
                .filter(|total| *total <= MAX_TOTAL_ASSET_BYTES)
                .ok_or(BackupError::TooLarge)?;
        }
        if manifest_assets != database_assets.keys().cloned().collect::<HashSet<_>>() {
            return Err(BackupError::Integrity);
        }

        Ok(BackupSummary {
            format_version: manifest.format_version,
            created_at_utc_ms: manifest.created_at_utc_ms as f64,
            asset_count: i32::try_from(manifest.assets.len()).unwrap_or(i32::MAX),
            encrypted_bytes: manifest
                .database
                .encrypted_bytes
                .saturating_add(total_asset_bytes) as f64,
            label,
            ready_for_restore: true,
        })
    })();
    if validation_directory.parent() == Some(validation_parent.as_path()) {
        let _ = fs::remove_dir_all(&validation_directory);
    }
    result
}

fn write_manifest(path: &Path, manifest: &BackupManifest) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(BackupError::TooLarge);
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn map_target_database_error(error: DatabaseError) -> BackupError {
    match error {
        DatabaseError::Sqlite(error) => BackupError::Database(error),
        DatabaseError::EmptyKey | DatabaseError::UnsupportedSchema(_) => {
            BackupError::InvalidDestination
        }
    }
}

fn ensure_database_budget(connection: &Connection) -> Result<(), BackupError> {
    let page_count = pragma_u64(connection, "page_count")?;
    let page_size = pragma_u64(connection, "page_size")?;
    let estimated_bytes = page_count
        .checked_mul(page_size)
        .ok_or(BackupError::TooLarge)?;
    if estimated_bytes > MAX_DATABASE_BYTES {
        return Err(BackupError::TooLarge);
    }
    Ok(())
}

fn pragma_u64(connection: &Connection, name: &str) -> Result<u64, BackupError> {
    let value: rusqlite::types::Value =
        connection.pragma_query_value(None, name, |row| row.get(0))?;
    match value {
        rusqlite::types::Value::Integer(value) => {
            u64::try_from(value).map_err(|_| BackupError::TooLarge)
        }
        rusqlite::types::Value::Text(value) => {
            value.parse::<u64>().map_err(|_| BackupError::TooLarge)
        }
        _ => Err(BackupError::TooLarge),
    }
}

fn ensure_single_account(
    connection: &Connection,
    account_id: &str,
    schema_version: i64,
) -> Result<(), BackupError> {
    let has_foreign_account: i64 = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM (
             SELECT account_id FROM learner_profiles
             UNION ALL SELECT account_id FROM problems
             UNION ALL SELECT account_id FROM assets
             UNION ALL SELECT account_id FROM review_events
             UNION ALL SELECT account_id FROM export_snapshots
             UNION ALL SELECT account_id FROM sync_operations
             UNION ALL SELECT account_id FROM sync_conflicts
             UNION ALL SELECT account_id FROM tombstones
           ) WHERE account_id <> ?1 LIMIT 1
         )",
        [account_id],
        |row| row.get(0),
    )?;
    if has_foreign_account != 0 {
        return Err(BackupError::ForeignAccountData);
    }
    let has_review_sessions = table_exists(connection, "review_sessions")?;
    if (schema_version == 1 && has_review_sessions) || (schema_version >= 2 && !has_review_sessions)
    {
        return Err(BackupError::Integrity);
    }
    if has_review_sessions {
        let has_foreign_session: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM review_sessions WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_session != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }

    let capture_tables = [
        "capture_batches",
        "capture_drafts",
        "capture_items",
        "capture_draft_items",
    ];
    let capture_table_count = capture_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 3 && capture_table_count != 0)
        || (schema_version >= 3 && capture_table_count != capture_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    if capture_table_count == capture_tables.len() {
        let has_foreign_capture_batch: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM capture_batches WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_capture_batch != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let has_profile_preferences = table_exists(connection, "profile_preferences")?;
    if (schema_version < 5 && has_profile_preferences)
        || (schema_version >= 5 && !has_profile_preferences)
    {
        return Err(BackupError::Integrity);
    }
    if has_profile_preferences {
        let has_foreign_preferences: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM profile_preferences WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_preferences != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let has_account_preferences = table_exists(connection, "account_preferences")?;
    if (schema_version < 6 && has_account_preferences)
        || (schema_version >= 6 && !has_account_preferences)
    {
        return Err(BackupError::Integrity);
    }
    if has_account_preferences {
        let invalid_account_preferences: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM account_preferences ap
               LEFT JOIN learner_profiles p ON p.id = ap.active_profile_id
               WHERE ap.account_id <> ?1
                  OR p.id IS NULL
                  OR p.account_id <> ap.account_id
               LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if invalid_account_preferences != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, BackupError> {
    let exists: i64 = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
         )",
        [table],
        |row| row.get(0),
    )?;
    Ok(exists != 0)
}

fn reject_sqlite_sidecars(root: &Path) -> Result<(), BackupError> {
    for name in ["library.db-wal", "library.db-shm", "library.db-journal"] {
        match fs::symlink_metadata(root.join(name)) {
            Ok(_) => return Err(BackupError::InvalidPackage),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(BackupError::Integrity),
        }
    }
    Ok(())
}

fn manifest_file_for_existing(
    path: &Path,
    relative_path: String,
    max_bytes: u64,
) -> Result<ManifestFile, BackupError> {
    let (encrypted_bytes, ciphertext_sha256) = hash_file(path, max_bytes)?;
    Ok(ManifestFile {
        relative_path,
        encrypted_bytes,
        ciphertext_sha256,
    })
}

fn read_verified_manifest_file(
    root: &Path,
    entry: &ManifestFile,
    max_bytes: u64,
) -> Result<(PathBuf, Vec<u8>), BackupError> {
    let relative = safe_relative_path(&entry.relative_path)?;
    ensure_no_reparse_components(root, &relative)?;
    let canonical = canonical_contained_file(root, &relative)?;
    let bytes = read_bounded(&canonical, max_bytes)?;
    if u64::try_from(bytes.len()).ok() != Some(entry.encrypted_bytes)
        || sha256_bytes(&bytes) != entry.ciphertext_sha256
    {
        return Err(BackupError::Integrity);
    }
    Ok((canonical, bytes))
}

fn canonical_contained_file(root: &Path, relative: &Path) -> Result<PathBuf, BackupError> {
    let canonical = root
        .join(relative)
        .canonicalize()
        .map_err(|_| BackupError::Integrity)?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(BackupError::Integrity);
    }
    Ok(canonical)
}

fn ensure_no_reparse_components(root: &Path, relative: &Path) -> Result<(), BackupError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BackupError::InvalidPackage);
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| BackupError::Integrity)?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err(BackupError::Integrity);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn copy_and_hash(
    source: &Path,
    destination: &Path,
    max_bytes: u64,
) -> Result<(u64, String), BackupError> {
    let mut input = fs::File::open(source)?;
    if input.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|value| *value <= max_bytes)
            .ok_or(BackupError::TooLarge)?;
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
    }
    output.sync_all()?;
    Ok((total, format!("{:x}", digest.finalize())))
}

fn hash_file(path: &Path, max_bytes: u64) -> Result<(u64, String), BackupError> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|value| *value <= max_bytes)
            .ok_or(BackupError::TooLarge)?;
        digest.update(&buffer[..read]);
    }
    Ok((total, format!("{:x}", digest.finalize())))
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BackupError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes {
        return Err(BackupError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(BackupError::TooLarge);
    }
    Ok(bytes)
}

fn safe_relative_path(value: &str) -> Result<PathBuf, BackupError> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || !path.components().all(|component| match component {
            Component::Normal(value) => safe_windows_component(value),
            _ => false,
        })
    {
        return Err(BackupError::InvalidPackage);
    }
    Ok(path.to_path_buf())
}

fn safe_windows_component(value: &std::ffi::OsStr) -> bool {
    let Some(value) = value.to_str() else {
        return false;
    };
    if value.is_empty()
        || value.contains(':')
        || value.ends_with('.')
        || value.ends_with(' ')
        || value.chars().any(char::is_control)
    {
        return false;
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(|character| character == '.' || character == ' ')
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !matches!(
            stem.as_str(),
            "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        )
        && !matches!(
            stem.as_str(),
            "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
        )
}

fn normalize_relative(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn safe_label(value: &str) -> String {
    let label = value
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>();
    if label.trim().is_empty() {
        "backup".to_owned()
    } else {
        label
    }
}
