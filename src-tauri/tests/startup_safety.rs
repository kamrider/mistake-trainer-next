use mistake_trainer_next_lib::modules::startup_safety::{
    STARTUP_FAILURE_FILE_NAME, write_startup_failure_record,
};

#[test]
fn startup_failure_record_contains_only_the_public_fixed_contract() {
    let directory = tempfile::tempdir().unwrap();
    let path = write_startup_failure_record(directory.path(), "0.1.0", 1_700_000_000_000).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "schemaVersion": 1,
            "applicationVersion": "0.1.0",
            "occurredAtUtcMs": 1_700_000_000_000_i64,
            "reasonCode": "tauri_startup_failed"
        })
    );
}

#[test]
fn a_new_failure_atomically_replaces_the_previous_sanitized_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(STARTUP_FAILURE_FILE_NAME);
    std::fs::write(
        &path,
        br#"{"internalError":"C:\\Users\\private\\library.db"}"#,
    )
    .unwrap();

    write_startup_failure_record(directory.path(), "0.1.0", 1_700_000_000_123).unwrap();

    let contents = std::fs::read_to_string(path).unwrap();
    assert!(contents.contains("\"occurredAtUtcMs\": 1700000000123"));
    assert!(!contents.contains("internalError"));
    assert!(!contents.contains("Users"));
    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        1,
        "temporary files must not remain"
    );
}
