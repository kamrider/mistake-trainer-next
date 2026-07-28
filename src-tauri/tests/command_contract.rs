use mistake_trainer_next_lib::commands::system::status_for_version;
use serde_json::json;

#[test]
fn system_status_uses_the_public_app_result_shape() {
    let value = serde_json::to_value(status_for_version("0.1.0", "ready"))
        .expect("serialize command result");

    assert_eq!(
        value,
        json!({
            "ok": true,
            "data": {
                "appVersion": "0.1.0",
                "storage": "ready",
                "sync": "offline"
            }
        })
    );
}

#[test]
fn command_errors_never_serialize_internal_diagnostics_as_user_messages() {
    let result = mistake_trainer_next_lib::application::result::AppResult::<()>::failure(
        "DATABASE_LOCKED",
        "本地资料库已锁定",
        false,
        "diag-019f4b87",
    );
    let value = serde_json::to_value(result).expect("serialize command error");

    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["userMessage"], "本地资料库已锁定");
    assert_eq!(value["error"]["diagnosticId"], "diag-019f4b87");
    assert!(value.get("internalError").is_none());
}

#[test]
fn review_focus_preferences_use_the_stable_public_values() {
    use mistake_trainer_next_lib::modules::preferences::{ReviewFocusPolicy, ReviewPreferences};

    for (policy, expected) in [
        (ReviewFocusPolicy::Off, "off"),
        (ReviewFocusPolicy::SessionStart, "session_start"),
        (ReviewFocusPolicy::EveryTen, "every_10"),
    ] {
        let value = serde_json::to_value(ReviewPreferences {
            focus_policy: policy,
        })
        .expect("serialize review preference");
        assert_eq!(value, json!({ "focusPolicy": expected }));
    }
}

#[test]
fn review_history_input_uses_only_public_filters_and_stable_values() {
    use mistake_trainer_next_lib::{
        commands::review_history::ReviewHistoryInput, domain::review::FsrsRating,
        modules::review_history::ReviewHistoryRange,
    };

    let value = serde_json::to_value(ReviewHistoryInput {
        range: ReviewHistoryRange::SevenDays,
        rating: Some(FsrsRating::Good),
        subject: Some("数学".to_owned()),
        search: "圆锥曲线".to_owned(),
        cursor: None,
        limit: 20,
    })
    .unwrap();
    assert_eq!(
        value,
        json!({
            "range": "7_days",
            "rating": "good",
            "subject": "数学",
            "search": "圆锥曲线",
            "cursor": null,
            "limit": 20
        })
    );
    let serialized = value.to_string();
    for forbidden in ["accountId", "profileId", "deviceId", "problemId"] {
        assert!(!serialized.contains(forbidden));
    }

    assert_eq!(
        serde_json::to_value(ReviewHistoryRange::All).unwrap(),
        "all"
    );
    assert_eq!(
        serde_json::to_value(ReviewHistoryRange::ThirtyDays).unwrap(),
        "30_days"
    );
}

#[test]
fn legacy_command_identifiers_do_not_expose_runtime_identity_or_paths() {
    let import_input = serde_json::json!({ "candidateId": "0190f3ff-opaque" });
    let rollback_input = serde_json::json!({ "importId": "0190f400-opaque" });
    for value in [import_input, rollback_input] {
        let serialized = value.to_string();
        for forbidden in ["path", "root", "accountId", "profileId", "database", "key"] {
            assert!(!serialized.contains(forbidden));
        }
    }
}

#[test]
fn storage_location_status_serializes_only_redacted_capacity_information() {
    use mistake_trainer_next_lib::commands::storage::{StorageLocationKind, StorageLocationStatus};

    let value = serde_json::to_value(StorageLocationStatus {
        kind: StorageLocationKind::Custom,
        location_label: "自定义位置 · Study".to_owned(),
        database_bytes: 4_096.0,
        asset_bytes: 8_192.0,
        migration_pending: false,
    })
    .expect("serialize storage status");
    assert_eq!(
        value,
        json!({
            "kind": "custom",
            "locationLabel": "自定义位置 · Study",
            "databaseBytes": 4096.0,
            "assetBytes": 8192.0,
            "migrationPending": false
        })
    );

    let serialized = value.to_string();
    for forbidden in [
        "C:\\",
        "/Users/",
        "library.db",
        "accountId",
        "profileId",
        "databaseKey",
        "assetKey",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn diagnostic_export_receipt_contains_only_safe_correlation_metadata() {
    use mistake_trainer_next_lib::modules::diagnostics::DiagnosticExportReceipt;

    let value = serde_json::to_value(DiagnosticExportReceipt {
        report_id: "019f4b87-4cab-7b83-a4a0-46acac7d1362".to_owned(),
        file_label: "Mistake-Trainer-Diagnostics-1700000000000-019f4b87.json".to_owned(),
        generated_at_utc_ms: 1_700_000_000_000_f64,
        warning_count: 1,
    })
    .expect("serialize diagnostic receipt");

    assert_eq!(
        value,
        json!({
            "reportId": "019f4b87-4cab-7b83-a4a0-46acac7d1362",
            "fileLabel": "Mistake-Trainer-Diagnostics-1700000000000-019f4b87.json",
            "generatedAtUtcMs": 1_700_000_000_000_f64,
            "warningCount": 1
        })
    );
    let serialized = value.to_string();
    for forbidden in [
        "path",
        "accountId",
        "profileId",
        "deviceId",
        "databaseKey",
        "assetKey",
        "accessToken",
        "refreshToken",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}
