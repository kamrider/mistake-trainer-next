use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::{
    commands::capture_inbox::{
        CaptureBatchCreateInput, CaptureImportBytesInput, capture_batch_create_for,
        capture_batch_detail_for, capture_batch_list_for, capture_import_bytes_for,
    },
    infrastructure::runtime::{SecretStore, initialize_local_library},
};
use tempfile::tempdir;

const VALID_PNG: &[u8] = include_bytes!("../icons/32x32.png");

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
fn public_capture_commands_use_runtime_identity_and_app_result() {
    let directory = tempdir().unwrap();
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    let created = serde_json::to_value(capture_batch_create_for(
        &runtime,
        CaptureBatchCreateInput {
            subject: "数学".to_owned(),
        },
        200,
    ))
    .unwrap();
    assert_eq!(created["ok"], true);
    let batch_id = created["data"]["id"].as_str().unwrap().to_owned();

    let imported = serde_json::to_value(capture_import_bytes_for(
        &runtime,
        CaptureImportBytesInput {
            batch_id: batch_id.clone(),
            client_upload_id: "clipboard-1".to_owned(),
            source_name: "clipboard.png".to_owned(),
            source_sequence: None,
            bytes: VALID_PNG.to_vec(),
        },
        210,
    ))
    .unwrap();
    assert_eq!(imported["ok"], true);
    assert_eq!(imported["data"]["sourceName"], "clipboard.png");

    let detail = serde_json::to_value(capture_batch_detail_for(&runtime, &batch_id)).unwrap();
    let batches = serde_json::to_value(capture_batch_list_for(&runtime)).unwrap();
    assert_eq!(detail["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(batches["data"][0]["itemCount"], 1);
    assert!(detail["data"].get("accountId").is_none());
    assert!(detail["data"].get("profileId").is_none());
}

#[test]
fn invalid_image_returns_a_stable_public_error_without_internal_details() {
    let directory = tempdir().unwrap();
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    let created = serde_json::to_value(capture_batch_create_for(
        &runtime,
        CaptureBatchCreateInput {
            subject: String::new(),
        },
        200,
    ))
    .unwrap();
    let batch_id = created["data"]["id"].as_str().unwrap();
    let result = serde_json::to_value(capture_import_bytes_for(
        &runtime,
        CaptureImportBytesInput {
            batch_id: batch_id.to_owned(),
            client_upload_id: "bad-image".to_owned(),
            source_name: "fake.png".to_owned(),
            source_sequence: None,
            bytes: b"not an image".to_vec(),
        },
        210,
    ))
    .unwrap();
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "capture_image_invalid");
    assert!(result.get("internalError").is_none());
}
