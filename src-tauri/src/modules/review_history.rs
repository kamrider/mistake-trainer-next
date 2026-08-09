use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::{
    domain::review::FsrsRating,
    modules::review::{ALGORITHM_VERSION, PARAMETER_VERSION},
};

#[path = "review_history_list_repository.rs"]
mod list_repository;

const MAX_EVENT_ID_CHARS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ReviewHistoryRange {
    All,
    #[serde(rename = "7_days")]
    SevenDays,
    #[serde(rename = "30_days")]
    ThirtyDays,
}

#[derive(Clone, Debug)]
pub struct ReviewHistoryQuery {
    pub account_id: String,
    pub profile_id: String,
    pub range: ReviewHistoryRange,
    pub rating: Option<FsrsRating>,
    pub subject: Option<String>,
    pub search: String,
    pub cursor: Option<String>,
    pub limit: u32,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ReviewHistoryDetailQuery {
    pub account_id: String,
    pub profile_id: String,
    pub event_id: String,
    pub current_device_id: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHistoryItem {
    pub event_id: String,
    pub subject: String,
    pub note_preview: String,
    pub problem_status: String,
    pub rating: FsrsRating,
    pub duration_ms: f64,
    pub occurred_at_utc_ms: f64,
    pub algorithm_version: String,
    pub parameter_version: String,
    pub algorithm_is_current: bool,
    pub parameters_are_current: bool,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHistoryPage {
    pub items: Vec<ReviewHistoryItem>,
    pub next_cursor: Option<String>,
    pub total_count: i32,
    pub available_subjects: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CurrentScheduleProjection {
    pub due_at_utc_ms: f64,
    pub stability: f64,
    pub difficulty: f64,
    pub last_reviewed_at_utc_ms: Option<f64>,
    pub algorithm_version: String,
    pub parameter_version: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHistoryDetail {
    pub event_id: String,
    pub subject: String,
    pub note: String,
    pub problem_status: String,
    pub rating: FsrsRating,
    pub duration_ms: f64,
    pub occurred_at_utc_ms: f64,
    pub algorithm_version: String,
    pub parameter_version: String,
    pub algorithm_is_current: bool,
    pub parameters_are_current: bool,
    pub is_current_device: bool,
    pub review_ordinal: i32,
    pub problem_review_count: i32,
    pub current_schedule: Option<CurrentScheduleProjection>,
}

#[derive(Debug, Error)]
pub enum ReviewHistoryError {
    #[error("review history query is invalid")]
    InvalidQuery,
    #[error("review history cursor is invalid")]
    InvalidCursor,
    #[error("review history event was not found")]
    NotFound,
    #[error("review history contains invalid persisted state")]
    CorruptState,
    #[error("review history database query failed")]
    Database(#[from] rusqlite::Error),
    #[error("review history cursor serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub fn list_review_history(
    connection: &Connection,
    query: ReviewHistoryQuery,
) -> Result<ReviewHistoryPage, ReviewHistoryError> {
    list_repository::list_review_history(connection, query)
}

pub fn get_review_history_detail(
    connection: &Connection,
    query: ReviewHistoryDetailQuery,
) -> Result<ReviewHistoryDetail, ReviewHistoryError> {
    if query.event_id.is_empty() || query.event_id.chars().count() > MAX_EVENT_ID_CHARS {
        return Err(ReviewHistoryError::InvalidQuery);
    }
    type DetailRow = (
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
        String,
        String,
        i64,
        i64,
        Option<i64>,
        Option<f64>,
        Option<f64>,
        Option<i64>,
        Option<String>,
        Option<String>,
    );
    let row: DetailRow = connection
        .query_row(
            "SELECT e.id, p.subject, p.note, p.status, e.rating, e.duration_ms,
                    e.occurred_at_utc_ms, e.algorithm_version, e.parameter_version, e.device_id,
                    (SELECT COUNT(*) FROM review_events prior
                     WHERE prior.account_id = e.account_id AND prior.profile_id = e.profile_id
                       AND prior.problem_id = e.problem_id
                       AND (prior.occurred_at_utc_ms < e.occurred_at_utc_ms
                         OR (prior.occurred_at_utc_ms = e.occurred_at_utc_ms AND prior.id <= e.id))),
                    (SELECT COUNT(*) FROM review_events all_events
                     WHERE all_events.account_id = e.account_id AND all_events.profile_id = e.profile_id
                       AND all_events.problem_id = e.problem_id),
                    s.due_at_utc_ms, s.stability, s.difficulty, s.last_reviewed_at_utc_ms,
                    s.algorithm_version, s.parameter_version
             FROM review_events e
             INNER JOIN problems p ON p.id = e.problem_id
                AND p.account_id = e.account_id
                AND p.profile_id = e.profile_id
             LEFT JOIN schedule_states s ON s.problem_id = e.problem_id
             WHERE e.account_id = ?1 AND e.profile_id = ?2 AND e.id = ?3",
            (&query.account_id, &query.profile_id, &query.event_id),
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?, row.get(17)?,
            )),
        )
        .optional()?
        .ok_or(ReviewHistoryError::NotFound)?;
    let (
        event_id,
        subject,
        note,
        status,
        rating,
        duration,
        occurred,
        algorithm,
        parameters,
        device,
        ordinal,
        total,
        due,
        stability,
        difficulty,
        last_reviewed,
        schedule_algorithm,
        schedule_parameters,
    ) = row;
    let current_schedule = match due {
        None => None,
        Some(due_at_utc_ms) => Some(CurrentScheduleProjection {
            due_at_utc_ms: due_at_utc_ms as f64,
            stability: stability.ok_or(ReviewHistoryError::CorruptState)?,
            difficulty: difficulty.ok_or(ReviewHistoryError::CorruptState)?,
            last_reviewed_at_utc_ms: last_reviewed.map(|value| value as f64),
            algorithm_version: schedule_algorithm.ok_or(ReviewHistoryError::CorruptState)?,
            parameter_version: schedule_parameters.ok_or(ReviewHistoryError::CorruptState)?,
        }),
    };
    Ok(ReviewHistoryDetail {
        event_id,
        subject,
        note,
        problem_status: status,
        rating: parse_rating(&rating)?,
        duration_ms: duration as f64,
        occurred_at_utc_ms: occurred as f64,
        algorithm_is_current: algorithm == ALGORITHM_VERSION,
        parameters_are_current: parameters == PARAMETER_VERSION,
        algorithm_version: algorithm,
        parameter_version: parameters,
        is_current_device: device == query.current_device_id,
        review_ordinal: bounded_i32(ordinal),
        problem_review_count: bounded_i32(total),
        current_schedule,
    })
}

fn parse_rating(value: &str) -> Result<FsrsRating, ReviewHistoryError> {
    match value {
        "again" => Ok(FsrsRating::Again),
        "hard" => Ok(FsrsRating::Hard),
        "good" => Ok(FsrsRating::Good),
        "easy" => Ok(FsrsRating::Easy),
        _ => Err(ReviewHistoryError::CorruptState),
    }
}

fn bounded_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
