use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::{application::result::AppResult, infrastructure::runtime::LibraryRuntime};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    app_version: String,
    storage: &'static str,
    sync: &'static str,
}

pub fn status_for_version(app_version: &str, storage: &'static str) -> AppResult<SystemStatus> {
    AppResult::success(SystemStatus {
        app_version: app_version.to_owned(),
        storage,
        sync: "offline",
    })
}

#[tauri::command]
#[specta::specta]
pub fn system_status(_state: State<'_, LibraryRuntime>) -> AppResult<SystemStatus> {
    status_for_version(env!("CARGO_PKG_VERSION"), "ready")
}

pub fn specta_commands<R: tauri::Runtime>() -> tauri_specta::Commands<R> {
    tauri_specta::collect_commands![system_status]
}
