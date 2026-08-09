use serde::Serialize;
use specta::Type;
use tauri::AppHandle;
#[cfg(windows)]
use tauri::Manager as _;
use uuid::Uuid;

use crate::application::result::AppResult;

static UPDATE_OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowsUpdateStatus {
    pub enabled: bool,
    pub current_version: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowsUpdateCheckReport {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowsUpdateInstallReceipt {
    pub accepted_version: String,
}

#[tauri::command]
#[specta::specta]
pub async fn windows_update_status(app: AppHandle) -> Result<AppResult<WindowsUpdateStatus>, ()> {
    Ok(AppResult::success(WindowsUpdateStatus {
        enabled: cfg!(windows) && updater_is_configured(&app),
        current_version: app.package_info().version.to_string(),
    }))
}

#[tauri::command]
#[specta::specta]
pub async fn windows_update_check(
    app: AppHandle,
) -> Result<AppResult<WindowsUpdateCheckReport>, ()> {
    if !updater_is_configured(&app) {
        return Ok(update_failure(
            "update_disabled",
            "当前安装包未接入自动更新，请使用新的正式安装包升级。",
            false,
        ));
    }
    let Ok(_operation) = UPDATE_OPERATION.try_lock() else {
        return Ok(update_failure(
            "update_busy",
            "另一项更新操作正在进行，请稍候。",
            true,
        ));
    };

    #[cfg(windows)]
    {
        use tauri_plugin_updater::UpdaterExt as _;

        let update = match app.updater() {
            Ok(updater) => match updater.check().await {
                Ok(update) => update,
                Err(_) => {
                    return Ok(update_failure(
                        "update_check_failed",
                        "暂时无法检查更新，请稍后重试。",
                        true,
                    ));
                }
            },
            Err(_) => {
                return Ok(update_failure(
                    "update_check_failed",
                    "暂时无法检查更新，请稍后重试。",
                    true,
                ));
            }
        };
        let current_version = app.package_info().version.to_string();
        Ok(AppResult::success(match update {
            Some(update) if valid_expected_version(&update.version) => WindowsUpdateCheckReport {
                available: true,
                current_version,
                version: Some(update.version),
                published_at: update.date.and_then(|date| {
                    date.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
            },
            Some(_) => {
                return Ok(update_failure(
                    "update_check_failed",
                    "更新信息没有通过格式校验，请稍后重试。",
                    true,
                ));
            }
            None => WindowsUpdateCheckReport {
                available: false,
                current_version,
                version: None,
                published_at: None,
            },
        }))
    }

    #[cfg(not(windows))]
    Ok(update_failure(
        "update_disabled",
        "当前平台未接入应用内更新。",
        false,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn windows_update_install(
    app: AppHandle,
    expected_version: String,
) -> Result<AppResult<WindowsUpdateInstallReceipt>, ()> {
    if !updater_is_configured(&app) {
        return Ok(update_failure(
            "update_disabled",
            "当前安装包未接入自动更新，请使用新的正式安装包升级。",
            false,
        ));
    }
    if !valid_expected_version(&expected_version) {
        return Ok(update_failure(
            "update_version_changed",
            "可用版本已经变化，请重新检查。",
            true,
        ));
    }
    let Ok(_operation) = UPDATE_OPERATION.try_lock() else {
        return Ok(update_failure(
            "update_busy",
            "另一项更新操作正在进行，请稍候。",
            true,
        ));
    };

    #[cfg(windows)]
    {
        use tauri_plugin_updater::UpdaterExt as _;

        let cleanup_app = app.clone();
        let updater = match app
            .updater_builder()
            .on_before_exit(move || stop_background_work_before_update(&cleanup_app))
            .build()
        {
            Ok(updater) => updater,
            Err(_) => {
                return Ok(update_failure(
                    "update_install_failed",
                    "更新没有安装，当前版本保持不变，请稍后重试。",
                    true,
                ));
            }
        };
        let update = match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => {
                return Ok(update_failure(
                    "update_version_changed",
                    "可用版本已经变化，请重新检查。",
                    true,
                ));
            }
            Err(_) => {
                return Ok(update_failure(
                    "update_install_failed",
                    "更新没有安装，当前版本保持不变，请稍后重试。",
                    true,
                ));
            }
        };
        if update.version != expected_version {
            return Ok(update_failure(
                "update_version_changed",
                "可用版本已经变化，请重新检查。",
                true,
            ));
        }
        if update.download_and_install(|_, _| {}, || {}).await.is_err() {
            return Ok(update_failure(
                "update_install_failed",
                "更新没有安装，当前版本保持不变，请稍后重试。",
                true,
            ));
        }
        Ok(AppResult::success(WindowsUpdateInstallReceipt {
            accepted_version: expected_version,
        }))
    }

    #[cfg(not(windows))]
    Ok(update_failure(
        "update_disabled",
        "当前平台未接入应用内更新。",
        false,
    ))
}

fn updater_is_configured(app: &AppHandle) -> bool {
    updater_config_is_ready(app.config().plugins.0.get("updater"))
}

pub(crate) fn updater_config_is_ready(config: Option<&serde_json::Value>) -> bool {
    let Some(config) = config else {
        return false;
    };
    let public_key_ready = config
        .get("pubkey")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| {
            let trimmed = value.trim();
            (32..=16384).contains(&trimmed.len())
        });
    let endpoints_ready = config
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|endpoints| {
            !endpoints.is_empty()
                && endpoints.iter().all(|endpoint| {
                    endpoint
                        .as_str()
                        .and_then(|value| reqwest::Url::parse(value).ok())
                        .is_some_and(|url| {
                            url.scheme() == "https"
                                && url.username().is_empty()
                                && url.password().is_none()
                                && url.fragment().is_none()
                        })
                })
        });
    public_key_ready && endpoints_ready
}

fn valid_expected_version(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == value
        && !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
}

fn update_failure<T>(
    code: &'static str,
    user_message: &'static str,
    retryable: bool,
) -> AppResult<T> {
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!("windows update failed [{diagnostic_id}] {code}");
    AppResult::failure(code, user_message, retryable, diagnostic_id)
}

#[cfg(windows)]
fn stop_background_work_before_update(app: &AppHandle) {
    let _ = app
        .state::<crate::modules::capture_lan::CaptureLanManager>()
        .stop();
    let recognition = app
        .state::<crate::infrastructure::capture_recognition_worker::CaptureRecognitionManager>()
        .inner()
        .clone();
    tauri::async_runtime::block_on(async {
        recognition.shutdown().await;
        let _ = recognition
            .wait_for_idle(std::time::Duration::from_secs(5))
            .await;
    });
    if let Ok(control_root) = app.path().app_data_dir() {
        let _ = std::fs::remove_dir_all(control_root.join("recognition-private-temp"));
    }
}

#[cfg(test)]
mod tests {
    use super::{updater_config_is_ready, valid_expected_version};
    use serde_json::json;

    #[test]
    fn expected_versions_are_small_ascii_tokens() {
        for value in ["0.2.0", "1.0.0-rc.1", "2.1.0+windows"] {
            assert!(valid_expected_version(value));
        }
        for value in ["", " 0.2.0", "0.2.0\n", "版本-1", &"1".repeat(65)] {
            assert!(!valid_expected_version(value));
        }
    }

    #[test]
    fn updater_configuration_requires_a_key_and_credential_free_https_endpoints() {
        let ready = json!({
            "pubkey": "a".repeat(64),
            "endpoints": ["https://updates.example.invalid/latest.json"]
        });
        assert!(updater_config_is_ready(Some(&ready)));

        for rejected in [
            json!({ "pubkey": "short", "endpoints": ["https://updates.example.invalid/latest.json"] }),
            json!({ "pubkey": "a".repeat(64), "endpoints": [] }),
            json!({ "pubkey": "a".repeat(64), "endpoints": ["http://updates.example.invalid/latest.json"] }),
            json!({ "pubkey": "a".repeat(64), "endpoints": ["https://user:secret@updates.example.invalid/latest.json"] }),
            json!({ "pubkey": "a".repeat(64), "endpoints": ["https://updates.example.invalid/latest.json#token"] }),
        ] {
            assert!(!updater_config_is_ready(Some(&rejected)));
        }
        assert!(!updater_config_is_ready(None));
    }
}
