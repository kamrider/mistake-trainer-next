use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use mistake_trainer_next_lib::{
    commands::review_history::{ReviewHistoryInput, review_history_list_for},
    infrastructure::runtime::{SecretStore, initialize_local_library},
    modules::review_history::ReviewHistoryRange,
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
fn history_reads_wait_for_an_active_profile_transition() {
    let directory = tempdir().unwrap();
    let runtime = Arc::new(
        initialize_local_library(directory.path(), &MemorySecretStore::default(), 100).unwrap(),
    );
    let transition = runtime.lock_profile_transition();
    let worker_runtime = Arc::clone(&runtime);
    let (sent, received) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        sent.send(review_history_list_for(
            &worker_runtime,
            ReviewHistoryInput {
                range: ReviewHistoryRange::All,
                rating: None,
                subject: None,
                search: String::new(),
                cursor: None,
                limit: 20,
            },
            200,
        ))
        .unwrap();
    });

    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    drop(transition);
    let result = received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        result,
        mistake_trainer_next_lib::application::result::AppResult::Success { .. }
    ));
    worker.join().unwrap();
}
