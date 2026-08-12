use serde::Deserialize;
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::preferences::{
        LearningGoal, PreferencesError, ReviewFocusPolicy, ReviewPreferences, SaveLearningGoal,
        SaveReviewPreferences, SaveSubjectPreferences, SubjectPreferences, load_learning_goal,
        load_review_preferences, load_subject_preferences, save_learning_goal,
        save_review_preferences, save_subject_preferences,
    },
};

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubjectPreferencesInput {
    pub enabled_subjects: Vec<String>,
    pub custom_subjects: Vec<String>,
    pub capture_sound_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPreferencesInput {
    pub focus_policy: ReviewFocusPolicy,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LearningGoalInput {
    pub daily_review_target: i32,
    pub daily_minutes_target: i32,
}

#[tauri::command]
#[specta::specta]
pub fn subject_preferences_get(state: State<'_, LibraryRuntime>) -> AppResult<SubjectPreferences> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return preferences_error("library_lock_poisoned", None),
    };
    match load_subject_preferences(&connection, state.account_id(), &profile.id) {
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
    let profile = state.active_profile();
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
        &profile.id,
        save,
        current_utc_millis(),
    ) {
        Ok(preferences) => AppResult::success(preferences),
        Err(error) => preferences_error(preferences_error_code(&error), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn review_preferences_get(state: State<'_, LibraryRuntime>) -> AppResult<ReviewPreferences> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return review_preferences_error("library_lock_poisoned", None),
    };
    match load_review_preferences(&connection, state.account_id(), &profile.id) {
        Ok(preferences) => AppResult::success(preferences),
        Err(error) => review_preferences_error(review_preferences_error_code(&error), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn review_preferences_save(
    state: State<'_, LibraryRuntime>,
    input: ReviewPreferencesInput,
) -> AppResult<ReviewPreferences> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return review_preferences_error("library_lock_poisoned", None),
    };
    match save_review_preferences(
        &connection,
        state.account_id(),
        &profile.id,
        SaveReviewPreferences {
            focus_policy: input.focus_policy,
        },
        current_utc_millis(),
    ) {
        Ok(preferences) => AppResult::success(preferences),
        Err(error) => review_preferences_error(review_preferences_error_code(&error), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn learning_goal_get(state: State<'_, LibraryRuntime>) -> AppResult<LearningGoal> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return learning_goal_error("library_lock_poisoned", None),
    };
    match load_learning_goal(&connection, state.account_id(), &profile.id) {
        Ok(goal) => AppResult::success(goal),
        Err(error) => learning_goal_error(learning_goal_error_code(&error), Some(&error)),
    }
}

#[tauri::command]
#[specta::specta]
pub fn learning_goal_save(
    state: State<'_, LibraryRuntime>,
    input: LearningGoalInput,
) -> AppResult<LearningGoal> {
    let profile = state.active_profile();
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return learning_goal_error("library_lock_poisoned", None),
    };
    match save_learning_goal(
        &connection,
        state.account_id(),
        &profile.id,
        SaveLearningGoal {
            daily_review_target: input.daily_review_target,
            daily_minutes_target: input.daily_minutes_target,
        },
        current_utc_millis(),
    ) {
        Ok(goal) => AppResult::success(goal),
        Err(error) => learning_goal_error(learning_goal_error_code(&error), Some(&error)),
    }
}

fn learning_goal_error_code(error: &PreferencesError) -> &'static str {
    match error {
        PreferencesError::InvalidInput => "learning_goal_invalid",
        PreferencesError::ProfileNotFound => "learning_goal_profile_not_found",
        PreferencesError::Database(_) | PreferencesError::Serialization(_) => {
            "learning_goal_save_failed"
        }
    }
}

fn learning_goal_error<T>(code: &str, error: Option<&PreferencesError>) -> AppResult<T> {
    let (message, retryable) = match code {
        "learning_goal_invalid" => (
            "每日复习题数需为 1–200，每日学习时间需为 5–240 分钟。",
            false,
        ),
        "learning_goal_profile_not_found" => ("当前学习档案已经变化，请重新选择后再试。", false),
        "library_lock_poisoned" => ("本地题库暂时不可用，请重新打开应用后重试。", true),
        _ => ("学习目标没有保存成功，原有目标保持不变，请稍后重试。", true),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!("learning goal error [{diagnostic_id}] {code}: {error}");
    }
    AppResult::failure(code, message, retryable, diagnostic_id)
}

fn review_preferences_error_code(error: &PreferencesError) -> &'static str {
    match error {
        PreferencesError::InvalidInput => "review_preferences_invalid",
        PreferencesError::ProfileNotFound => "review_preferences_profile_not_found",
        PreferencesError::Database(_) | PreferencesError::Serialization(_) => {
            "review_preferences_save_failed"
        }
    }
}

fn review_preferences_error<T>(code: &str, error: Option<&PreferencesError>) -> AppResult<T> {
    let (message, retryable) = match code {
        "review_preferences_invalid" => ("训练节奏配置无效，请重新选择。", false),
        "review_preferences_profile_not_found" => {
            ("当前学习档案已经变化，请重新选择后再试。", false)
        }
        "library_lock_poisoned" => ("本地题库暂时不可用，请重新打开应用后重试。", true),
        _ => ("训练节奏没有保存成功，原有配置保持不变，请稍后重试。", true),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!("review preferences error [{diagnostic_id}] {code}: {error}");
    }
    AppResult::failure(code, message, retryable, diagnostic_id)
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
