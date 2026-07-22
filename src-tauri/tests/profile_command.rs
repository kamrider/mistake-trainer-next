use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use mistake_trainer_next_lib::{
    application::result::AppResult,
    commands::profiles::{
        ProfileDeleteInput, ProfileNameInput, ProfileRenameInput, profile_create_for,
        profile_delete_for, profile_list_for, profile_rename_for, profile_select_for,
    },
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::capture_lan::CaptureLanManager,
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
fn create_select_and_rename_return_a_private_profile_overview() {
    let directory = tempdir().unwrap();
    let runtime =
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap();
    let manager = CaptureLanManager::default();
    let original_id = runtime.active_profile().id;

    let created = profile_create_for(
        &runtime,
        &manager,
        ProfileNameInput {
            name: "  竞赛档案  ".to_owned(),
        },
        200,
    );
    let created_json = serde_json::to_value(&created).unwrap();
    assert_eq!(created_json["ok"], true);
    assert_eq!(created_json["data"]["profiles"][1]["name"], "竞赛档案");
    assert!(
        created_json["data"]["profiles"][0]
            .get("accountId")
            .is_none()
    );
    let created_id = created_json["data"]["activeProfileId"]
        .as_str()
        .unwrap()
        .to_owned();

    let renamed = profile_rename_for(
        &runtime,
        ProfileRenameInput {
            profile_id: created_id.clone(),
            name: "竞赛强化".to_owned(),
        },
        300,
    );
    let renamed_json = serde_json::to_value(renamed).unwrap();
    assert_eq!(renamed_json["data"]["profiles"][1]["name"], "竞赛强化");
    assert_eq!(runtime.active_profile().name, "竞赛强化");

    let selected = profile_select_for(&runtime, &manager, original_id.clone(), 400);
    let selected_json = serde_json::to_value(selected).unwrap();
    assert_eq!(selected_json["data"]["activeProfileId"], original_id);
    assert_eq!(runtime.active_profile().id, original_id);
}

#[test]
fn duplicate_names_and_forged_selections_do_not_change_the_active_profile() {
    let directory = tempdir().unwrap();
    let runtime =
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap();
    let manager = CaptureLanManager::default();
    let original = runtime.active_profile();

    let duplicate = profile_create_for(
        &runtime,
        &manager,
        ProfileNameInput {
            name: original.name.clone(),
        },
        200,
    );
    assert_failure_code(&duplicate, "profile_name_duplicate");
    assert_eq!(runtime.active_profile(), original);

    let forged = profile_select_for(&runtime, &manager, "forged-profile".to_owned(), 300);
    assert_failure_code(&forged, "profile_not_found");
    assert_eq!(runtime.active_profile(), original);

    let overview = serde_json::to_value(profile_list_for(&runtime)).unwrap();
    assert_eq!(overview["data"]["profiles"].as_array().unwrap().len(), 1);
}

#[test]
fn selection_waits_for_an_in_flight_lan_transition() {
    let directory = tempdir().unwrap();
    let runtime = Arc::new(
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap(),
    );
    let manager = Arc::new(CaptureLanManager::default());
    let target = {
        let created = profile_create_for(
            &runtime,
            &manager,
            ProfileNameInput {
                name: "第二档案".to_owned(),
            },
            200,
        );
        serde_json::to_value(created).unwrap()["data"]["activeProfileId"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let original = serde_json::to_value(profile_list_for(&runtime)).unwrap()["data"]["profiles"][0]
        ["id"]
        .as_str()
        .unwrap()
        .to_owned();
    profile_select_for(&runtime, &manager, original, 300);

    let transition = runtime.lock_profile_transition();
    let (sent, received) = mpsc::channel();
    let worker_runtime = Arc::clone(&runtime);
    let worker_manager = Arc::clone(&manager);
    let worker = std::thread::spawn(move || {
        let result = profile_select_for(&worker_runtime, &worker_manager, target, 400);
        sent.send(result).unwrap();
    });

    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    drop(transition);
    let result = received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(serde_json::to_value(result).unwrap()["ok"], true);
    worker.join().unwrap();
}

#[test]
fn deletion_requires_the_exact_name_and_removes_only_committed_orphan_blobs() {
    let directory = tempdir().unwrap();
    let runtime =
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap();
    let manager = CaptureLanManager::default();
    let original = runtime.active_profile();
    let created = profile_create_for(
        &runtime,
        &manager,
        ProfileNameInput {
            name: "竞赛强化".to_owned(),
        },
        200,
    );
    let created_id = serde_json::to_value(created).unwrap()["data"]["activeProfileId"]
        .as_str()
        .unwrap()
        .to_owned();
    let relative = "aa/delete-with-profile.enc";
    std::fs::create_dir_all(runtime.blob_root.join("aa")).unwrap();
    std::fs::write(runtime.blob_root.join(relative), b"encrypted fixture").unwrap();
    {
        let connection = runtime.connection.lock().unwrap();
        connection.execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES('018f0000-0000-7000-8000-000000000041', ?1, ?2, ?3, 17, 'image/png', 210)",
            rusqlite::params![runtime.account_id(), "d".repeat(64), relative],
        ).unwrap();
        connection.execute(
            "INSERT INTO problems(id, account_id, profile_id, subject, created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES('018f0000-0000-7000-8000-000000000042', ?1, ?2, '数学', 210, 210, 1)",
            rusqlite::params![runtime.account_id(), created_id],
        ).unwrap();
        connection.execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position)
             VALUES('018f0000-0000-7000-8000-000000000042', '018f0000-0000-7000-8000-000000000041', 'question', 0)",
            [],
        ).unwrap();
    }

    let mismatch = profile_delete_for(
        &runtime,
        &manager,
        ProfileDeleteInput {
            profile_id: created_id.clone(),
            confirmation_name: "竞赛".to_owned(),
        },
        300,
    );
    assert_failure_code(&mismatch, "profile_delete_confirmation_mismatch");
    assert!(runtime.blob_root.join(relative).exists());
    assert_eq!(runtime.active_profile().id, created_id);

    let deleted = profile_delete_for(
        &runtime,
        &manager,
        ProfileDeleteInput {
            profile_id: created_id,
            confirmation_name: "竞赛强化".to_owned(),
        },
        400,
    );
    let deleted_json = serde_json::to_value(deleted).unwrap();
    assert_eq!(deleted_json["ok"], true);
    assert_eq!(deleted_json["data"]["activeProfileId"], original.id);
    assert_eq!(
        deleted_json["data"]["profiles"].as_array().unwrap().len(),
        1
    );
    assert_eq!(runtime.active_profile(), original);
    assert!(!runtime.blob_root.join(relative).exists());
}

#[test]
fn the_last_profile_cannot_be_deleted_through_the_command() {
    let directory = tempdir().unwrap();
    let runtime =
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap();
    let manager = CaptureLanManager::default();
    let only = runtime.active_profile();

    let result = profile_delete_for(
        &runtime,
        &manager,
        ProfileDeleteInput {
            profile_id: only.id,
            confirmation_name: only.name,
        },
        200,
    );

    assert_failure_code(&result, "profile_last_cannot_delete");
    assert_eq!(
        serde_json::to_value(profile_list_for(&runtime)).unwrap()["data"]["profiles"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

fn assert_failure_code<T>(result: &AppResult<T>, expected: &str) {
    match result {
        AppResult::Failure { error, .. } => assert_eq!(error.code, expected),
        AppResult::Success { .. } => panic!("expected {expected} failure"),
    }
}
