use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use rusqlite::{Connection, backup::Backup};
use sha2::{Digest, Sha256};

use crate::infrastructure::{
    assets::{decrypt_asset, plaintext_sha256},
    database::{open_encrypted_database, run_migrations},
};

use super::{
    ASSETS_DIRECTORY, COPY_BUFFER_BYTES, DATABASE_FILE, MAX_ASSET_BYTES, MAX_ASSETS,
    MAX_DATABASE_BYTES, MAX_TOTAL_ASSET_BYTES, StorageMigrationError, StorageMigrationSource,
    collect_relative_files, ensure_no_link_or_reparse_ancestor, ensure_no_reparse_components,
    ensure_safe_directory, ensure_safe_regular_file, hash_file, is_windows_reparse_point,
    normalize_relative, safe_relative_asset_path, validate_owner,
};

#[derive(Clone, Debug)]
struct AssetRecord {
    encrypted_path: String,
    plaintext_sha256: String,
    byte_length: i64,
}

pub(super) fn stage_library_snapshot(
    connection: &Connection,
    source: &StorageMigrationSource,
    stage_library_root: &Path,
    stage_product_root: &Path,
    migration_id: &str,
) -> Result<(usize, u64), StorageMigrationError> {
    validate_account_boundary(connection, source.account_id())?;
    ensure_database_budget(connection)?;
    create_database_snapshot(
        connection,
        &stage_library_root.join(DATABASE_FILE),
        source.database_key(),
    )?;
    let assets = query_assets(connection, source.account_id())?;
    let copied_bytes = copy_referenced_assets(&source.blob_root, stage_library_root, &assets)?;
    validate_library_tree(
        stage_library_root,
        source.database_key(),
        &source.asset_key,
        source.account_id(),
        Some(stage_product_root),
        Some(migration_id),
    )?;
    Ok((assets.len(), copied_bytes))
}

pub(super) fn storage_usage_bytes(
    source: &StorageMigrationSource,
) -> Result<(u64, u64), StorageMigrationError> {
    let library_root = source
        .blob_root
        .parent()
        .ok_or(StorageMigrationError::InvalidDestination)?;
    ensure_no_link_or_reparse_ancestor(library_root)?;
    ensure_safe_directory(library_root)?;
    let database_path = library_root.join(DATABASE_FILE);
    ensure_safe_regular_file(&database_path)?;
    let database_bytes = fs::metadata(database_path)?.len();
    if database_bytes > MAX_DATABASE_BYTES {
        return Err(StorageMigrationError::TooLarge);
    }

    let assets = {
        let connection = source
            .connection
            .lock()
            .map_err(|_| StorageMigrationError::Lock)?;
        validate_account_boundary(&connection, source.account_id())?;
        query_assets(&connection, source.account_id())?
    };
    let canonical_blob_root = source
        .blob_root
        .canonicalize()
        .map_err(|_| StorageMigrationError::Integrity)?;
    ensure_safe_directory(&canonical_blob_root)?;
    let mut seen = HashSet::with_capacity(assets.len());
    let mut asset_bytes = 0_u64;
    for asset in assets {
        let relative = safe_relative_asset_path(&asset.encrypted_path)?;
        let normalized = normalize_relative(&relative);
        if !seen.insert(normalized) {
            return Err(StorageMigrationError::Integrity);
        }
        ensure_no_reparse_components(&canonical_blob_root, &relative)?;
        let path = canonical_blob_root.join(relative);
        ensure_safe_regular_file(&path)?;
        let bytes = fs::metadata(path)?.len();
        if bytes > MAX_ASSET_BYTES {
            return Err(StorageMigrationError::TooLarge);
        }
        asset_bytes = asset_bytes
            .checked_add(bytes)
            .filter(|value| *value <= MAX_TOTAL_ASSET_BYTES)
            .ok_or(StorageMigrationError::TooLarge)?;
    }
    Ok((database_bytes, asset_bytes))
}

fn create_database_snapshot(
    source: &Connection,
    destination: &Path,
    database_key: &str,
) -> Result<(), StorageMigrationError> {
    let mut target = open_encrypted_database(destination, database_key)?;
    {
        let backup = Backup::new(source, &mut target)?;
        backup.run_to_completion(128, Duration::from_millis(5), None)?;
    }
    target.pragma_update(None, "journal_mode", "DELETE")?;
    let quick_check: String = target.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(StorageMigrationError::Integrity);
    }
    target.close().map_err(|(_, error)| error)?;
    ensure_safe_regular_file(destination)?;
    if fs::metadata(destination)?.len() > MAX_DATABASE_BYTES {
        return Err(StorageMigrationError::TooLarge);
    }
    Ok(())
}

fn query_assets(
    connection: &Connection,
    account_id: &str,
) -> Result<Vec<AssetRecord>, StorageMigrationError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM assets WHERE account_id = ?1",
        [account_id],
        |row| row.get(0),
    )?;
    let count = usize::try_from(count).map_err(|_| StorageMigrationError::TooLarge)?;
    if count > MAX_ASSETS {
        return Err(StorageMigrationError::TooLarge);
    }
    let mut statement = connection.prepare(
        "SELECT encrypted_path, plaintext_sha256, byte_length
         FROM assets WHERE account_id = ?1 ORDER BY encrypted_path LIMIT ?2",
    )?;
    let records = statement
        .query_map(
            rusqlite::params![account_id, i64::try_from(MAX_ASSETS).unwrap_or(i64::MAX)],
            |row| {
                Ok(AssetRecord {
                    encrypted_path: row.get(0)?,
                    plaintext_sha256: row.get(1)?,
                    byte_length: row.get(2)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != count {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(records)
}

fn copy_referenced_assets(
    source_blob_root: &Path,
    destination_library_root: &Path,
    assets: &[AssetRecord],
) -> Result<u64, StorageMigrationError> {
    if assets.is_empty() {
        return Ok(0);
    }
    ensure_no_link_or_reparse_ancestor(source_blob_root)?;
    ensure_safe_directory(source_blob_root)?;
    let canonical_blob_root = source_blob_root
        .canonicalize()
        .map_err(|_| StorageMigrationError::Integrity)?;
    let destination_blob_root = destination_library_root.join(ASSETS_DIRECTORY);
    let mut seen_paths = HashSet::with_capacity(assets.len());
    let mut seen_sources = HashSet::with_capacity(assets.len());
    let mut total = 0_u64;

    for asset in assets {
        let relative = safe_relative_asset_path(&asset.encrypted_path)?;
        let normalized = normalize_relative(&relative);
        if !seen_paths.insert(normalized) {
            return Err(StorageMigrationError::Integrity);
        }
        ensure_no_reparse_components(&canonical_blob_root, &relative)?;
        let source = canonical_blob_root.join(&relative);
        let canonical_source = source
            .canonicalize()
            .map_err(|_| StorageMigrationError::Integrity)?;
        if !canonical_source.starts_with(&canonical_blob_root)
            || !seen_sources.insert(canonical_source.clone())
        {
            return Err(StorageMigrationError::Integrity);
        }
        ensure_safe_regular_file(&canonical_source)?;
        let destination = destination_blob_root.join(&relative);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or(StorageMigrationError::InvalidDestination)?,
        )?;
        let copied = copy_and_verify(&canonical_source, &destination)?;
        total = total
            .checked_add(copied)
            .filter(|value| *value <= MAX_TOTAL_ASSET_BYTES)
            .ok_or(StorageMigrationError::TooLarge)?;
    }
    Ok(total)
}

fn copy_and_verify(source: &Path, destination: &Path) -> Result<u64, StorageMigrationError> {
    let source_metadata = fs::symlink_metadata(source)?;
    if !source_metadata.is_file()
        || source_metadata.file_type().is_symlink()
        || is_windows_reparse_point(&source_metadata)
        || source_metadata.len() > MAX_ASSET_BYTES
    {
        return Err(StorageMigrationError::Integrity);
    }
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut source_hash = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(count).map_err(|_| StorageMigrationError::TooLarge)?)
            .filter(|value| *value <= MAX_ASSET_BYTES)
            .ok_or(StorageMigrationError::TooLarge)?;
        source_hash.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
    }
    output.sync_all()?;
    drop(output);

    let (destination_bytes, destination_hash) = hash_file(destination, MAX_ASSET_BYTES)?;
    if copied != source_metadata.len()
        || copied != destination_bytes
        || source_hash.finalize().as_slice() != destination_hash.as_slice()
    {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(copied)
}

pub(super) fn validate_library_tree(
    library_root: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    product_root: Option<&Path>,
    migration_id: Option<&str>,
) -> Result<(), StorageMigrationError> {
    ensure_no_link_or_reparse_ancestor(library_root)?;
    ensure_safe_directory(library_root)?;
    let database_path = library_root.join(DATABASE_FILE);
    ensure_safe_regular_file(&database_path)?;
    if fs::metadata(&database_path)?.len() > MAX_DATABASE_BYTES {
        return Err(StorageMigrationError::TooLarge);
    }
    if let (Some(product_root), Some(migration_id)) = (product_root, migration_id) {
        validate_owner(product_root, migration_id, account_id)?;
    }

    let mut database = open_encrypted_database(&database_path, database_key)?;
    run_migrations(&mut database)?;
    let quick_check: String = database.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(StorageMigrationError::Integrity);
    }
    validate_account_boundary(&database, account_id)?;
    let assets = query_assets(&database, account_id)?;
    drop(database);

    let blob_root = library_root.join(ASSETS_DIRECTORY);
    ensure_safe_directory(&blob_root)?;
    let actual_files = collect_relative_files(&blob_root)?;
    let expected_files = assets
        .iter()
        .map(|asset| {
            safe_relative_asset_path(&asset.encrypted_path).map(|path| normalize_relative(&path))
        })
        .collect::<Result<HashSet<_>, _>>()?;
    if actual_files != expected_files {
        return Err(StorageMigrationError::Integrity);
    }

    for asset in assets {
        let relative = safe_relative_asset_path(&asset.encrypted_path)?;
        ensure_no_reparse_components(&blob_root, &relative)?;
        let path = blob_root.join(relative);
        ensure_safe_regular_file(&path)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_ASSET_BYTES {
            return Err(StorageMigrationError::TooLarge);
        }
        let encrypted = fs::read(path)?;
        let plaintext = decrypt_asset(&encrypted, asset_key)?;
        if i64::try_from(plaintext.len()).ok() != Some(asset.byte_length)
            || plaintext_sha256(&plaintext) != asset.plaintext_sha256
        {
            return Err(StorageMigrationError::Integrity);
        }
    }
    Ok(())
}

fn validate_account_boundary(
    connection: &Connection,
    account_id: &str,
) -> Result<(), StorageMigrationError> {
    const ACCOUNT_TABLES: &[&str] = &[
        "learner_profiles",
        "problems",
        "assets",
        "review_events",
        "export_snapshots",
        "sync_operations",
        "sync_conflicts",
        "tombstones",
        "review_sessions",
        "capture_batches",
        "profile_preferences",
        "account_preferences",
        "legacy_imports",
        "cloud_sync_state",
        "asset_derivations",
        "sync_entity_snapshots",
    ];
    for table in ACCOUNT_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE account_id != ?1 OR account_id = ''");
        let foreign: i64 = connection.query_row(&sql, [account_id], |row| row.get(0))?;
        if foreign != 0 {
            return Err(StorageMigrationError::Integrity);
        }
    }
    let own_profiles: i64 = connection.query_row(
        "SELECT COUNT(*) FROM learner_profiles WHERE account_id = ?1",
        [account_id],
        |row| row.get(0),
    )?;
    if own_profiles == 0 {
        return Err(StorageMigrationError::Integrity);
    }
    Ok(())
}

fn ensure_database_budget(connection: &Connection) -> Result<(), StorageMigrationError> {
    let page_count = pragma_u64(connection, "page_count")?;
    let page_size = pragma_u64(connection, "page_size")?;
    let estimated = page_count
        .checked_mul(page_size)
        .ok_or(StorageMigrationError::TooLarge)?;
    if estimated > MAX_DATABASE_BYTES {
        return Err(StorageMigrationError::TooLarge);
    }
    Ok(())
}

fn pragma_u64(connection: &Connection, name: &str) -> Result<u64, StorageMigrationError> {
    let value: rusqlite::types::Value =
        connection.pragma_query_value(None, name, |row| row.get(0))?;
    match value {
        rusqlite::types::Value::Integer(value) => {
            u64::try_from(value).map_err(|_| StorageMigrationError::TooLarge)
        }
        rusqlite::types::Value::Text(value) => value
            .parse::<u64>()
            .map_err(|_| StorageMigrationError::TooLarge),
        _ => Err(StorageMigrationError::TooLarge),
    }
}
