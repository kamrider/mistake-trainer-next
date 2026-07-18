use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    domain::review::FsrsRating,
    infrastructure::runtime::LibraryRuntime,
    modules::review::{
        ReviewQueueQuery, ReviewSubmission, SubmitReview, list_review_queue, submit_review,
    },
};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueItem {
    pub problem_id: String,
    pub due_at_utc_ms: Option<f64>,
    pub review_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueOverview {
    pub session_id: Option<String>,
    pub mode: String,
    pub resumed: bool,
    pub completed_count: i32,
    pub total_count: i32,
    pub items: Vec<ReviewQueueItem>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSubmitInput {
    pub problem_id: String,
    pub rating: FsrsRating,
    pub duration_ms: u32,
}

pub fn review_queue_for(
    runtime: &LibraryRuntime,
    problem_id: Option<String>,
    now_utc_ms: i64,
) -> AppResult<ReviewQueueOverview> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    let overview = match list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id,
            now_utc_ms,
        },
    ) {
        Ok(overview) => overview,
        Err(_) => return internal_review_error("review_queue_failed"),
    };
    let items = overview
        .items
        .into_iter()
        .map(|entry| ReviewQueueItem {
            problem_id: entry.problem_id,
            due_at_utc_ms: entry.due_at_utc_ms,
            review_count: entry.review_count,
        })
        .collect();
    AppResult::success(ReviewQueueOverview {
        session_id: overview.session_id,
        mode: overview.mode,
        resumed: overview.resumed,
        completed_count: overview.completed_count,
        total_count: overview.total_count,
        items,
    })
}

pub fn review_submit_for(
    runtime: &LibraryRuntime,
    input: ReviewSubmitInput,
    now_utc_ms: i64,
) -> AppResult<ReviewSubmission> {
    let profile = runtime.active_profile();
    let mut connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return internal_review_error("library_lock_poisoned"),
    };
    match submit_review(
        &mut connection,
        SubmitReview {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            problem_id: input.problem_id,
            device_id: runtime.device_id().to_owned(),
            rating: input.rating,
            duration_ms: input.duration_ms.min(86_400_000),
            occurred_at_utc_ms: now_utc_ms,
        },
    ) {
        Ok(submission) => AppResult::success(submission),
        Err(_) => internal_review_error("review_submit_failed"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn review_queue(
    state: State<'_, LibraryRuntime>,
    problem_id: Option<String>,
) -> AppResult<ReviewQueueOverview> {
    review_queue_for(&state, problem_id, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_submit(
    state: State<'_, LibraryRuntime>,
    input: ReviewSubmitInput,
) -> AppResult<ReviewSubmission> {
    review_submit_for(&state, input, current_utc_millis())
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn internal_review_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "训练记录暂时无法读取或保存，请稍后重试。",
        true,
        Uuid::now_v7().to_string(),
    )
}
