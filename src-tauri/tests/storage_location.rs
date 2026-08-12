use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use mistake_trainer_next_lib::{
    application::startup::{
        LibraryRecoveryReason, LibraryStartup, StartupAccessUnavailable,
        initialize_configured_application_library_if_accessible,
    },
    commands::storage::reconnect_existing_library,
    infrastructure::{
        runtime::{SecretStore, initialize_local_library},
        storage_location::{
            ResolvedStorage, STORAGE_POINTER_FILE, StoragePointer, resolve_storage,
            write_storage_pointer,
        },
    },
};
use tempfile::tempdir;

#[derive(Default)]
struct MemorySecretStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecretStore {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .insert(name.to_owned(), value.to_owned());
        Ok(())
    }
}

#[test]
fn reconnect_validates_the_existing_encrypted_library_before_writing_the_pointer() {
    let control = tempdir().unwrap();
    let selected = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    let library_root = custom_library_root(selected.path());
    fs::create_dir_all(library_root.parent().unwrap()).unwrap();
    drop(initialize_local_library(&library_root, &secrets, 100).unwrap());

    reconnect_existing_library(control.path(), library_root.parent().unwrap(), &secrets).unwrap();

    assert_eq!(
        resolve_storage(control.path()).unwrap().library_root(),
        library_root
    );
}

#[test]
fn reconnect_with_the_wrong_credentials_leaves_the_pointer_unchanged() {
    let control = tempdir().unwrap();
    let selected = tempdir().unwrap();
    let original_secrets = MemorySecretStore::default();
    let library_root = custom_library_root(selected.path());
    fs::create_dir_all(library_root.parent().unwrap()).unwrap();
    drop(initialize_local_library(&library_root, &original_secrets, 100).unwrap());
    let wrong_secrets = MemorySecretStore::default();
    seed_complete_credentials(&wrong_secrets);

    assert!(
        reconnect_existing_library(
            control.path(),
            library_root.parent().unwrap(),
            &wrong_secrets,
        )
        .is_err()
    );
    assert!(!control.path().join(STORAGE_POINTER_FILE).exists());
}

fn write_pointer(control_root: &Path, pointer: &serde_json::Value) {
    fs::write(
        control_root.join(STORAGE_POINTER_FILE),
        serde_json::to_vec(pointer).unwrap(),
    )
    .unwrap();
}

fn custom_library_root(parent: &Path) -> PathBuf {
    parent.join("Mistake Trainer Next Data").join("library")
}

fn seed_complete_credentials(secrets: &MemorySecretStore) {
    secrets.set("database-key", &"11".repeat(32)).unwrap();
    secrets.set("asset-key", &"22".repeat(32)).unwrap();
    secrets
        .set("account-id", "33333333-3333-4333-8333-333333333333")
        .unwrap();
}

#[test]
fn missing_pointer_uses_only_the_existing_default_root() {
    let control = tempdir().unwrap();

    let resolved = resolve_storage(control.path()).unwrap();

    assert_eq!(
        resolved.library_root(),
        control.path().join("library").as_path(),
    );
    assert!(matches!(resolved, ResolvedStorage::Default { .. }));
    assert!(!control.path().join("library").exists());
}

#[test]
fn retained_credentials_and_missing_default_data_require_recovery_without_creating_an_empty_library()
 {
    let control = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    seed_complete_credentials(&secrets);

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 100)
            .expect("cleared data is a recoverable startup state");

    assert!(matches!(
        startup,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::LocalDataMissing)
    ));
    assert!(!control.path().join("library").exists());
}

#[test]
fn genuinely_empty_install_still_creates_the_first_default_library() {
    let control = tempdir().unwrap();
    let secrets = MemorySecretStore::default();

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 100)
            .expect("first run initializes the default library");

    assert!(matches!(startup, LibraryStartup::Ready(_)));
    assert!(control.path().join("library/library.db").is_file());
}

#[test]
fn invalid_restore_evidence_never_falls_open_into_first_run() {
    let control = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    fs::create_dir(control.path().join("restore-pending.json")).unwrap();

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 100)
            .expect("invalid restore evidence is a structured recovery state");

    assert!(matches!(
        startup,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::RestoreInterrupted)
    ));
    assert!(!control.path().join("library/library.db").exists());
}

#[test]
fn interrupted_first_run_is_recoverable_without_creating_a_library() {
    let control = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    secrets.set("database-key", &"11".repeat(32)).unwrap();

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 100)
            .expect("partial first-run credentials are a structured recovery state");

    assert!(matches!(
        startup,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::SetupInterrupted)
    ));
    assert!(!control.path().join("library/library.db").exists());
}

#[test]
fn locked_marker_never_hides_partial_credentials_beside_existing_data() {
    let control = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    fs::create_dir_all(control.path().join("library")).unwrap();
    fs::write(control.path().join("library/library.db"), b"existing").unwrap();
    secrets.set("database-key", &"11".repeat(32)).unwrap();
    secrets.set("library-lock-state", "locked").unwrap();

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 100)
            .expect("partial credentials must outrank a stale lock marker");

    assert!(matches!(
        startup,
        LibraryStartup::RecoveryRequired(LibraryRecoveryReason::CredentialsIncomplete)
    ));
}

#[test]
fn malformed_unknown_relative_and_wrong_suffix_pointers_fail_closed() {
    let control = tempdir().unwrap();
    let pointer_path = control.path().join(STORAGE_POINTER_FILE);

    fs::write(&pointer_path, b"{").unwrap();
    assert!(resolve_storage(control.path()).is_err());

    write_pointer(
        control.path(),
        &serde_json::json!({
            "schemaVersion": 1,
            "libraryRoot": custom_library_root(control.path()),
            "unexpected": true,
        }),
    );
    assert!(resolve_storage(control.path()).is_err());

    write_pointer(
        control.path(),
        &serde_json::json!({
            "schemaVersion": 1,
            "libraryRoot": "relative/library",
        }),
    );
    assert!(resolve_storage(control.path()).is_err());

    write_pointer(
        control.path(),
        &serde_json::json!({
            "schemaVersion": 1,
            "libraryRoot": control.path().join("another-name").join("library"),
        }),
    );
    assert!(resolve_storage(control.path()).is_err());

    let external = tempdir().unwrap();
    let valid_shape = custom_library_root(external.path());
    fs::create_dir_all(&valid_shape).unwrap();
    fs::write(valid_shape.join("library.db"), b"fixture").unwrap();
    write_pointer(
        control.path(),
        &serde_json::json!({
            "schemaVersion": 2,
            "libraryRoot": valid_shape,
        }),
    );
    assert!(resolve_storage(control.path()).is_err());

    fs::write(&pointer_path, vec![b' '; 64 * 1024 + 1]).unwrap();
    assert!(resolve_storage(control.path()).is_err());

    assert!(!control.path().join("library").exists());
}

#[test]
fn unavailable_custom_root_never_falls_back_to_an_existing_default_library() {
    let control = tempdir().unwrap();
    let detached = tempdir().unwrap();
    let missing_custom = custom_library_root(detached.path());
    let secrets = MemorySecretStore::default();
    let default_root = control.path().join("library");
    let default_runtime =
        initialize_local_library(&default_root, &secrets, 100).expect("seed default library");
    let default_profile = default_runtime.active_profile();
    drop(default_runtime);
    fs::remove_dir_all(detached.path()).unwrap();

    write_pointer(
        control.path(),
        &serde_json::to_value(StoragePointer {
            schema_version: 1,
            library_root: missing_custom.clone(),
        })
        .unwrap(),
    );

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 200)
            .expect("storage unavailability is a recoverable locked-shell state");

    assert!(matches!(
        startup,
        LibraryStartup::AccessUnavailable(StartupAccessUnavailable::Storage(_))
    ));
    assert!(!missing_custom.exists());
    let reopened =
        initialize_local_library(&default_root, &secrets, 300).expect("default remains untouched");
    assert_eq!(reopened.active_profile(), default_profile);
}

#[test]
fn valid_custom_pointer_opens_only_the_selected_library() {
    let control = tempdir().unwrap();
    let selected = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    let custom_root = custom_library_root(selected.path());
    fs::create_dir_all(custom_root.parent().unwrap()).unwrap();
    let custom = initialize_local_library(&custom_root, &secrets, 100).unwrap();
    let custom_profile = custom.active_profile();
    drop(custom);

    write_pointer(
        control.path(),
        &serde_json::to_value(StoragePointer {
            schema_version: 1,
            library_root: custom_root,
        })
        .unwrap(),
    );

    let startup =
        initialize_configured_application_library_if_accessible(control.path(), &secrets, 200)
            .unwrap();
    let LibraryStartup::Ready(runtime) = startup else {
        panic!("valid custom storage should open")
    };

    assert_eq!(runtime.active_profile(), custom_profile);
    assert!(!control.path().join("library").exists());
}

#[test]
fn pointer_writer_replaces_the_previous_value_and_leaves_no_temporary_file() {
    let control = tempdir().unwrap();
    let first_parent = tempdir().unwrap();
    let second_parent = tempdir().unwrap();
    let secrets = MemorySecretStore::default();
    let first_root = custom_library_root(first_parent.path());
    let second_root = custom_library_root(second_parent.path());
    fs::create_dir_all(first_root.parent().unwrap()).unwrap();
    fs::create_dir_all(second_root.parent().unwrap()).unwrap();
    drop(initialize_local_library(&first_root, &secrets, 100).unwrap());
    fs::create_dir_all(&second_root).unwrap();
    fs::copy(
        first_root.join("library.db"),
        second_root.join("library.db"),
    )
    .unwrap();

    write_storage_pointer(control.path(), &first_root).unwrap();
    assert_eq!(
        resolve_storage(control.path()).unwrap().library_root(),
        first_root
    );

    write_storage_pointer(control.path(), &second_root).unwrap();
    assert_eq!(
        resolve_storage(control.path()).unwrap().library_root(),
        second_root
    );
    assert_eq!(
        fs::read_dir(control.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() != STORAGE_POINTER_FILE)
            .count(),
        0
    );
}
