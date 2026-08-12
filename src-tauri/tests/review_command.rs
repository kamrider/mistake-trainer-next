use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::{
    commands::review::{ReviewQuickStartInput, review_quick_start_for},
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::{
        problems::{AssetRole, CaptureAsset, CreateProblem, create_problem},
        review::QuickReviewPreset,
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
fn quick_start_uses_runtime_identity_and_returns_persisted_overview() {
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    create_problem(
        &mut runtime.connection.lock().unwrap(),
        &runtime.blob_root,
        &runtime.asset_key,
        CreateProblem {
            account_id: runtime.account_id().to_owned(),
            profile_id: runtime.active_profile().id,
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"quick-command-question".to_vec(),
            }],
            now_utc_ms: 200,
        },
    )
    .expect("problem");

    let result = serde_json::to_value(review_quick_start_for(
        &runtime,
        ReviewQuickStartInput {
            preset: QuickReviewPreset::FiveMinutes,
            subject: Some("数学".to_owned()),
            tag: None,
        },
        300,
    ))
    .expect("result json");

    assert_eq!(result["ok"], true);
    assert_eq!(result["data"]["mode"], "manual");
    assert_eq!(result["data"]["totalCount"], 1);
    assert!(result["data"]["sessionId"].is_string());
}

#[test]
fn quick_start_maps_empty_candidates_to_actionable_non_retryable_error() {
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");

    let result = serde_json::to_value(review_quick_start_for(
        &runtime,
        ReviewQuickStartInput {
            preset: QuickReviewPreset::RecentlyForgotten,
            subject: None,
            tag: None,
        },
        300,
    ))
    .expect("result json");

    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "review_quick_empty");
    assert_eq!(result["error"]["retryable"], false);
}
