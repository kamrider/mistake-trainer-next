use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::infrastructure::runtime::{SecretStore, initialize_local_library};
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
    let first_profile = first.profile_id().to_owned();
    assert_eq!(first.profile_name(), "本机学习档案");
    drop(first);

    let reopened =
        initialize_local_library(directory.path(), &secrets, 200).expect("reopen local library");

    assert_eq!(reopened.account_id(), first_account);
    assert_eq!(reopened.profile_id(), first_profile);
    assert_eq!(secrets.values.lock().unwrap().len(), 3);
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
