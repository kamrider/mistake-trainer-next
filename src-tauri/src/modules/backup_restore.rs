use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use super::{
    BackupError, BackupManifest, BackupRestoreCandidate, BackupRestoreReceipt, BackupSummary,
    MANIFEST_FILE, MAX_ASSET_BYTES, MAX_DATABASE_BYTES, MAX_MANIFEST_BYTES, ManifestFile,
    RESTORE_CANDIDATE_TTL_MS,
    backup_package_repository::{
        canonical_contained_file, copy_and_hash, ensure_no_reparse_components, hash_file,
        read_bounded, safe_relative_path, sha256_bytes, write_new_synced,
    },
    backup_restore_repository::{
        PendingRestoreMarker, RESTORE_PENDING_FILE, RestoreCandidateMetadata, RestoreMode,
        ensure_owned_directory_if_present, read_pending_marker, read_restore_candidate_metadata,
        read_restore_receipt, remove_control_file, remove_exact_file, remove_restore_receipt,
        restore_directory_name, rollback_directory_name, write_control_file,
        write_restore_candidate_metadata, write_restore_receipt,
    },
    validate_backup,
};

pub struct RestoreSwap {
    application_root: PathBuf,
    live_root: PathBuf,
    rollback_root: Option<PathBuf>,
    bootstrap_stage_root: Option<PathBuf>,
    marker_path: PathBuf,
    label: String,
}

pub fn prepare_backup_restore(
    source: &Path,
    application_root: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupRestoreCandidate, BackupError> {
    let summary = validate_backup(source, database_key, asset_key, account_id)?;
    let source = source
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    if !application_root.is_dir() {
        return Err(BackupError::InvalidDestination);
    }

    let manifest_path = canonical_contained_file(&source, Path::new(MANIFEST_FILE))?;
    let manifest_bytes = read_bounded(&manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: BackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BackupError::InvalidPackage)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);
    let id = Uuid::now_v7().to_string();
    let directory_name = restore_directory_name(&id)?;
    let temporary = application_root.join(format!(".{directory_name}.tmp"));
    let final_path = application_root.join(&directory_name);
    fs::create_dir(&temporary)?;

    let result = (|| {
        copy_verified_manifest_entry(&source, &temporary, &manifest.database, MAX_DATABASE_BYTES)?;
        for asset in &manifest.assets {
            copy_verified_manifest_entry(&source, &temporary, asset, MAX_ASSET_BYTES)?;
        }
        write_new_synced(&temporary.join(MANIFEST_FILE), &manifest_bytes)?;
        let metadata = RestoreCandidateMetadata {
            id: id.clone(),
            label: summary.label.clone(),
            prepared_at_utc_ms: now_utc_ms,
            manifest_sha256,
        };
        write_restore_candidate_metadata(&temporary, &metadata)?;
        validate_backup(&temporary, database_key, asset_key, account_id)?;
        fs::rename(&temporary, &final_path)?;
        Ok(BackupRestoreCandidate {
            id,
            summary,
            expires_at_utc_ms: now_utc_ms.saturating_add(RESTORE_CANDIDATE_TTL_MS) as f64,
        })
    })();

    if result.is_err() && temporary.parent() == Some(application_root.as_path()) {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

pub fn validate_restore_candidate(
    application_root: &Path,
    candidate_id: &str,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupSummary, BackupError> {
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    let directory_name = restore_directory_name(candidate_id)?;
    let expected = application_root.join(directory_name);
    let metadata = fs::symlink_metadata(&expected).map_err(|_| BackupError::InvalidPackage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BackupError::InvalidPackage);
    }
    let candidate = expected
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    if candidate.parent() != Some(application_root.as_path()) {
        return Err(BackupError::InvalidPackage);
    }

    validate_candidate_directory(
        &candidate,
        candidate_id,
        database_key,
        asset_key,
        account_id,
        now_utc_ms,
    )
}

fn validate_candidate_directory(
    candidate: &Path,
    candidate_id: &str,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupSummary, BackupError> {
    let candidate = candidate
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    if !candidate.is_dir() {
        return Err(BackupError::InvalidPackage);
    }
    let metadata = read_restore_candidate_metadata(&candidate)?;
    if metadata.id != candidate_id {
        return Err(BackupError::InvalidPackage);
    }
    let age = now_utc_ms
        .checked_sub(metadata.prepared_at_utc_ms)
        .ok_or(BackupError::InvalidPackage)?;
    if age > RESTORE_CANDIDATE_TTL_MS {
        return Err(BackupError::ExpiredCandidate);
    }
    let manifest_path = canonical_contained_file(&candidate, Path::new(MANIFEST_FILE))?;
    let (_, manifest_sha256) = hash_file(&manifest_path, MAX_MANIFEST_BYTES)?;
    if manifest_sha256 != metadata.manifest_sha256 {
        return Err(BackupError::Integrity);
    }
    let mut summary = validate_backup(&candidate, database_key, asset_key, account_id)?;
    summary.label = metadata.label;
    Ok(summary)
}

pub fn schedule_backup_restore(
    application_root: &Path,
    candidate_id: &str,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<BackupSummary, BackupError> {
    schedule_backup_restore_with_mode(
        application_root,
        candidate_id,
        database_key,
        asset_key,
        account_id,
        now_utc_ms,
        RestoreMode::ReplaceExisting,
    )
}

pub fn schedule_backup_restore_with_mode(
    application_root: &Path,
    candidate_id: &str,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
    mode: RestoreMode,
) -> Result<BackupSummary, BackupError> {
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    let summary = validate_restore_candidate(
        &application_root,
        candidate_id,
        database_key,
        asset_key,
        account_id,
        now_utc_ms,
    )?;
    let marker_path = application_root.join(RESTORE_PENDING_FILE);
    if marker_path.exists() {
        return Err(BackupError::RestorePending);
    }
    if mode == RestoreMode::BootstrapMissing && application_root.join("library").exists() {
        return Err(BackupError::InvalidDestination);
    }
    let marker = PendingRestoreMarker {
        schema_version: 1,
        candidate_id: candidate_id.to_owned(),
        rollback_id: Uuid::now_v7().to_string(),
        label: summary.label.clone(),
        scheduled_at_utc_ms: now_utc_ms,
        mode,
    };
    write_control_file(&application_root, RESTORE_PENDING_FILE, &marker, false)?;
    Ok(summary)
}

pub fn begin_pending_restore(
    application_root: &Path,
    database_key: &str,
    asset_key: &[u8; 32],
    account_id: &str,
    now_utc_ms: i64,
) -> Result<Option<RestoreSwap>, BackupError> {
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    let marker_path = application_root.join(RESTORE_PENDING_FILE);
    if !marker_path.exists() {
        return Ok(None);
    }
    let marker = read_pending_marker(&application_root)?;
    let stage_root = application_root.join(restore_directory_name(&marker.candidate_id)?);
    let rollback_root = application_root.join(rollback_directory_name(&marker.rollback_id)?);
    let live_root = application_root.join("library");
    ensure_owned_directory_if_present(&application_root, &live_root)?;
    ensure_owned_directory_if_present(&application_root, &stage_root)?;
    ensure_owned_directory_if_present(&application_root, &rollback_root)?;

    let live_exists = live_root.is_dir();
    let stage_exists = stage_root.is_dir();
    let rollback_exists = rollback_root.is_dir();
    let validated_label = if marker.mode == RestoreMode::BootstrapMissing {
        match (live_exists, stage_exists, rollback_exists) {
            (false, true, false) => {
                let summary = validate_candidate_directory(
                    &stage_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                )?;
                if summary.label != marker.label {
                    return Err(BackupError::Integrity);
                }
                fs::rename(&stage_root, &live_root)?;
                summary.label
            }
            (true, false, false) => {
                validate_candidate_directory(
                    &live_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                )?
                .label
            }
            _ => return Err(BackupError::Integrity),
        }
    } else {
        match (live_exists, stage_exists, rollback_exists) {
            (true, true, false) => {
                let summary = validate_candidate_directory(
                    &stage_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                )?;
                if summary.label != marker.label {
                    return Err(BackupError::Integrity);
                }
                fs::rename(&live_root, &rollback_root)?;
                if let Err(error) = fs::rename(&stage_root, &live_root) {
                    let _ = fs::rename(&rollback_root, &live_root);
                    return Err(BackupError::Io(error));
                }
                summary.label
            }
            (false, true, true) => {
                let summary = match validate_candidate_directory(
                    &stage_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                ) {
                    Ok(summary) => summary,
                    Err(_) => {
                        return rollback_interrupted_restore(
                            &application_root,
                            &live_root,
                            &rollback_root,
                            marker.label,
                            now_utc_ms,
                        );
                    }
                };
                if summary.label != marker.label {
                    return rollback_interrupted_restore(
                        &application_root,
                        &live_root,
                        &rollback_root,
                        marker.label,
                        now_utc_ms,
                    );
                }
                fs::rename(&stage_root, &live_root)?;
                summary.label
            }
            (true, false, true) => {
                let summary = match validate_candidate_directory(
                    &live_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                ) {
                    Ok(summary) => summary,
                    Err(_) => {
                        return rollback_interrupted_restore(
                            &application_root,
                            &live_root,
                            &rollback_root,
                            marker.label,
                            now_utc_ms,
                        );
                    }
                };
                if summary.label != marker.label {
                    return rollback_interrupted_restore(
                        &application_root,
                        &live_root,
                        &rollback_root,
                        marker.label,
                        now_utc_ms,
                    );
                }
                summary.label
            }
            (true, false, false) => {
                validate_candidate_directory(
                    &live_root,
                    &marker.candidate_id,
                    database_key,
                    asset_key,
                    account_id,
                    now_utc_ms,
                )?
                .label
            }
            (false, false, true) => {
                fs::rename(&rollback_root, &live_root)?;
                let receipt = BackupRestoreReceipt {
                    status: "rolled_back".to_owned(),
                    label: marker.label,
                    finished_at_utc_ms: now_utc_ms as f64,
                };
                write_restore_receipt(&application_root, &receipt)?;
                remove_control_file(&application_root, RESTORE_PENDING_FILE)?;
                return Ok(None);
            }
            _ => return Err(BackupError::Integrity),
        }
    };
    if validated_label != marker.label {
        return Err(BackupError::Integrity);
    }

    let rollback_root = rollback_root.is_dir().then_some(rollback_root);
    let bootstrap_stage_root = (marker.mode == RestoreMode::BootstrapMissing)
        .then_some(application_root.join(restore_directory_name(&marker.candidate_id)?));
    Ok(Some(RestoreSwap {
        application_root,
        live_root,
        rollback_root,
        bootstrap_stage_root,
        marker_path,
        label: marker.label,
    }))
}

fn rollback_interrupted_restore(
    application_root: &Path,
    live_root: &Path,
    rollback_root: &Path,
    label: String,
    now_utc_ms: i64,
) -> Result<Option<RestoreSwap>, BackupError> {
    ensure_owned_directory_if_present(application_root, live_root)?;
    ensure_owned_directory_if_present(application_root, rollback_root)?;
    if live_root.is_dir() {
        fs::remove_dir_all(live_root)?;
    }
    fs::rename(rollback_root, live_root)?;
    let receipt = BackupRestoreReceipt {
        status: "rolled_back".to_owned(),
        label,
        finished_at_utc_ms: now_utc_ms as f64,
    };
    write_restore_receipt(application_root, &receipt)?;
    remove_control_file(application_root, RESTORE_PENDING_FILE)?;
    Ok(None)
}

impl RestoreSwap {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn replaces_existing_library(&self) -> bool {
        self.bootstrap_stage_root.is_none()
    }

    pub fn commit(self, now_utc_ms: i64) -> Result<BackupRestoreReceipt, BackupError> {
        if let Some(rollback_root) = &self.rollback_root {
            ensure_owned_directory_if_present(&self.application_root, rollback_root)?;
            if rollback_root.is_dir() {
                fs::remove_dir_all(rollback_root)?;
            }
        }
        let receipt = BackupRestoreReceipt {
            status: "succeeded".to_owned(),
            label: self.label,
            finished_at_utc_ms: now_utc_ms as f64,
        };
        write_restore_receipt(&self.application_root, &receipt)?;
        remove_exact_file(&self.marker_path)?;
        Ok(receipt)
    }

    pub fn rollback(self, now_utc_ms: i64) -> Result<BackupRestoreReceipt, BackupError> {
        if let Some(stage_root) = self.bootstrap_stage_root {
            ensure_owned_directory_if_present(&self.application_root, &self.live_root)?;
            ensure_owned_directory_if_present(&self.application_root, &stage_root)?;
            if stage_root.exists() || !self.live_root.is_dir() {
                return Err(BackupError::Integrity);
            }
            fs::rename(&self.live_root, &stage_root)?;
            return Ok(BackupRestoreReceipt {
                status: "rolled_back".to_owned(),
                label: self.label,
                finished_at_utc_ms: now_utc_ms as f64,
            });
        }
        let rollback_root = self.rollback_root.ok_or(BackupError::Integrity)?;
        ensure_owned_directory_if_present(&self.application_root, &self.live_root)?;
        ensure_owned_directory_if_present(&self.application_root, &rollback_root)?;
        if self.live_root.is_dir() {
            fs::remove_dir_all(&self.live_root)?;
        }
        if let Err(error) = fs::rename(&rollback_root, &self.live_root) {
            return Err(BackupError::Io(error));
        }
        let receipt = BackupRestoreReceipt {
            status: "rolled_back".to_owned(),
            label: self.label,
            finished_at_utc_ms: now_utc_ms as f64,
        };
        write_restore_receipt(&self.application_root, &receipt)?;
        remove_exact_file(&self.marker_path)?;
        Ok(receipt)
    }
}

pub fn record_failed_restore(
    application_root: &Path,
    now_utc_ms: i64,
) -> Result<BackupRestoreReceipt, BackupError> {
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    let label = read_pending_marker(&application_root)
        .map(|marker| marker.label)
        .unwrap_or_else(|_| "恢复包".to_owned());
    let receipt = BackupRestoreReceipt {
        status: "failed_validation".to_owned(),
        label,
        finished_at_utc_ms: now_utc_ms as f64,
    };
    write_restore_receipt(&application_root, &receipt)?;
    remove_control_file(&application_root, RESTORE_PENDING_FILE)?;
    Ok(receipt)
}

pub fn take_restore_receipt(
    application_root: &Path,
) -> Result<Option<BackupRestoreReceipt>, BackupError> {
    let application_root = application_root
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    let Some(receipt) = read_restore_receipt(&application_root)? else {
        return Ok(None);
    };
    if !matches!(
        receipt.status.as_str(),
        "succeeded" | "rolled_back" | "failed_validation"
    ) {
        return Err(BackupError::InvalidPackage);
    }
    remove_restore_receipt(&application_root)?;
    Ok(Some(receipt))
}

fn copy_verified_manifest_entry(
    source_root: &Path,
    destination_root: &Path,
    entry: &ManifestFile,
    max_bytes: u64,
) -> Result<(), BackupError> {
    let relative = safe_relative_path(&entry.relative_path)?;
    ensure_no_reparse_components(source_root, &relative)?;
    let source = canonical_contained_file(source_root, &relative)?;
    let destination = destination_root.join(&relative);
    let parent = destination.parent().ok_or(BackupError::InvalidPackage)?;
    fs::create_dir_all(parent)?;
    let (bytes, sha256) = copy_and_hash(&source, &destination, max_bytes)?;
    if bytes != entry.encrypted_bytes || sha256 != entry.ciphertext_sha256 {
        return Err(BackupError::Integrity);
    }
    Ok(())
}
