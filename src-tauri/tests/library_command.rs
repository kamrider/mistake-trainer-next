use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::{
    commands::{
        library::{library_context_for, problem_filter_options_for, problem_list_for},
        review::review_current_problem_for,
    },
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::problems::{
        AssetRole, CaptureAsset, CreateProblem, ProblemAnswerState, ProblemListInput,
        ProblemReviewState, ProblemStatusFilter, create_problem,
    },
    modules::review::{
        BeginExamGrading, StartExamReview, begin_exam_grading, start_exam_review_queue,
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
fn exam_current_problem_never_sends_answer_assets_during_the_answering_pass() {
    const VALID_PNG: &[u8] = include_bytes!("../icons/32x32.png");
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");
    let profile_id = runtime.active_profile().id;
    let problem = create_problem(
        &mut runtime.connection.lock().unwrap(),
        &runtime.blob_root,
        &runtime.asset_key,
        CreateProblem {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile_id.clone(),
            subject: "数学".to_owned(),
            note: "严格保密答案".to_owned(),
            assets: vec![
                CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: VALID_PNG.to_vec(),
                },
                CaptureAsset {
                    role: AssetRole::Answer,
                    media_type: "image/png".to_owned(),
                    bytes: VALID_PNG.to_vec(),
                },
            ],
            now_utc_ms: 200,
        },
    )
    .expect("problem");
    start_exam_review_queue(
        &mut runtime.connection.lock().unwrap(),
        StartExamReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![problem.id],
            now_utc_ms: 300,
        },
    )
    .expect("exam");

    let answering = serde_json::to_value(review_current_problem_for(&runtime)).unwrap();
    assert_eq!(answering["ok"], true);
    assert_eq!(answering["data"]["assets"].as_array().unwrap().len(), 1);
    assert_eq!(answering["data"]["assets"][0]["role"], "question");

    begin_exam_grading(
        &mut runtime.connection.lock().unwrap(),
        BeginExamGrading {
            account_id: runtime.account_id().to_owned(),
            profile_id,
            now_utc_ms: 400,
        },
    )
    .expect("grading");
    let grading = serde_json::to_value(review_current_problem_for(&runtime)).unwrap();
    assert_eq!(grading["data"]["assets"].as_array().unwrap().len(), 2);
    assert_eq!(grading["data"]["assets"][1]["role"], "answer");
}

#[test]
fn commands_use_runtime_identity_instead_of_accepting_account_or_profile_ids() {
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
            note: "奇函数".to_owned(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"question".to_vec(),
            }],
            now_utc_ms: 200,
        },
    )
    .expect("problem");

    let context = serde_json::to_value(library_context_for(&runtime)).expect("context json");
    let problems = serde_json::to_value(problem_list_for(
        &runtime,
        ProblemListInput {
            status: ProblemStatusFilter::Active,
            search: None,
            subjects: vec!["数学".to_owned()],
            tags: vec![],
            review_state: ProblemReviewState::Any,
            answer_state: ProblemAnswerState::Any,
            cursor: None,
        },
        300,
    ))
    .expect("problem list json");
    let filter_options = serde_json::to_value(problem_filter_options_for(
        &runtime,
        ProblemStatusFilter::Active,
    ))
    .expect("filter options json");

    assert_eq!(context["ok"], true);
    assert_eq!(context["data"]["profileName"], "本机学习档案");
    assert_eq!(context["data"]["storage"], "ready");
    assert_eq!(problems["ok"], true);
    assert_eq!(problems["data"]["items"][0]["subject"], "数学");
    assert_eq!(problems["data"]["items"][0]["questionAssetCount"], 1);
    assert_eq!(problems["data"]["nextCursor"], serde_json::Value::Null);
    assert_eq!(filter_options["ok"], true);
    assert_eq!(filter_options["data"]["subjects"][0], "数学");
}

#[test]
fn problem_list_reports_invalid_filters_as_non_retryable() {
    let directory = tempdir().expect("tempdir");
    let runtime = initialize_local_library(directory.path(), &MemorySecretStore::default(), 100)
        .expect("runtime");

    let result = serde_json::to_value(problem_list_for(
        &runtime,
        ProblemListInput {
            status: ProblemStatusFilter::Active,
            search: Some("筛".repeat(101)),
            subjects: vec![],
            tags: vec![],
            review_state: ProblemReviewState::Any,
            answer_state: ProblemAnswerState::Any,
            cursor: None,
        },
        300,
    ))
    .expect("problem list json");

    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "problem_filter_invalid");
    assert_eq!(result["error"]["retryable"], false);
}
