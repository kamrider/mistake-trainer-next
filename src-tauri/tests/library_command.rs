use std::{collections::HashMap, sync::Mutex};

use mistake_trainer_next_lib::{
    commands::library::{library_context_for, problem_list_for},
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::problems::{
        AssetRole, CaptureAsset, CreateProblem, ProblemStatusFilter, create_problem,
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
        ProblemStatusFilter::Active,
        None,
    ))
    .expect("problem list json");

    assert_eq!(context["ok"], true);
    assert_eq!(context["data"]["profileName"], "本机学习档案");
    assert_eq!(context["data"]["storage"], "ready");
    assert_eq!(problems["ok"], true);
    assert_eq!(problems["data"][0]["subject"], "数学");
    assert_eq!(problems["data"][0]["questionAssetCount"], 1);
}
