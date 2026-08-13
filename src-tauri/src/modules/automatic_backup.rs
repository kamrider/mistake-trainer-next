use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::backup::{BackupError, BackupSummary, create_backup};

const POLICY_FILE: &str = "automatic-backup.json";
const POLICY_SCHEMA_VERSION: u32 = 1;
const BACKUP_PREFIX: &str = "mistake-trainer-backup-";
const AUTOMATIC_BACKUP_DIRECTORY: &str = "Mistake Trainer Automatic Backups";
const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAutomaticBackupPolicy {
    schema_version: u32,
    enabled: bool,
    interval_days: u32,
    retention_count: u32,
    destination: PathBuf,
    last_success_at_utc_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticBackupStatus {
    pub enabled: bool,
    pub interval_days: u32,
    pub retention_count: u32,
    pub destination_label: Option<String>,
    pub last_success_at_utc_ms: Option<f64>,
}

pub fn configure_automatic_backup(
    control_root: &Path,
    destination: &Path,
    interval_days: u32,
    retention_count: u32,
) -> Result<AutomaticBackupStatus, BackupError> {
    validate_policy_values(interval_days, retention_count)?;
    let control_root = canonical_directory(control_root)?;
    let selected_destination = canonical_directory(destination)?;
    if selected_destination.starts_with(&control_root) {
        return Err(BackupError::InvalidDestination);
    }
    let automatic_destination = selected_destination.join(AUTOMATIC_BACKUP_DIRECTORY);
    match fs::symlink_metadata(&automatic_destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(BackupError::InvalidDestination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&automatic_destination)?;
        }
        Err(error) => return Err(error.into()),
    }
    let destination = canonical_directory(&automatic_destination)?;
    let previous = load_policy(&control_root)?;
    let last_success_at_utc_ms = previous.as_ref().and_then(|value| {
        (value.destination == destination)
            .then_some(value.last_success_at_utc_ms)
            .flatten()
    });
    let policy = StoredAutomaticBackupPolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        enabled: true,
        interval_days,
        retention_count,
        destination,
        last_success_at_utc_ms,
    };
    write_policy(&control_root, &policy)?;
    Ok(status_for(&policy))
}

pub fn disable_automatic_backup(control_root: &Path) -> Result<AutomaticBackupStatus, BackupError> {
    let control_root = canonical_directory(control_root)?;
    let Some(mut policy) = load_policy(&control_root)? else {
        return Ok(disabled_status());
    };
    policy.enabled = false;
    write_policy(&control_root, &policy)?;
    Ok(status_for(&policy))
}

pub fn automatic_backup_status(control_root: &Path) -> Result<AutomaticBackupStatus, BackupError> {
    let control_root = canonical_directory(control_root)?;
    Ok(load_policy(&control_root)?
        .as_ref()
        .map(status_for)
        .unwrap_or_else(disabled_status))
}

pub fn run_due_automatic_backup(
    control_root: &Path,
    connection: &Mutex<Connection>,
    blob_root: &Path,
    database_key: &str,
    account_id: &str,
    now_utc_ms: i64,
) -> Result<Option<BackupSummary>, BackupError> {
    let control_root = canonical_directory(control_root)?;
    let Some(mut policy) = load_policy(&control_root)? else {
        return Ok(None);
    };
    if !policy.enabled
        || !backup_is_due(
            policy.last_success_at_utc_ms,
            now_utc_ms,
            policy.interval_days,
        )
    {
        return Ok(None);
    }
    let summary = create_backup(
        connection,
        blob_root,
        database_key,
        account_id,
        &policy.destination,
        now_utc_ms,
    )?;
    policy.last_success_at_utc_ms = Some(now_utc_ms);
    write_policy(&control_root, &policy)?;
    prune_owned_packages(&policy.destination, policy.retention_count)?;
    Ok(Some(summary))
}

pub fn backup_is_due(last_success_at_utc_ms: Option<i64>, now_utc_ms: i64, days: u32) -> bool {
    last_success_at_utc_ms.is_none_or(|last| {
        now_utc_ms.saturating_sub(last) >= i64::from(days).saturating_mul(MILLIS_PER_DAY)
    })
}

fn validate_policy_values(interval_days: u32, retention_count: u32) -> Result<(), BackupError> {
    if !(1..=30).contains(&interval_days) || !(1..=20).contains(&retention_count) {
        return Err(BackupError::InvalidPolicy);
    }
    Ok(())
}

fn canonical_directory(path: &Path) -> Result<PathBuf, BackupError> {
    let path = path
        .canonicalize()
        .map_err(|_| BackupError::InvalidDestination)?;
    if !path.is_dir() {
        return Err(BackupError::InvalidDestination);
    }
    Ok(path)
}

fn load_policy(control_root: &Path) -> Result<Option<StoredAutomaticBackupPolicy>, BackupError> {
    let path = control_root.join(POLICY_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| BackupError::InvalidPackage)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 64 * 1024 {
        return Err(BackupError::InvalidPackage);
    }
    let bytes = fs::read(path)?;
    let policy: StoredAutomaticBackupPolicy =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::InvalidPackage)?;
    if policy.schema_version != POLICY_SCHEMA_VERSION {
        return Err(BackupError::InvalidPackage);
    }
    validate_policy_values(policy.interval_days, policy.retention_count)?;
    Ok(Some(policy))
}

fn write_policy(
    control_root: &Path,
    policy: &StoredAutomaticBackupPolicy,
) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(policy).map_err(|_| BackupError::InvalidPackage)?;
    let temporary = control_root.join(format!(".{POLICY_FILE}.{}.tmp", Uuid::now_v7().simple()));
    let target = control_root.join(POLICY_FILE);
    {
        use std::io::Write as _;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        output.write_all(&bytes)?;
        output.sync_all()?;
    }
    let result = (|| {
        if target.exists() {
            let metadata = fs::symlink_metadata(&target)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(BackupError::InvalidPackage);
            }
        }
        replace_file_atomically(&temporary, &target)?;
        Ok(())
    })();
    if result.is_err() && temporary.parent() == Some(control_root) {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(windows)]
fn replace_file_atomically(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR::from_raw(source.as_ptr()),
            PCWSTR::from_raw(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))
    }
}

#[cfg(not(windows))]
fn replace_file_atomically(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

fn prune_owned_packages(destination: &Path, retention_count: u32) -> Result<(), BackupError> {
    let destination = canonical_directory(destination)?;
    let mut owned = Vec::new();
    for entry in fs::read_dir(&destination)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(suffix) = name.strip_prefix(BACKUP_PREFIX) else {
            continue;
        };
        if suffix.len() != 32
            || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
            || Uuid::parse_str(suffix).is_err()
        {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let manifest = path.join("manifest.json");
        let manifest_metadata = match fs::symlink_metadata(&manifest) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !manifest_metadata.is_file() || manifest_metadata.file_type().is_symlink() {
            continue;
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| BackupError::InvalidPackage)?;
        if canonical.parent() == Some(destination.as_path()) {
            owned.push((name.to_owned(), canonical));
        }
    }
    owned.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in owned.into_iter().skip(retention_count as usize) {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn status_for(policy: &StoredAutomaticBackupPolicy) -> AutomaticBackupStatus {
    AutomaticBackupStatus {
        enabled: policy.enabled,
        interval_days: policy.interval_days,
        retention_count: policy.retention_count,
        destination_label: policy
            .destination
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .map(|value| format!("{value} / {AUTOMATIC_BACKUP_DIRECTORY}")),
        last_success_at_utc_ms: policy.last_success_at_utc_ms.map(|value| value as f64),
    }
}

fn disabled_status() -> AutomaticBackupStatus {
    AutomaticBackupStatus {
        enabled: false,
        interval_days: 7,
        retention_count: 5,
        destination_label: None,
        last_success_at_utc_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_calculation_is_bounded_and_deterministic() {
        assert!(backup_is_due(None, 100, 7));
        assert!(!backup_is_due(Some(100), 100 + 7 * MILLIS_PER_DAY - 1, 7));
        assert!(backup_is_due(Some(100), 100 + 7 * MILLIS_PER_DAY, 7));
        assert!(!backup_is_due(Some(200), 100, 7));
    }

    #[test]
    fn retention_removes_only_valid_owned_packages() {
        let root = tempfile::tempdir().expect("destination");
        let keep = root.path().join("unrelated");
        fs::create_dir(&keep).expect("unrelated");
        let fake = root
            .path()
            .join("mistake-trainer-backup-ffffffffffffffffffffffffffffffff");
        fs::create_dir(&fake).expect("fake");
        for suffix in [
            "00000000000000000000000000000001",
            "00000000000000000000000000000002",
            "00000000000000000000000000000003",
        ] {
            let package = root.path().join(format!("{BACKUP_PREFIX}{suffix}"));
            fs::create_dir(&package).expect("package");
            fs::write(package.join("manifest.json"), b"manifest").expect("manifest");
        }

        prune_owned_packages(root.path(), 2).expect("prune");

        assert!(keep.exists());
        assert!(fake.exists(), "a prefix alone does not authorize deletion");
        assert!(
            !root
                .path()
                .join("mistake-trainer-backup-00000000000000000000000000000001")
                .exists()
        );
        assert!(
            root.path()
                .join("mistake-trainer-backup-00000000000000000000000000000003")
                .exists()
        );
    }

    #[test]
    fn policy_round_trips_without_exposing_the_full_destination_in_status() {
        let control = tempfile::tempdir().expect("control");
        let destination = tempfile::tempdir().expect("destination");

        let status = configure_automatic_backup(control.path(), destination.path(), 7, 5)
            .expect("configure");
        let loaded = automatic_backup_status(control.path()).expect("status");

        assert_eq!(status, loaded);
        assert!(loaded.enabled);
        assert_eq!(loaded.interval_days, 7);
        assert_eq!(loaded.retention_count, 5);
        assert_ne!(
            loaded.destination_label.as_deref(),
            destination.path().to_str()
        );
        assert!(
            loaded
                .destination_label
                .as_deref()
                .is_some_and(|label| label.ends_with(AUTOMATIC_BACKUP_DIRECTORY))
        );
        assert!(!disable_automatic_backup(control.path()).unwrap().enabled);
    }

    #[test]
    fn changing_destination_makes_the_new_location_due_immediately() {
        let control = tempfile::tempdir().expect("control");
        let first = tempfile::tempdir().expect("first destination");
        let second = tempfile::tempdir().expect("second destination");
        configure_automatic_backup(control.path(), first.path(), 7, 5).expect("first policy");
        let mut policy = load_policy(control.path()).unwrap().unwrap();
        policy.last_success_at_utc_ms = Some(123);
        write_policy(control.path(), &policy).unwrap();

        configure_automatic_backup(control.path(), second.path(), 7, 5).expect("second policy");
        assert_eq!(
            load_policy(control.path())
                .unwrap()
                .unwrap()
                .last_success_at_utc_ms,
            None
        );
    }

    #[test]
    fn failed_policy_replacement_preserves_the_previous_policy() {
        let root = tempfile::tempdir().expect("policy root");
        let target = root.path().join(POLICY_FILE);
        let missing_source = root.path().join("missing-policy.tmp");
        fs::write(&target, b"previous policy").expect("seed policy");

        assert!(replace_file_atomically(&missing_source, &target).is_err());
        assert_eq!(
            fs::read(&target).expect("previous policy remains"),
            b"previous policy"
        );
    }
}
