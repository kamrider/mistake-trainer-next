use serde::Serialize;
use specta::Type;

use crate::application::result::AppResult;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    app_version: String,
    storage: &'static str,
    sync: &'static str,
}

pub fn status_for_version(app_version: &str) -> AppResult<SystemStatus> {
    AppResult::success(SystemStatus {
        app_version: app_version.to_owned(),
        storage: "locked",
        sync: "offline",
    })
}

#[tauri::command]
#[specta::specta]
pub fn system_status() -> AppResult<SystemStatus> {
    status_for_version(env!("CARGO_PKG_VERSION"))
}

pub fn specta_commands<R: tauri::Runtime>() -> tauri_specta::Commands<R> {
    tauri_specta::collect_commands![system_status]
}
