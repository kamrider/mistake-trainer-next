use mistake_trainer_next_lib::modules::startup_safety::{
    STARTUP_FAILURE_FILE_NAME, StartupFailureReason, WindowsSelfCheckFailureCode,
    build_windows_self_check_report, read_startup_failure_record, write_startup_failure_record,
};
use mistake_trainer_next_lib::modules::windows_compatibility::{
    MINIMUM_WINDOWS_BUILD, WindowsCompatibilityStatus, WindowsSupportLevel,
};

fn windows_status(
    support_level: WindowsSupportLevel,
    webview2_version: Option<&str>,
) -> WindowsCompatibilityStatus {
    WindowsCompatibilityStatus {
        support_level,
        supported: support_level != WindowsSupportLevel::Unsupported,
        os_name: "Windows 11 Pro".to_owned(),
        display_version: "24H2".to_owned(),
        build_number: 26_100,
        update_build_revision: 1,
        process_architecture: "x86_64".to_owned(),
        native_architecture: "x86_64".to_owned(),
        webview2_version: webview2_version.map(str::to_owned),
        minimum_windows_build: MINIMUM_WINDOWS_BUILD,
        summary: "test".to_owned(),
    }
}

#[test]
fn self_check_requires_supported_windows_and_webview2() {
    let ready = build_windows_self_check_report(
        "1.2.3",
        100,
        windows_status(WindowsSupportLevel::Supported, Some("150.0.0.0")),
    );
    assert!(ready.ready);
    assert!(ready.failure_codes.is_empty());

    let missing_runtime = build_windows_self_check_report(
        "1.2.3",
        100,
        windows_status(WindowsSupportLevel::Supported, None),
    );
    assert!(!missing_runtime.ready);
    assert_eq!(
        missing_runtime.failure_codes,
        vec![WindowsSelfCheckFailureCode::Webview2RuntimeMissing]
    );

    let unsupported = build_windows_self_check_report(
        "1.2.3",
        100,
        windows_status(WindowsSupportLevel::Unsupported, Some("150.0.0.0")),
    );
    assert!(!unsupported.ready);
    assert_eq!(
        unsupported.failure_codes,
        vec![WindowsSelfCheckFailureCode::WindowsUnsupported]
    );
}

#[test]
fn self_check_schema_exposes_only_fixed_readiness_codes() {
    let report = build_windows_self_check_report(
        "1.2.3",
        100,
        windows_status(WindowsSupportLevel::Supported, None),
    );
    let serialized = serde_json::to_value(report).unwrap();

    assert_eq!(serialized["schemaVersion"], 2);
    assert_eq!(serialized["ready"], false);
    assert_eq!(
        serialized["failureCodes"],
        serde_json::json!(["webview2_runtime_missing"])
    );
    assert!(serialized.get("error").is_none());
    assert!(serialized.get("path").is_none());
}

#[test]
fn startup_failure_record_contains_only_the_public_fixed_contract() {
    let directory = tempfile::tempdir().unwrap();
    let path = write_startup_failure_record(
        directory.path(),
        "0.1.0",
        1_700_000_000_000,
        StartupFailureReason::RustPanic,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "schemaVersion": 1,
            "applicationVersion": "0.1.0",
            "occurredAtUtcMs": 1_700_000_000_000_i64,
            "reasonCode": "rust_panic"
        })
    );
    assert!(!value.to_string().contains("C:\\Users\\private"));
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

    write_startup_failure_record(
        directory.path(),
        "0.1.0",
        1_700_000_000_123,
        StartupFailureReason::TauriStartupFailed,
    )
    .unwrap();

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

#[test]
fn startup_failure_reader_accepts_only_the_bounded_known_schema() {
    let directory = tempfile::tempdir().unwrap();

    assert_eq!(read_startup_failure_record(directory.path()).unwrap(), None);

    write_startup_failure_record(
        directory.path(),
        "0.1.0",
        1_700_000_000_456,
        StartupFailureReason::RustPanic,
    )
    .unwrap();
    let record = read_startup_failure_record(directory.path())
        .unwrap()
        .expect("known record");
    assert_eq!(record.reason_code, StartupFailureReason::RustPanic);

    let path = directory.path().join(STARTUP_FAILURE_FILE_NAME);
    std::fs::write(&path, b"{not-json").unwrap();
    assert_eq!(read_startup_failure_record(directory.path()).unwrap(), None);

    std::fs::write(
        &path,
        br#"{"schemaVersion":1,"applicationVersion":"0.1.0","occurredAtUtcMs":1,"reasonCode":"unknown"}"#,
    )
    .unwrap();
    assert_eq!(read_startup_failure_record(directory.path()).unwrap(), None);

    std::fs::write(
        &path,
        br#"{"schemaVersion":99,"applicationVersion":"0.1.0","occurredAtUtcMs":1,"reasonCode":"rust_panic"}"#,
    )
    .unwrap();
    assert_eq!(read_startup_failure_record(directory.path()).unwrap(), None);

    std::fs::write(&path, vec![b'x'; 4_097]).unwrap();
    assert_eq!(read_startup_failure_record(directory.path()).unwrap(), None);
}
