use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use super::{
    runtime::{RuntimeError, SecretStore, delete_local_credential_envelope},
    storage_location::{
        RESET_PENDING_FILE, RESTORE_PENDING_FILE, STORAGE_PENDING_FILE, STORAGE_POINTER_FILE,
        STORAGE_RECEIPT_FILE, StorageLocationError, remove_control_file, write_control_json,
    },
};

#[derive(Debug, Error)]
pub enum LibraryResetError {
    #[error("the reset journal could not be updated")]
    Storage(#[from] StorageLocationError),
    #[error("the local credential envelope could not be removed")]
    Credentials(#[from] RuntimeError),
}

impl LibraryResetError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "library_reset_storage_failed",
            Self::Credentials(_) => "library_reset_credentials_failed",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResetJournal {
    schema_version: u32,
    reason: &'static str,
}

/// Abandons only the missing library's local identity and product-owned control
/// files. The journal is removed last, so every interruption is safely retryable.
pub fn reset_missing_library(
    control_root: &Path,
    secrets: &dyn SecretStore,
) -> Result<(), LibraryResetError> {
    write_control_json(
        control_root,
        RESET_PENDING_FILE,
        &ResetJournal {
            schema_version: 1,
            reason: "user_confirmed_fresh_start",
        },
        true,
    )?;

    delete_local_credential_envelope(secrets)?;
    for file_name in [
        STORAGE_POINTER_FILE,
        STORAGE_PENDING_FILE,
        STORAGE_RECEIPT_FILE,
        RESTORE_PENDING_FILE,
    ] {
        remove_control_file(control_root, file_name)?;
    }
    remove_control_file(control_root, RESET_PENDING_FILE)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;
    use crate::infrastructure::{
        runtime::{CredentialEnvelopeState, inspect_local_credential_envelope},
        storage_location::write_storage_pointer,
    };

    #[derive(Default)]
    struct MemorySecrets {
        values: Mutex<HashMap<String, String>>,
        fail_delete_after: Mutex<Option<usize>>,
        delete_count: Mutex<usize>,
    }

    impl SecretStore for MemorySecrets {
        fn get(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self.values.lock().unwrap().get(name).cloned())
        }

        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            self.values
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            let mut count = self.delete_count.lock().unwrap();
            if self
                .fail_delete_after
                .lock()
                .unwrap()
                .is_some_and(|limit| *count == limit)
            {
                return Err("injected deletion failure".to_owned());
            }
            self.values.lock().unwrap().remove(name);
            *count += 1;
            Ok(())
        }
    }

    fn seed_complete(store: &MemorySecrets) {
        store.set("database-key", &"11".repeat(32)).unwrap();
        store.set("asset-key", &"22".repeat(32)).unwrap();
        store
            .set("account-id", "018f0f00-0000-7000-8000-000000000001")
            .unwrap();
        store
            .set("device-id", "018f0f00-0000-7000-8000-000000000002")
            .unwrap();
        store.set("library-lock-state", "unlocked").unwrap();
    }

    #[test]
    fn reset_removes_only_the_credential_envelope_and_owned_control_files() {
        let root = tempfile::tempdir().unwrap();
        let secrets = MemorySecrets::default();
        seed_complete(&secrets);
        let external = tempfile::tempdir().unwrap();
        let owned = external.path().join("Mistake Trainer Next Data/library");
        std::fs::create_dir_all(&owned).unwrap();
        std::fs::write(owned.join("library.db"), b"preserve").unwrap();
        write_storage_pointer(root.path(), &owned).unwrap();
        std::fs::write(root.path().join("unrelated.txt"), b"keep").unwrap();

        reset_missing_library(root.path(), &secrets).unwrap();

        assert_eq!(
            inspect_local_credential_envelope(&secrets).unwrap(),
            CredentialEnvelopeState::Absent
        );
        assert!(!root.path().join(STORAGE_POINTER_FILE).exists());
        assert!(!root.path().join(RESET_PENDING_FILE).exists());
        assert!(root.path().join("unrelated.txt").is_file());
        assert!(owned.is_dir());
    }

    #[test]
    fn interrupted_reset_keeps_its_journal_and_converges_on_retry() {
        let root = tempfile::tempdir().unwrap();
        let secrets = MemorySecrets::default();
        seed_complete(&secrets);
        *secrets.fail_delete_after.lock().unwrap() = Some(2);

        assert!(reset_missing_library(root.path(), &secrets).is_err());
        assert!(root.path().join(RESET_PENDING_FILE).is_file());

        *secrets.fail_delete_after.lock().unwrap() = None;
        *secrets.delete_count.lock().unwrap() = 0;
        reset_missing_library(root.path(), &secrets).unwrap();
        assert!(!root.path().join(RESET_PENDING_FILE).exists());
        assert_eq!(
            inspect_local_credential_envelope(&secrets).unwrap(),
            CredentialEnvelopeState::Absent
        );
    }
}
