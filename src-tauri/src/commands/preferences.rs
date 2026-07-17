use serde::Deserialize;
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::preferences::{
        PreferencesError, SaveSubjectPreferences, SubjectPreferences, load_subject_preferences,
        save_subject_preferences,
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubjectPreferencesInput {
    pub enabled_subjects: Vec<String>,
    pub custom_subjects: Vec<String>,
    pub capture_sound_enabled: bool,
}

#[tauri::command]
#[specta::specta]
pub fn subject_preferences_get(state: State<'_, LibraryRuntime>) -> AppResult<SubjectPreferences> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return preferences_error("library_lock_poisoned", None),
    };
    match load_subject_preferences(&connection, state.account_id(), state.profile_id()) {
        Ok(preferences) => AppResult::success(preferences),
        Err(error) => preferences_error(preferences_error_code(&error), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn subject_preferences_save(
    state: State<'_, LibraryRuntime>,
    input: SubjectPreferencesInput,
) -> AppResult<SubjectPreferences> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return preferences_error("library_lock_poisoned", None),
    };
    let save = SaveSubjectPreferences {
        enabled_subjects: input.enabled_subjects,
        custom_subjects: input.custom_subjects,
        capture_sound_enabled: input.capture_sound_enabled,
    };
    match save_subject_preferences(
        &connection,
        state.account_id(),
        state.profile_id(),
        save,
        current_utc_millis(),
    ) {
        Ok(preferences) => AppResult::success(preferences),
        Err(error) => preferences_error(preferences_error_code(&error), Some(&error)),
    }
}

fn preferences_error_code(error: &PreferencesError) -> &'static str {
    match error {
        PreferencesError::InvalidInput => "subject_preferences_invalid",
        PreferencesError::ProfileNotFound => "subject_preferences_profile_not_found",
        PreferencesError::Database(_) | PreferencesError::Serialization(_) => {
            "subject_preferences_save_failed"
        }
    }
}

fn preferences_error<T>(code: &str, error: Option<&PreferencesError>) -> AppResult<T> {
    let (message, retryable) = match code {
        "subject_preferences_invalid" => (
            "至少保留一个科目；自定义科目最多 20 个，每个不超过 40 个字。",
            false,
        ),
        "subject_preferences_profile_not_found" => {
            ("当前学习档案已经变化，请重新选择后再试。", false)
        }
        "library_lock_poisoned" => ("本地题库暂时不可用，请重新打开应用后重试。", true),
        _ => ("科目配置没有保存成功，原有配置保持不变，请稍后重试。", true),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!("preferences error [{diagnostic_id}] {code}: {error}");
    }
    AppResult::failure(code, message, retryable, diagnostic_id)
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
