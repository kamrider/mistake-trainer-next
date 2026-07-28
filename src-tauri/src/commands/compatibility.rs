use crate::{
    application::result::AppResult,
    modules::windows_compatibility::{WindowsCompatibilityStatus, current_windows_compatibility},
};

#[tauri::command]
#[specta::specta]
pub async fn compatibility_status() -> Result<AppResult<WindowsCompatibilityStatus>, ()> {
    Ok(AppResult::success(current_windows_compatibility()))
}
