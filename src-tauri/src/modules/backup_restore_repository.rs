use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    BackupError, BackupRestoreReceipt,
    backup_package_repository::{canonical_contained_file, read_bounded, write_new_synced},
};

const RESTORE_METADATA_FILE: &str = "restore-candidate.json";
const MAX_RESTORE_METADATA_BYTES: u64 = 64 * 1024;
const RESTORE_DIRECTORY_PREFIX: &str = ".mistake-trainer-restore-";
const RESTORE_ROLLBACK_PREFIX: &str = ".mistake-trainer-rollback-";
pub(super) const RESTORE_PENDING_FILE: &str = "restore-pending.json";
const RESTORE_RECEIPT_FILE: &str = "restore-receipt.json";
const MAX_RESTORE_CONTROL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RestoreCandidateMetadata {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) prepared_at_utc_ms: i64,
    pub(super) manifest_sha256: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode {
    #[default]
    ReplaceExisting,
    BootstrapMissing,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PendingRestoreMarker {
    #[serde(default = "default_marker_schema_version")]
    pub(super) schema_version: u32,
    pub(super) candidate_id: String,
    pub(super) rollback_id: String,
    pub(super) label: String,
    pub(super) scheduled_at_utc_ms: i64,
    #[serde(default)]
    pub(super) mode: RestoreMode,
}

const fn default_marker_schema_version() -> u32 {
    1
}

pub(super) fn read_pending_marker(
    application_root: &Path,
) -> Result<PendingRestoreMarker, BackupError> {
    let path = canonical_contained_file(application_root, Path::new(RESTORE_PENDING_FILE))?;
    let bytes = read_bounded(&path, MAX_RESTORE_CONTROL_BYTES)?;
    let marker: PendingRestoreMarker =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::InvalidPackage)?;
    if marker.schema_version != 1 {
        return Err(BackupError::InvalidPackage);
    }
    restore_directory_name(&marker.candidate_id)?;
    rollback_directory_name(&marker.rollback_id)?;
    Ok(marker)
}

pub(super) fn write_restore_candidate_metadata(
    candidate: &Path,
    metadata: &RestoreCandidateMetadata,
) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(metadata).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESTORE_METADATA_BYTES {
        return Err(BackupError::TooLarge);
    }
    write_new_synced(&candidate.join(RESTORE_METADATA_FILE), &bytes)
}

pub(super) fn read_restore_candidate_metadata(
    candidate: &Path,
) -> Result<RestoreCandidateMetadata, BackupError> {
    let path = canonical_contained_file(candidate, Path::new(RESTORE_METADATA_FILE))?;
    let bytes = read_bounded(&path, MAX_RESTORE_METADATA_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|_| BackupError::InvalidPackage)
}

pub(super) fn read_restore_receipt(
    application_root: &Path,
) -> Result<Option<BackupRestoreReceipt>, BackupError> {
    let path = application_root.join(RESTORE_RECEIPT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let path = canonical_contained_file(application_root, Path::new(RESTORE_RECEIPT_FILE))?;
    let bytes = read_bounded(&path, MAX_RESTORE_CONTROL_BYTES)?;
    let receipt = serde_json::from_slice(&bytes).map_err(|_| BackupError::InvalidPackage)?;
    Ok(Some(receipt))
}

pub(super) fn remove_restore_receipt(application_root: &Path) -> Result<(), BackupError> {
    remove_control_file(application_root, RESTORE_RECEIPT_FILE)
}

pub(super) fn restore_directory_name(candidate_id: &str) -> Result<String, BackupError> {
    let parsed = Uuid::parse_str(candidate_id).map_err(|_| BackupError::InvalidPackage)?;
    if parsed.to_string() != candidate_id {
        return Err(BackupError::InvalidPackage);
    }
    Ok(format!("{RESTORE_DIRECTORY_PREFIX}{candidate_id}"))
}

pub(super) fn rollback_directory_name(rollback_id: &str) -> Result<String, BackupError> {
    let parsed = Uuid::parse_str(rollback_id).map_err(|_| BackupError::InvalidPackage)?;
    if parsed.to_string() != rollback_id {
        return Err(BackupError::InvalidPackage);
    }
    Ok(format!("{RESTORE_ROLLBACK_PREFIX}{rollback_id}"))
}

pub(super) fn ensure_owned_directory_if_present(
    root: &Path,
    path: &Path,
) -> Result<(), BackupError> {
    if !path.exists() {
        return Ok(());
    }
    if path.parent() != Some(root) {
        return Err(BackupError::InvalidPackage);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| BackupError::InvalidPackage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BackupError::InvalidPackage);
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| BackupError::InvalidPackage)?;
    if canonical.parent() != Some(root) {
        return Err(BackupError::InvalidPackage);
    }
    Ok(())
}

pub(super) fn write_control_file<T: Serialize>(
    application_root: &Path,
    file_name: &str,
    value: &T,
    replace: bool,
) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| BackupError::InvalidPackage)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESTORE_CONTROL_BYTES {
        return Err(BackupError::TooLarge);
    }
    let target = application_root.join(file_name);
    if target.exists() && !replace {
        return Err(BackupError::RestorePending);
    }
    let temporary = application_root.join(format!(".{file_name}.{}.tmp", Uuid::now_v7()));
    write_new_synced(&temporary, &bytes)?;
    let result = (|| {
        if replace && target.exists() {
            remove_exact_file(&target)?;
        }
        fs::rename(&temporary, &target)?;
        Ok(())
    })();
    if result.is_err() && temporary.parent() == Some(application_root) {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn write_restore_receipt(
    application_root: &Path,
    receipt: &BackupRestoreReceipt,
) -> Result<(), BackupError> {
    write_control_file(application_root, RESTORE_RECEIPT_FILE, receipt, true)
}

pub(super) fn remove_control_file(
    application_root: &Path,
    file_name: &str,
) -> Result<(), BackupError> {
    let path = application_root.join(file_name);
    if !path.exists() {
        return Ok(());
    }
    if path.parent() != Some(application_root) {
        return Err(BackupError::InvalidPackage);
    }
    remove_exact_file(&path)
}

pub(super) fn remove_exact_file(path: &Path) -> Result<(), BackupError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| BackupError::InvalidPackage)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(BackupError::InvalidPackage);
    }
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        PendingRestoreMarker, RESTORE_PENDING_FILE, read_pending_marker, remove_exact_file,
        restore_directory_name, write_control_file,
    };
    use crate::modules::backup::{BackupError, RestoreMode};

    const CANDIDATE_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
    const ROLLBACK_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";

    fn marker() -> PendingRestoreMarker {
        PendingRestoreMarker {
            schema_version: 1,
            candidate_id: CANDIDATE_ID.to_owned(),
            rollback_id: ROLLBACK_ID.to_owned(),
            label: "数学备份".to_owned(),
            scheduled_at_utc_ms: 42,
            mode: RestoreMode::ReplaceExisting,
        }
    }

    #[test]
    fn restore_directory_names_require_canonical_uuid_text() {
        assert_eq!(
            restore_directory_name(CANDIDATE_ID).unwrap(),
            format!(".mistake-trainer-restore-{CANDIDATE_ID}")
        );
        assert!(matches!(
            restore_directory_name("0191365E-2F2F-7B89-B3B0-111111111111"),
            Err(BackupError::InvalidPackage)
        ));
        assert!(matches!(
            restore_directory_name("0191365e2f2f7b89b3b0111111111111"),
            Err(BackupError::InvalidPackage)
        ));
    }

    #[test]
    fn a_second_non_replacing_control_write_preserves_restore_pending() {
        let root = tempdir().unwrap();
        write_control_file(root.path(), RESTORE_PENDING_FILE, &marker(), false).unwrap();

        assert!(matches!(
            write_control_file(root.path(), RESTORE_PENDING_FILE, &marker(), false),
            Err(BackupError::RestorePending)
        ));
    }

    #[test]
    fn exact_file_removal_rejects_directories() {
        let root = tempdir().unwrap();
        let directory = root.path().join("not-a-file");
        fs::create_dir(&directory).unwrap();

        assert!(matches!(
            remove_exact_file(&directory),
            Err(BackupError::InvalidPackage)
        ));
        assert!(directory.is_dir());
    }

    #[test]
    fn pending_marker_round_trip_preserves_camel_case_payload() {
        let root = tempdir().unwrap();
        let application_root = root.path().canonicalize().unwrap();
        let expected = marker();
        write_control_file(&application_root, RESTORE_PENDING_FILE, &expected, false).unwrap();

        let bytes = fs::read(application_root.join(RESTORE_PENDING_FILE)).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["candidateId"], CANDIDATE_ID);
        assert_eq!(json["rollbackId"], ROLLBACK_ID);
        assert_eq!(json["scheduledAtUtcMs"], 42);

        let actual = read_pending_marker(&application_root).unwrap();
        assert_eq!(actual.candidate_id, expected.candidate_id);
        assert_eq!(actual.rollback_id, expected.rollback_id);
        assert_eq!(actual.label, expected.label);
        assert_eq!(actual.scheduled_at_utc_ms, expected.scheduled_at_utc_ms);
    }
}
