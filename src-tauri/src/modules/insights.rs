use rusqlite::Connection;
use serde::Serialize;
use specta::Type;
use thiserror::Error;

#[path = "insights_read_repository.rs"]
mod read_repository;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub day_start_utc_ms: f64,
    pub review_count: i32,
    pub duration_ms: f64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubjectActivity {
    pub subject: String,
    pub problem_count: i32,
    pub review_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WeakAreaSummary {
    pub label: String,
    pub kind: String,
    pub reviewed_count: i32,
    pub lapse_count: i32,
    pub lapse_rate: f64,
    pub average_duration_ms: f64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DueForecastDay {
    pub local_date: String,
    pub due_count: i32,
    pub overdue_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub active_problem_count: i32,
    pub due_problem_count: i32,
    pub review_count: i32,
    pub remembered_rate: f64,
    pub total_duration_ms: f64,
    pub current_streak_days: i32,
    pub daily_activity: Vec<DailyActivity>,
    pub subject_activity: Vec<SubjectActivity>,
    pub weak_areas: Vec<WeakAreaSummary>,
    pub due_forecast: Vec<DueForecastDay>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DashboardOverview {
    pub profile_name: String,
    pub active_problem_count: i32,
    pub due_problem_count: i32,
    pub reviewed_today_count: i32,
    pub remembered_rate_30_days: Option<f64>,
    pub current_streak_days: i32,
    pub pending_capture_batch_count: i32,
    pub pending_capture_item_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOverview {
    pub active_problem_count: i32,
    pub archived_problem_count: i32,
    pub trashed_problem_count: i32,
    pub pending_operation_count: i32,
    pub failed_operation_count: i32,
    pub unresolved_conflict_count: i32,
    pub local_encryption_ready: bool,
    pub cloud_sync_configured: bool,
}

#[derive(Debug, Error)]
pub enum InsightsError {
    #[error("timezone offset is outside the supported range")]
    InvalidTimezoneOffset,
    #[error("report query failed")]
    Database(#[from] rusqlite::Error),
}

pub fn dashboard_overview(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Result<DashboardOverview, InsightsError> {
    read_repository::dashboard_overview(
        connection,
        account_id,
        profile_id,
        now_utc_ms,
        utc_offset_minutes,
    )
}

pub fn report_summary(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Result<ReportSummary, InsightsError> {
    read_repository::report_summary(
        connection,
        account_id,
        profile_id,
        now_utc_ms,
        utc_offset_minutes,
    )
}

pub fn settings_overview(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<SettingsOverview, InsightsError> {
    read_repository::settings_overview(connection, account_id, profile_id)
}
