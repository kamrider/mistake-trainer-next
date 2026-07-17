use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    infrastructure::runtime::LibraryRuntime,
    modules::insights::{
        DashboardOverview, InsightsError, ReportSummary, SettingsOverview,
        dashboard_overview as load_dashboard_overview, report_summary as load_report_summary,
        settings_overview as load_settings_overview,
    },
};

#[tauri::command]
#[specta::specta]
pub fn dashboard_overview(
    state: State<'_, LibraryRuntime>,
    utc_offset_minutes: i32,
) -> AppResult<DashboardOverview> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return insights_error("library_lock_poisoned"),
    };
    match load_dashboard_overview(
        &connection,
        state.account_id(),
        state.profile_id(),
        current_utc_millis(),
        utc_offset_minutes,
    ) {
        Ok(overview) => AppResult::success(overview),
        Err(InsightsError::InvalidTimezoneOffset) => AppResult::failure(
            "dashboard_timezone_invalid",
            "系统时区设置异常，请检查 Windows 日期和时间设置。",
            false,
            Uuid::now_v7().to_string(),
        ),
        Err(InsightsError::Database(_)) => insights_error("dashboard_overview_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn report_summary(state: State<'_, LibraryRuntime>) -> AppResult<ReportSummary> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return insights_error("library_lock_poisoned"),
    };
    match load_report_summary(
        &connection,
        state.account_id(),
        state.profile_id(),
        current_utc_millis(),
    ) {
        Ok(summary) => AppResult::success(summary),
        Err(_) => insights_error("report_summary_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn settings_overview(state: State<'_, LibraryRuntime>) -> AppResult<SettingsOverview> {
    let connection = match state.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return insights_error("library_lock_poisoned"),
    };
    match load_settings_overview(&connection, state.account_id(), state.profile_id()) {
        Ok(overview) => AppResult::success(overview),
        Err(_) => insights_error("settings_overview_failed"),
    }
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn insights_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "学习统计暂时无法读取，请稍后重试。",
        true,
        Uuid::now_v7().to_string(),
    )
}
