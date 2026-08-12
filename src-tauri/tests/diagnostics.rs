use std::{fs, sync::Mutex};

use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        diagnostics::{DiagnosticContext, DiagnosticStorageKind, export_diagnostic_report},
        startup_safety::{StartupFailureReason, StartupFailureRecord},
        windows_compatibility::{WindowsCompatibilityFacts, assess_windows_compatibility},
    },
};
use rusqlite::params;
use serde_json::Value;
use tempfile::tempdir;

const NOW_UTC_MS: i64 = 1_700_000_000_000;
const ACCOUNT_SENTINEL: &str = "SECRET_ACCOUNT_0b2f79";
const PROFILE_SENTINEL: &str = "SECRET_PROFILE_18a031";
const DEVICE_SENTINEL: &str = "SECRET_DEVICE_a61c20";
const SUBJECT_SENTINEL: &str = "SECRET_SUBJECT_2169f2";
const NOTE_SENTINEL: &str = "SECRET_NOTE_f728a0";
const TAG_SENTINEL: &str = "SECRET_TAG_29431e";
const PATH_SENTINEL: &str = "SECRET_PATH_13d4b1.jpeg";
const PAYLOAD_SENTINEL: &str = "SECRET_PAYLOAD_b650cf";
const FILE_SENTINEL: &str = "SECRET_FILENAME_1ad02e.jpg";

fn supported_windows()
-> mistake_trainer_next_lib::modules::windows_compatibility::WindowsCompatibilityStatus {
    assess_windows_compatibility(WindowsCompatibilityFacts {
        os_name: "Windows 11 Pro".to_owned(),
        display_version: "24H2".to_owned(),
        build_number: 26_100,
        update_build_revision: 4_200,
        process_architecture: "x86_64".to_owned(),
        native_architecture: "x86_64".to_owned(),
        webview2_version: Some("140.0.0.0".to_owned()),
    })
}

#[test]
fn report_contains_only_fixed_aggregates_and_no_user_content() {
    let library = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&library.path().join("library.db"), "diagnostic-key").unwrap();
    run_migrations(&mut connection).unwrap();

    connection
        .execute(
            "INSERT INTO learner_profiles(
                id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES(?1, ?2, 'PRIVATE_LEARNER_NAME', 1, 1, 1)",
            params![PROFILE_SENTINEL, ACCOUNT_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO problems(
                id, account_id, profile_id, subject, tags_json, note, status,
                created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('problem-private', ?1, ?2, ?3, ?4, ?5, 'active', 2, 2, 1)",
            params![
                ACCOUNT_SENTINEL,
                PROFILE_SENTINEL,
                SUBJECT_SENTINEL,
                format!("[\"{TAG_SENTINEL}\"]"),
                NOTE_SENTINEL,
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO assets(
                id, account_id, plaintext_sha256, encrypted_path, byte_length,
                media_type, created_at_utc_ms
             ) VALUES('asset-private', ?1, 'private-sha', ?2, 10, 'image/jpeg', 2)",
            params![ACCOUNT_SENTINEL, PATH_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_events(
                id, account_id, profile_id, problem_id, device_id, rating,
                duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version
             ) VALUES('review-private', ?1, ?2, 'problem-private', ?3, 'again',
                400, 3, 'fsrs-6', 'default')",
            params![ACCOUNT_SENTINEL, PROFILE_SENTINEL, DEVICE_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO export_snapshots(
                id, account_id, profile_id, title, problem_ids_json,
                configuration_json, created_at_utc_ms, revision
             ) VALUES('export-private', ?1, ?2, 'PRIVATE_EXPORT_TITLE',
                '[\"problem-private\"]', '{}', 4, 1)",
            params![ACCOUNT_SENTINEL, PROFILE_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO capture_batches(
                id, account_id, profile_id, subject, state,
                created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('batch-private', ?1, ?2, ?3, 'organizing', 5, 5, 1)",
            params![ACCOUNT_SENTINEL, PROFILE_SENTINEL, SUBJECT_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO capture_items(
                id, batch_id, asset_id, client_upload_id, source_name,
                source_sequence, width, height, created_at_utc_ms
             ) VALUES('item-private', 'batch-private', 'asset-private',
                'upload-private', ?1, 0, 100, 100, 5)",
            [FILE_SENTINEL],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_operations(
                id, account_id, profile_id, entity_type, entity_id, operation,
                payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
             ) VALUES('operation-private', ?1, ?2, 'problem', 'problem-private',
                'upsert', ?3, 'pending', 0, 6, 6)",
            params![
                ACCOUNT_SENTINEL,
                PROFILE_SENTINEL,
                format!("{{\"private\":\"{PAYLOAD_SENTINEL}\"}}"),
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
                id, account_id, profile_id, entity_type, entity_id, field_name,
                local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES('conflict-private', ?1, ?2, 'problem', 'problem-private',
                'note', ?3, '\"remote-private\"', 1, 7)",
            params![
                ACCOUNT_SENTINEL,
                PROFILE_SENTINEL,
                format!("\"{NOTE_SENTINEL}\""),
            ],
        )
        .unwrap();

    let windows = supported_windows();
    let startup_failure = StartupFailureRecord {
        schema_version: 1,
        application_version: "0.1.0-test".to_owned(),
        occurred_at_utc_ms: NOW_UTC_MS - 1_000,
        reason_code: StartupFailureReason::RustPanic,
    };
    let receipt = export_diagnostic_report(
        &Mutex::new(connection),
        destination.path(),
        DiagnosticContext {
            app_version: "0.1.0-test",
            storage_kind: DiagnosticStorageKind::Custom,
            now_utc_ms: NOW_UTC_MS,
            windows_compatibility: &windows,
            startup_failure: Some(&startup_failure),
        },
    )
    .unwrap();

    assert_eq!(receipt.generated_at_utc_ms, NOW_UTC_MS as f64);
    assert_eq!(receipt.warning_count, 1);
    assert!(
        receipt
            .file_label
            .starts_with("Mistake-Trainer-Diagnostics-")
    );
    assert!(receipt.file_label.ends_with(".json"));
    assert!(
        !receipt
            .file_label
            .contains(library.path().to_string_lossy().as_ref())
    );

    let bytes = fs::read(destination.path().join(&receipt.file_label)).unwrap();
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["schemaVersion"], 3);
    assert_eq!(report["reportId"], receipt.report_id);
    assert_eq!(report["generatedAtUtcMs"], NOW_UTC_MS);
    assert_eq!(report["application"]["name"], "Mistake Trainer Next");
    assert_eq!(report["application"]["version"], "0.1.0-test");
    assert_eq!(report["application"]["platform"], std::env::consts::OS);
    assert_eq!(
        report["application"]["architecture"],
        std::env::consts::ARCH
    );
    assert_eq!(
        report["application"]["windows"]["supportLevel"],
        "supported"
    );
    assert_eq!(report["application"]["windows"]["buildNumber"], 26_100);
    assert_eq!(
        report["application"]["windows"]["webview2Version"],
        "140.0.0.0"
    );
    assert_eq!(
        report["application"]["lastStartupFailure"],
        serde_json::json!({
            "schemaVersion": 1,
            "applicationVersion": "0.1.0-test",
            "occurredAtUtcMs": NOW_UTC_MS - 1_000,
            "reasonCode": "rust_panic"
        })
    );
    assert_eq!(report["library"]["storageKind"], "custom");
    assert_eq!(report["library"]["schemaVersion"], 18);
    assert_eq!(report["library"]["integrity"], "ok");
    assert_eq!(report["library"]["profileCount"], 1);
    assert_eq!(report["library"]["problemCount"], 1);
    assert_eq!(report["library"]["assetCount"], 1);
    assert_eq!(report["library"]["captureBatchCount"], 1);
    assert_eq!(report["library"]["reviewEventCount"], 1);
    assert_eq!(report["library"]["exportSnapshotCount"], 1);
    assert_eq!(report["sync"]["pendingOperationCount"], 1);
    assert_eq!(report["sync"]["failedOperationCount"], 0);
    assert_eq!(report["sync"]["unresolvedConflictCount"], 1);
    assert_eq!(
        report["warnings"],
        serde_json::json!([{ "code": "previous_startup_failure_detected" }])
    );

    let serialized = String::from_utf8(bytes).unwrap();
    for forbidden in [
        ACCOUNT_SENTINEL,
        PROFILE_SENTINEL,
        DEVICE_SENTINEL,
        SUBJECT_SENTINEL,
        NOTE_SENTINEL,
        TAG_SENTINEL,
        PATH_SENTINEL,
        PAYLOAD_SENTINEL,
        FILE_SENTINEL,
        "PRIVATE_LEARNER_NAME",
        "PRIVATE_EXPORT_TITLE",
        library.path().to_string_lossy().as_ref(),
        destination.path().to_string_lossy().as_ref(),
    ] {
        assert!(
            !serialized.contains(forbidden),
            "diagnostic report leaked forbidden value: {forbidden}"
        );
    }
}

#[test]
fn report_rejects_a_non_directory_destination() {
    let library = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let not_a_directory = destination.path().join("plain-file");
    fs::write(&not_a_directory, b"not a directory").unwrap();
    let mut connection =
        open_encrypted_database(&library.path().join("library.db"), "diagnostic-key").unwrap();
    run_migrations(&mut connection).unwrap();

    let windows = supported_windows();
    let error = export_diagnostic_report(
        &Mutex::new(connection),
        &not_a_directory,
        DiagnosticContext {
            app_version: "0.1.0-test",
            storage_kind: DiagnosticStorageKind::Default,
            now_utc_ms: NOW_UTC_MS,
            windows_compatibility: &windows,
            startup_failure: None,
        },
    )
    .unwrap_err();

    assert_eq!(error.code(), "invalid_destination");
    assert!(destination.path().join("plain-file").is_file());
}

#[test]
fn report_uses_only_fixed_windows_compatibility_warning_codes() {
    let library = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&library.path().join("library.db"), "diagnostic-key").unwrap();
    run_migrations(&mut connection).unwrap();
    let windows = assess_windows_compatibility(WindowsCompatibilityFacts {
        os_name: "Windows".to_owned(),
        display_version: "unknown".to_owned(),
        build_number: 17_000,
        update_build_revision: 0,
        process_architecture: "x86".to_owned(),
        native_architecture: "x86".to_owned(),
        webview2_version: None,
    });

    let receipt = export_diagnostic_report(
        &Mutex::new(connection),
        destination.path(),
        DiagnosticContext {
            app_version: "0.1.0-test",
            storage_kind: DiagnosticStorageKind::Default,
            now_utc_ms: NOW_UTC_MS,
            windows_compatibility: &windows,
            startup_failure: None,
        },
    )
    .unwrap();
    let report: Value =
        serde_json::from_slice(&fs::read(destination.path().join(receipt.file_label)).unwrap())
            .unwrap();

    assert_eq!(
        report["warnings"],
        serde_json::json!([
            { "code": "windows_release_unsupported" },
            { "code": "webview2_runtime_not_detected" }
        ])
    );
}
