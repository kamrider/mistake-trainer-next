use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::infrastructure::runtime::{SecretStore, initialize_local_library};
use mistake_trainer_next_lib::modules::profiles::{
    CreateProfile, ProfileUseCaseError, create_profile,
};
use tempfile::tempdir;

#[derive(Default)]
struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemorySecretStore {
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
}

#[test]
fn local_library_reopens_with_the_same_identity_profile_and_encryption_keys() {
    let directory = tempdir().expect("tempdir");
    let secrets = MemorySecretStore::default();

    let first = initialize_local_library(directory.path(), &secrets, 100)
        .expect("initialize first local library");
    let first_account = first.account_id().to_owned();
    let first_profile = first.active_profile();
    let first_device = first.device_id().to_owned();
    assert_eq!(first_profile.name, "本机学习档案");
    let debug_output = format!("{first:?}");
    for secret in secrets.values.lock().unwrap().values() {
        assert!(!debug_output.contains(secret));
    }
    assert!(!debug_output.contains(&first_profile.name));
    assert!(debug_output.contains("<redacted>"));
    drop(first);

    let reopened =
        initialize_local_library(directory.path(), &secrets, 200).expect("reopen local library");

    assert_eq!(reopened.account_id(), first_account);
    assert_eq!(reopened.active_profile().id, first_profile.id);
    assert_eq!(reopened.device_id(), first_device);
    assert_eq!(secrets.values.lock().unwrap().len(), 4);
}

#[test]
fn selected_profile_survives_restart_and_forged_selection_changes_nothing() {
    let directory = tempdir().expect("tempdir");
    let secrets = MemorySecretStore::default();
    let runtime = initialize_local_library(directory.path(), &secrets, 100).unwrap();
    let original = runtime.active_profile();
    let second = {
        let mut connection = runtime.connection.lock().unwrap();
        create_profile(
            &mut connection,
            CreateProfile {
                account_id: runtime.account_id().to_owned(),
                name: "竞赛档案".to_owned(),
                now_utc_ms: 200,
            },
        )
        .unwrap()
    };

    let selected = runtime.activate_profile(&second.id, 300).unwrap();
    assert_eq!(
        (selected.id.as_str(), selected.name.as_str()),
        (second.id.as_str(), "竞赛档案")
    );
    let stored: String = runtime
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT active_profile_id FROM account_preferences WHERE account_id = ?1",
            [runtime.account_id()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, second.id);

    assert!(matches!(
        runtime.activate_profile("forged-profile", 400),
        Err(ProfileUseCaseError::NotFound)
    ));
    assert_eq!(runtime.active_profile().id, second.id);
    drop(runtime);

    let reopened = initialize_local_library(directory.path(), &secrets, 500).unwrap();
    assert_eq!(reopened.active_profile().id, second.id);
    let debug_output = format!("{reopened:?}");
    assert!(!debug_output.contains(&second.id));
    assert!(!debug_output.contains(&original.id));
    assert!(!debug_output.contains("竞赛档案"));
}

#[test]
fn malformed_stored_asset_key_is_rejected_without_overwriting_it() {
    let directory = tempdir().expect("tempdir");
    let secrets = MemorySecretStore::default();
    secrets
        .set("asset-key", "not-a-32-byte-hex-key")
        .expect("seed malformed key");

    let error = initialize_local_library(directory.path(), &secrets, 100)
        .expect_err("malformed secret must fail closed");

    assert_eq!(error.code(), "invalid_asset_key");
    assert_eq!(
        secrets.get("asset-key").unwrap().as_deref(),
        Some("not-a-32-byte-hex-key")
    );
}

#[test]
fn existing_library_with_a_missing_secret_fails_closed_without_replacement() {
    for missing_name in ["asset-key", "database-key", "account-id"] {
        let directory = tempdir().expect("tempdir");
        let secrets = MemorySecretStore::default();
        let runtime = initialize_local_library(directory.path(), &secrets, 100)
            .expect("initialize local library");
        drop(runtime);
        secrets.values.lock().unwrap().remove(missing_name);

        let error = initialize_local_library(directory.path(), &secrets, 200)
            .expect_err("existing data must never create replacement credentials");

        assert_eq!(error.code(), "library_credentials_missing");
        assert_eq!(secrets.get(missing_name).unwrap(), None);
    }
}
