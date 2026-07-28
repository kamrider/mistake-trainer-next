use std::path::Path;

use thiserror::Error;

use crate::{
    infrastructure::{
        runtime::{
            LibraryRuntime, RuntimeError, SecretStore, initialize_local_library, library_is_locked,
            load_restore_credentials,
        },
        storage_location::{StorageLocationError, resolve_storage},
    },
    modules::{
        backup::{BackupError, begin_pending_restore, record_failed_restore},
        storage_migration::{StorageMigrationError, apply_pending_storage_migration},
    },
};

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("local library startup failed")]
    Runtime(#[from] RuntimeError),
    #[error("pending restore could not be applied")]
    Restore(#[from] BackupError),
    #[error("restored library failed to initialize and rollback also failed")]
    RollbackFailed {
        runtime: RuntimeError,
        rollback: BackupError,
    },
    #[error("restore validation and current library startup both failed")]
    RestoreFallbackFailed {
        restore: BackupError,
        runtime: RuntimeError,
    },
}

pub enum LibraryStartup {
    Ready(LibraryRuntime),
    Locked,
    AccessUnavailable(StartupAccessUnavailable),
}

#[derive(Debug, Error)]
pub enum StartupAccessUnavailable {
    #[error("secure credential access is unavailable")]
    Credentials(#[source] RuntimeError),
    #[error("configured library storage is unavailable")]
    Storage(#[source] StorageLocationError),
    #[error("a pending storage migration could not be applied")]
    StorageMigration(#[source] StorageMigrationError),
}

impl StartupAccessUnavailable {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Credentials(error) => error.code(),
            Self::Storage(error) => error.code(),
            Self::StorageMigration(error) => error.code(),
        }
    }
}

/// Applies the process-start access boundary before any SQLCipher connection
/// can be created. The caller can still start the Tauri shell for locked and
/// credential-error states without ever constructing `LibraryRuntime`.
pub fn initialize_application_library_if_accessible(
    data_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<LibraryStartup, StartupError> {
    match library_is_locked(secrets) {
        Ok(false) => initialize_application_library(data_root, secrets, now_utc_ms)
            .map(LibraryStartup::Ready),
        Ok(true) => Ok(LibraryStartup::Locked),
        Err(error) => Ok(LibraryStartup::AccessUnavailable(
            StartupAccessUnavailable::Credentials(error),
        )),
    }
}

/// Resolves the movable encrypted library only after the process-start lock
/// marker has allowed access. Invalid or unavailable custom storage is exposed
/// only as a public-safe locked-shell state and never falls back to the default
/// root.
pub fn initialize_configured_application_library_if_accessible(
    control_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<LibraryStartup, StartupError> {
    match library_is_locked(secrets) {
        Ok(true) => Ok(LibraryStartup::Locked),
        Err(error) => Ok(LibraryStartup::AccessUnavailable(
            StartupAccessUnavailable::Credentials(error),
        )),
        Ok(false) => {
            match apply_pending_storage_migration(control_root, secrets, now_utc_ms) {
                Ok(Some(runtime)) => return Ok(LibraryStartup::Ready(runtime)),
                Ok(None) => {}
                Err(error) => {
                    return Ok(LibraryStartup::AccessUnavailable(
                        StartupAccessUnavailable::StorageMigration(error),
                    ));
                }
            }
            let storage = match resolve_storage(control_root) {
                Ok(storage) => storage,
                Err(error) => {
                    return Ok(LibraryStartup::AccessUnavailable(
                        StartupAccessUnavailable::Storage(error),
                    ));
                }
            };
            initialize_application_library(storage.library_root(), secrets, now_utc_ms)
                .map(LibraryStartup::Ready)
        }
    }
}

/// Opens the fixed local library root, applying any scheduled restore before the
/// first SQLCipher connection is created. A restored library is committed only
/// after its migrations and active profile have initialized successfully.
pub fn initialize_application_library(
    data_root: &Path,
    secrets: &dyn SecretStore,
    now_utc_ms: i64,
) -> Result<LibraryRuntime, StartupError> {
    let application_root = data_root.parent().ok_or(BackupError::InvalidDestination)?;
    std::fs::create_dir_all(application_root).map_err(RuntimeError::File)?;

    let marker_path = application_root.join("restore-pending.json");
    if !marker_path.exists() {
        return initialize_local_library(data_root, secrets, now_utc_ms).map_err(Into::into);
    }

    let credentials = load_restore_credentials(secrets)?;
    let pending = begin_pending_restore(
        application_root,
        &credentials.database_key,
        &credentials.asset_key,
        &credentials.account_id,
        now_utc_ms,
    );

    let Some(swap) = (match pending {
        Ok(value) => value,
        Err(restore) if is_candidate_validation_error(&restore) => {
            match initialize_local_library(data_root, secrets, now_utc_ms) {
                Ok(runtime) => {
                    // Only consume the marker after proving the untouched live
                    // library still opens. If it does not, a later startup keeps
                    // the recovery state available instead of destroying evidence.
                    record_failed_restore(application_root, now_utc_ms)?;
                    return Ok(runtime);
                }
                Err(runtime) => {
                    return Err(StartupError::RestoreFallbackFailed { restore, runtime });
                }
            }
        }
        Err(error) => return Err(error.into()),
    }) else {
        return initialize_local_library(data_root, secrets, now_utc_ms).map_err(Into::into);
    };

    match initialize_local_library(data_root, secrets, now_utc_ms) {
        Ok(runtime) => {
            swap.commit(now_utc_ms)?;
            Ok(runtime)
        }
        Err(runtime) => {
            if let Err(rollback) = swap.rollback(now_utc_ms) {
                return Err(StartupError::RollbackFailed { runtime, rollback });
            }
            initialize_local_library(data_root, secrets, now_utc_ms).map_err(Into::into)
        }
    }
}

fn is_candidate_validation_error(error: &BackupError) -> bool {
    matches!(
        error,
        BackupError::InvalidPackage
            | BackupError::AccountMismatch
            | BackupError::ForeignAccountData
            | BackupError::UnsupportedSchema
            | BackupError::TooLarge
            | BackupError::Integrity
            | BackupError::ExpiredCandidate
    )
}
