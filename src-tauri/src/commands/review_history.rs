use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    domain::review::FsrsRating,
    infrastructure::runtime::LibraryRuntime,
    modules::review_history::{
        ReviewHistoryDetail, ReviewHistoryDetailQuery, ReviewHistoryError, ReviewHistoryPage,
        ReviewHistoryQuery, ReviewHistoryRange, get_review_history_detail, list_review_history,
    },
};

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHistoryInput {
    pub range: ReviewHistoryRange,
    pub rating: Option<FsrsRating>,
    pub subject: Option<String>,
    pub search: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

pub fn review_history_list_for(
    runtime: &LibraryRuntime,
    input: ReviewHistoryInput,
    now_utc_ms: i64,
) -> AppResult<ReviewHistoryPage> {
    let _transition = runtime.lock_profile_transition();
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return read_error("library_lock_poisoned"),
    };
    match list_review_history(
        &connection,
        ReviewHistoryQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            range: input.range,
            rating: input.rating,
            subject: input.subject,
            search: input.search,
            cursor: input.cursor,
            limit: input.limit,
            now_utc_ms,
        },
    ) {
        Ok(page) => AppResult::success(page),
        Err(error) => map_history_error(error),
    }
}

pub fn review_history_detail_for(
    runtime: &LibraryRuntime,
    event_id: String,
) -> AppResult<ReviewHistoryDetail> {
    let _transition = runtime.lock_profile_transition();
    let profile = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return read_error("library_lock_poisoned"),
    };
    match get_review_history_detail(
        &connection,
        ReviewHistoryDetailQuery {
            account_id: runtime.account_id().to_owned(),
            profile_id: profile.id,
            event_id,
            current_device_id: runtime.device_id().to_owned(),
        },
    ) {
        Ok(detail) => AppResult::success(detail),
        Err(error) => map_history_error(error),
    }
}

#[tauri::command]
#[specta::specta]
pub fn review_history_list(
    state: State<'_, LibraryRuntime>,
    input: ReviewHistoryInput,
) -> AppResult<ReviewHistoryPage> {
    review_history_list_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn review_history_detail(
    state: State<'_, LibraryRuntime>,
    event_id: String,
) -> AppResult<ReviewHistoryDetail> {
    review_history_detail_for(&state, event_id)
}

fn map_history_error<T>(error: ReviewHistoryError) -> AppResult<T> {
    match error {
        ReviewHistoryError::InvalidQuery | ReviewHistoryError::InvalidCursor => AppResult::failure(
            "review_history_query_invalid",
            "复习历史的筛选条件已失效，请重置筛选后重试。",
            false,
            Uuid::now_v7().to_string(),
        ),
        ReviewHistoryError::NotFound => AppResult::failure(
            "review_history_event_missing",
            "这条复习记录不存在，或不属于当前学习档案。",
            false,
            Uuid::now_v7().to_string(),
        ),
        ReviewHistoryError::CorruptState
        | ReviewHistoryError::Database(_)
        | ReviewHistoryError::Serialization(_) => read_error("review_history_read_failed"),
    }
}

fn read_error<T>(code: &str) -> AppResult<T> {
    AppResult::failure(
        code,
        "复习历史暂时无法读取，请稍后重试。",
        true,
        Uuid::now_v7().to_string(),
    )
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_errors_are_stable_and_do_not_contain_internal_details() {
        for (error, code, retryable) in [
            (
                ReviewHistoryError::InvalidCursor,
                "review_history_query_invalid",
                false,
            ),
            (
                ReviewHistoryError::NotFound,
                "review_history_event_missing",
                false,
            ),
            (
                ReviewHistoryError::CorruptState,
                "review_history_read_failed",
                true,
            ),
        ] {
            let value = serde_json::to_value(map_history_error::<()>(error)).unwrap();
            assert_eq!(value["error"]["code"], code);
            assert_eq!(value["error"]["retryable"], retryable);
            let serialized = value.to_string();
            assert!(!serialized.contains("device"));
            assert!(!serialized.contains("SELECT"));
            assert!(!serialized.contains("library.db"));
        }
    }
}
