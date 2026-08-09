use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

use uuid::Uuid;

use crate::infrastructure::{
    assets::{decrypt_asset, plaintext_sha256},
    database::open_encrypted_database_read_only,
};

use super::{
    ASSETS_DIRECTORY, BackupError, BackupManifest, BackupSummary, CURRENT_SCHEMA_VERSION,
    DATABASE_FILE, FORMAT_VERSION, MANIFEST_FILE, MAX_ASSET_BYTES, MAX_ASSETS, MAX_DATABASE_BYTES,
    MAX_MANIFEST_BYTES, MAX_TOTAL_ASSET_BYTES,
    backup_package_repository::{
        canonical_contained_file, copy_and_hash, ensure_no_reparse_components, normalize_relative,
        read_bounded, read_verified_manifest_file, reject_sqlite_sidecars, safe_label,
        safe_relative_path, sha256_bytes,
    },
    backup_schema_validation::ensure_single_account,
};

#[derive(Debug)]
struct ValidationWorkspace {
    parent: PathBuf,
    path: PathBuf,
}

impl ValidationWorkspace {
    fn create(parent: PathBuf) -> Result<Self, BackupError> {
        let path = parent.join(format!(
            ".mistake-trainer-validate-{}.tmp",
            Uuid::now_v7().simple()
        ));
        fs::create_dir(&path)?;
        Ok(Self { parent, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ValidationWorkspace {
    fn drop(&mut self) {
        if self.path.parent() == Some(self.parent.as_path()) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
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
    let workspace = ValidationWorkspace::create(validation_parent)?;
    let relative_database = safe_relative_path(DATABASE_FILE)?;
    ensure_no_reparse_components(&source, &relative_database)?;
    let source_database = canonical_contained_file(&source, &relative_database)?;
    let staged_database = workspace.path().join(DATABASE_FILE);
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
    let quick_check: String = database.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    let foreign_key_violation: i64 = database.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check LIMIT 1)",
        [],
        |row| row.get(0),
    )?;
    let schema_version: i64 =
        database.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let account_exists: i64 = database.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE account_id = ?1)",
        [account_id],
        |row| row.get(0),
    )?;
    if !journal_mode.eq_ignore_ascii_case("delete")
        || quick_check != "ok"
        || foreign_key_violation != 0
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
        let plaintext = decrypt_asset(&encrypted, asset_key).map_err(|_| BackupError::Integrity)?;
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
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::ValidationWorkspace;

    #[test]
    fn dropping_validation_workspace_removes_only_its_owned_directory() {
        let temporary = tempdir().expect("temporary parent");
        let parent = temporary.path().canonicalize().expect("canonical parent");
        let sentinel = parent.join("keep.txt");
        fs::write(&sentinel, b"keep").expect("write sentinel");
        let workspace = ValidationWorkspace::create(parent).expect("create workspace");
        let workspace_path = workspace.path().to_owned();
        let nested = workspace.path().join("nested");
        fs::create_dir(&nested).expect("create nested directory");
        fs::write(nested.join("library.db"), b"encrypted").expect("write nested file");

        drop(workspace);

        assert!(!workspace_path.exists());
        assert_eq!(fs::read(sentinel).expect("read sentinel"), b"keep");
    }

    #[test]
    fn validation_workspaces_use_distinct_private_names() {
        let temporary = tempdir().expect("temporary parent");
        let parent = temporary.path().canonicalize().expect("canonical parent");
        let first = ValidationWorkspace::create(parent.clone()).expect("first workspace");
        let second = ValidationWorkspace::create(parent).expect("second workspace");
        let first_name = first
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("first private name");
        let second_name = second
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("second private name");

        assert!(first_name.starts_with(".mistake-trainer-validate-"));
        assert!(first_name.ends_with(".tmp"));
        assert!(second_name.starts_with(".mistake-trainer-validate-"));
        assert!(second_name.ends_with(".tmp"));
        assert_ne!(first_name, second_name);
    }
}
