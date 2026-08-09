use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use rusqlite::{Connection, backup::Backup};
use uuid::Uuid;

use crate::infrastructure::database::{DatabaseError, open_encrypted_database};

use super::{
    ASSETS_DIRECTORY, BackupError, BackupManifest, BackupSummary, CURRENT_SCHEMA_VERSION,
    DATABASE_FILE, FORMAT_VERSION, MANIFEST_FILE, MAX_ASSET_BYTES, MAX_ASSETS, MAX_DATABASE_BYTES,
    MAX_MANIFEST_BYTES, MAX_TOTAL_ASSET_BYTES, ManifestFile,
    backup_package_repository::{
        copy_and_hash, ensure_no_reparse_components, manifest_file_for_existing,
        normalize_relative, safe_relative_path, sha256_bytes, write_new_synced,
    },
    backup_schema_validation::{ensure_database_budget, ensure_single_account},
};

#[derive(Debug)]
struct StagedBackupPackage {
    temporary_path: PathBuf,
    final_path: PathBuf,
    published: bool,
}

impl StagedBackupPackage {
    fn create(destination: &Path, label: &str) -> Result<Self, BackupError> {
        let temporary_path = destination.join(format!(".{label}.tmp"));
        let final_path = destination.join(label);
        fs::create_dir(&temporary_path)?;
        Ok(Self {
            temporary_path,
            final_path,
            published: false,
        })
    }

    fn path(&self) -> &Path {
        &self.temporary_path
    }

    fn publish(&mut self) -> Result<(), BackupError> {
        match fs::symlink_metadata(&self.final_path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "backup package already exists",
                )
                .into());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::rename(&self.temporary_path, &self.final_path)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagedBackupPackage {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.temporary_path);
        }
    }
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
    let mut package = StagedBackupPackage::create(&destination, &label)?;
    fs::create_dir(package.path().join(ASSETS_DIRECTORY))?;
    let database_path = package.path().join(DATABASE_FILE);
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

    let database =
        manifest_file_for_existing(&database_path, DATABASE_FILE.to_owned(), MAX_DATABASE_BYTES)?;
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
        let output = package.path().join(ASSETS_DIRECTORY).join(&relative);
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
    write_manifest(&package.path().join(MANIFEST_FILE), &manifest)?;
    package.publish()?;

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
}

fn write_manifest(path: &Path, manifest: &BackupManifest) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(BackupError::TooLarge);
    }
    write_new_synced(path, &bytes)
}

fn map_target_database_error(error: DatabaseError) -> BackupError {
    match error {
        DatabaseError::Sqlite(error) => BackupError::Database(error),
        DatabaseError::EmptyKey | DatabaseError::UnsupportedSchema(_) => {
            BackupError::InvalidDestination
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use tempfile::tempdir;

    use super::{BackupError, StagedBackupPackage};

    const LABEL: &str = "mistake-trainer-backup-test";

    #[test]
    fn dropping_unpublished_backup_removes_only_its_temporary_directory() {
        let destination = tempdir().expect("temporary destination");
        let sentinel = destination.path().join("keep.txt");
        fs::write(&sentinel, b"keep").expect("write sentinel");
        let package =
            StagedBackupPackage::create(destination.path(), LABEL).expect("stage package");
        let temporary_path = package.path().to_owned();

        drop(package);

        assert!(!temporary_path.exists());
        assert_eq!(fs::read(sentinel).expect("read sentinel"), b"keep");
    }

    #[test]
    fn publishing_rejects_and_preserves_a_preexisting_completed_package() {
        let destination = tempdir().expect("temporary destination");
        let completed = destination.path().join(LABEL);
        fs::create_dir(&completed).expect("create completed package");
        let sentinel = completed.join("manifest.json");
        fs::write(&sentinel, b"completed").expect("write completed sentinel");
        let mut package =
            StagedBackupPackage::create(destination.path(), LABEL).expect("stage package");
        let temporary_path = package.path().to_owned();

        let error = package.publish().expect_err("existing package must win");
        assert!(matches!(
            error,
            BackupError::Io(ref error) if error.kind() == io::ErrorKind::AlreadyExists
        ));
        drop(package);

        assert!(!temporary_path.exists());
        assert_eq!(
            fs::read(sentinel).expect("read completed sentinel"),
            b"completed"
        );
    }

    #[test]
    fn publishing_keeps_the_completed_backup_package() {
        let destination = tempdir().expect("temporary destination");
        let completed = destination.path().join(LABEL);
        let mut package =
            StagedBackupPackage::create(destination.path(), LABEL).expect("stage package");
        fs::write(package.path().join("manifest.json"), b"completed")
            .expect("write staged manifest");

        package.publish().expect("publish package");
        drop(package);

        assert_eq!(
            fs::read(completed.join("manifest.json")).expect("read completed manifest"),
            b"completed"
        );
    }
}
