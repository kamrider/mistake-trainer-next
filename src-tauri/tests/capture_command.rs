use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::{
    commands::capture::{CaptureCommitInput, capture_commit_for},
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::capture::{CaptureStage, stage_image_bytes},
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
fn commit_uses_only_staged_ids_and_clears_them_after_atomic_persistence() {
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    let stage = CaptureStage::default();
    let question = stage_image_bytes(&stage, "question.png", "question", VALID_PNG.to_vec())
        .expect("question");
    let answer =
        stage_image_bytes(&stage, "answer.png", "answer", VALID_PNG.to_vec()).expect("answer");

    let result = serde_json::to_value(capture_commit_for(
        &runtime,
        &stage,
        CaptureCommitInput {
            subject: "数学".to_owned(),
            note: "检查定义域".to_owned(),
            staged_asset_ids: vec![question.id, answer.id],
        },
        200,
    ))
    .expect("result json");

    assert_eq!(result["ok"], true);
    assert_eq!(stage.len().expect("stage"), 0);
    let connection = runtime.connection.lock().unwrap();
    let problem_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM problems", [], |row| row.get(0))
        .expect("problem count");
    let link_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM problem_assets", [], |row| row.get(0))
        .expect("link count");
    assert_eq!(problem_count, 1);
    assert_eq!(link_count, 2);
}

#[test]
fn unknown_staged_ids_leave_the_database_and_existing_stage_unchanged() {
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    let stage = CaptureStage::default();
    stage_image_bytes(&stage, "question.png", "question", VALID_PNG.to_vec()).expect("question");

    let result = serde_json::to_value(capture_commit_for(
        &runtime,
        &stage,
        CaptureCommitInput {
            subject: "数学".to_owned(),
            note: String::new(),
            staged_asset_ids: vec!["not-a-stage-id".to_owned()],
        },
        200,
    ))
    .expect("result json");

    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "staged_asset_missing");
    assert_eq!(stage.len().expect("stage"), 1);
    let connection = runtime.connection.lock().unwrap();
    let problem_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM problems", [], |row| row.get(0))
        .expect("problem count");
    assert_eq!(problem_count, 0);
}
