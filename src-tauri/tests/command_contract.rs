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
