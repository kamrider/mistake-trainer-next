use std::path::Path;

use serde::Serialize;
use specta::Type;

use crate::infrastructure::runtime::CredentialEnvelopeState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryArtifactState {
    Absent,
    Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LibraryRecoveryReason {
    LocalDataMissing,
    SetupInterrupted,
    CredentialsIncomplete,
    ResetIncomplete,
    StorageDisconnected,
    MigrationInterrupted,
    RestoreInterrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupInventory {
    pub credentials: CredentialEnvelopeState,
    pub artifacts: LibraryArtifactState,
    pub pointer_present: bool,
    pub storage_migration_pending: bool,
    pub restore_pending: bool,
    pub reset_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupDisposition {
    FirstRun,
    OpenExisting,
    RecoveryRequired(LibraryRecoveryReason),
}

pub const fn classify_startup_inventory(inventory: StartupInventory) -> StartupDisposition {
    if inventory.reset_pending {
        return StartupDisposition::RecoveryRequired(LibraryRecoveryReason::ResetIncomplete);
    }
    if inventory.storage_migration_pending {
        return StartupDisposition::RecoveryRequired(LibraryRecoveryReason::MigrationInterrupted);
    }
    if inventory.restore_pending {
        return StartupDisposition::RecoveryRequired(LibraryRecoveryReason::RestoreInterrupted);
    }
    match (
        inventory.credentials,
        inventory.artifacts,
        inventory.pointer_present,
    ) {
        (CredentialEnvelopeState::Absent, LibraryArtifactState::Absent, false) => {
            StartupDisposition::FirstRun
        }
        (CredentialEnvelopeState::Complete, LibraryArtifactState::Present, _) => {
            StartupDisposition::OpenExisting
        }
        (CredentialEnvelopeState::Complete, LibraryArtifactState::Absent, _)
        | (CredentialEnvelopeState::Absent, LibraryArtifactState::Absent, true) => {
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
        }
        (CredentialEnvelopeState::Partial, LibraryArtifactState::Absent, false) => {
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::SetupInterrupted)
        }
        (CredentialEnvelopeState::Absent, LibraryArtifactState::Present, _)
        | (CredentialEnvelopeState::Partial, _, _) => {
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::CredentialsIncomplete)
        }
    }
}

pub fn inspect_library_artifacts(
    library_root: &Path,
) -> Result<LibraryArtifactState, std::io::Error> {
    for path in [library_root.join("library.db"), library_root.join("assets")] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Ok(LibraryArtifactState::Present),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(LibraryArtifactState::Absent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_truly_empty_inventory_is_first_run() {
        let empty = StartupInventory {
            credentials: CredentialEnvelopeState::Absent,
            artifacts: LibraryArtifactState::Absent,
            pointer_present: false,
            storage_migration_pending: false,
            restore_pending: false,
            reset_pending: false,
        };
        assert_eq!(
            classify_startup_inventory(empty),
            StartupDisposition::FirstRun
        );
        assert_eq!(
            classify_startup_inventory(StartupInventory {
                credentials: CredentialEnvelopeState::Complete,
                ..empty
            }),
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
        );
        assert_eq!(
            classify_startup_inventory(StartupInventory {
                pointer_present: true,
                ..empty
            }),
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
        );
    }

    #[test]
    fn partial_or_missing_credentials_never_replace_existing_artifacts() {
        for credentials in [
            CredentialEnvelopeState::Absent,
            CredentialEnvelopeState::Partial,
        ] {
            assert_eq!(
                classify_startup_inventory(StartupInventory {
                    credentials,
                    artifacts: LibraryArtifactState::Present,
                    pointer_present: false,
                    storage_migration_pending: false,
                    restore_pending: false,
                    reset_pending: false,
                }),
                StartupDisposition::RecoveryRequired(LibraryRecoveryReason::CredentialsIncomplete)
            );
        }
    }

    #[test]
    fn partial_first_run_is_distinct_from_credentials_missing_beside_data() {
        let empty = StartupInventory {
            credentials: CredentialEnvelopeState::Partial,
            artifacts: LibraryArtifactState::Absent,
            pointer_present: false,
            storage_migration_pending: false,
            restore_pending: false,
            reset_pending: false,
        };
        assert_eq!(
            classify_startup_inventory(empty),
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::SetupInterrupted)
        );
        assert_eq!(
            classify_startup_inventory(StartupInventory {
                artifacts: LibraryArtifactState::Present,
                ..empty
            }),
            StartupDisposition::RecoveryRequired(LibraryRecoveryReason::CredentialsIncomplete)
        );
    }
}
