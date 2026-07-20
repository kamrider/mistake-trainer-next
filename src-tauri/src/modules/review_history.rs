use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::{
    domain::review::FsrsRating,
    modules::review::{ALGORITHM_VERSION, PARAMETER_VERSION},
};

const DAY_MS: i64 = 86_400_000;
const MAX_CURSOR_BYTES: usize = 512;
const MAX_EVENT_ID_CHARS: usize = 128;
const MAX_SEARCH_CHARS: usize = 80;
const MAX_SUBJECT_CHARS: usize = 40;
const NOTE_PREVIEW_CHARS: usize = 72;

const FILTER_SQL: &str = r#"
    e.account_id = :account_id
    AND e.profile_id = :profile_id
    AND (:start_utc_ms IS NULL OR e.occurred_at_utc_ms >= :start_utc_ms)
    AND (:rating IS NULL OR e.rating = :rating)
    AND (:subject IS NULL OR p.subject = :subject)
    AND (:search = '' OR p.note LIKE :search ESCAPE '\')
"#;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewHistoryCursor {
    occurred_at_utc_ms: i64,
    event_id: String,
}

struct ValidatedQuery {
    start_utc_ms: Option<i64>,
    rating: Option<&'static str>,
    subject: Option<String>,
    search_pattern: String,
    cursor: Option<ReviewHistoryCursor>,
    limit: i64,
}

pub fn list_review_history(
    connection: &Connection,
    query: ReviewHistoryQuery,
) -> Result<ReviewHistoryPage, ReviewHistoryError> {
    let validated = validate_query(&query)?;
    let cursor_time = validated
        .cursor
        .as_ref()
        .map(|cursor| cursor.occurred_at_utc_ms);
    let cursor_id = validated
        .cursor
        .as_ref()
        .map(|cursor| cursor.event_id.as_str());
    let subject = validated.subject.as_deref();
    let rating = validated.rating;
    let fetch_limit = validated.limit + 1;
    let list_sql = format!(
        "SELECT e.id, p.subject, p.note, p.status, e.rating, e.duration_ms,
                e.occurred_at_utc_ms, e.algorithm_version, e.parameter_version
         FROM review_events e
         INNER JOIN problems p ON p.id = e.problem_id
         WHERE {FILTER_SQL}
           AND (:cursor_time IS NULL
             OR e.occurred_at_utc_ms < :cursor_time
             OR (e.occurred_at_utc_ms = :cursor_time AND e.id < :cursor_id))
         ORDER BY e.occurred_at_utc_ms DESC, e.id DESC
         LIMIT :fetch_limit"
    );
    let mut statement = connection.prepare(&list_sql)?;
    let mut items = statement
        .query_map(
            named_params! {
                ":account_id": query.account_id,
                ":profile_id": query.profile_id,
                ":start_utc_ms": validated.start_utc_ms,
                ":rating": rating,
                ":subject": subject,
                ":search": validated.search_pattern,
                ":cursor_time": cursor_time,
                ":cursor_id": cursor_id,
                ":fetch_limit": fetch_limit,
            },
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )?
        .map(|row| {
            let (
                event_id,
                subject,
                note,
                problem_status,
                rating,
                duration,
                occurred,
                algorithm,
                parameters,
            ) = row?;
            Ok(ReviewHistoryItem {
                event_id,
                subject,
                note_preview: note_preview(&note),
                problem_status,
                rating: parse_rating(&rating)?,
                duration_ms: duration as f64,
                occurred_at_utc_ms: occurred as f64,
                algorithm_is_current: algorithm == ALGORITHM_VERSION,
                parameters_are_current: parameters == PARAMETER_VERSION,
                algorithm_version: algorithm,
                parameter_version: parameters,
            })
        })
        .collect::<Result<Vec<_>, ReviewHistoryError>>()?;

    let has_more = items.len() > usize::try_from(validated.limit).unwrap_or(usize::MAX);
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items
            .last()
            .map(|item| {
                encode_cursor(&ReviewHistoryCursor {
                    occurred_at_utc_ms: item.occurred_at_utc_ms as i64,
                    event_id: item.event_id.clone(),
                })
            })
            .transpose()?
    } else {
        None
    };

    let count_sql = format!(
        "SELECT COUNT(*)
         FROM review_events e INNER JOIN problems p ON p.id = e.problem_id
         WHERE {FILTER_SQL}"
    );
    let total_count = connection.query_row(
        &count_sql,
        named_params! {
            ":account_id": query.account_id,
            ":profile_id": query.profile_id,
            ":start_utc_ms": validated.start_utc_ms,
            ":rating": rating,
            ":subject": subject,
            ":search": validated.search_pattern,
        },
        |row| row.get::<_, i64>(0),
    )?;
    let subjects_sql = format!(
        "SELECT DISTINCT p.subject
         FROM review_events e INNER JOIN problems p ON p.id = e.problem_id
         WHERE {FILTER_SQL} AND trim(p.subject) != ''
         ORDER BY p.subject COLLATE NOCASE, p.subject"
    );
    let available_subjects = connection
        .prepare(&subjects_sql)?
        .query_map(
            named_params! {
                ":account_id": query.account_id,
                ":profile_id": query.profile_id,
                ":start_utc_ms": validated.start_utc_ms,
                ":rating": rating,
                ":subject": subject,
                ":search": validated.search_pattern,
            },
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ReviewHistoryPage {
        items,
        next_cursor,
        total_count: bounded_i32(total_count),
        available_subjects,
    })
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

fn validate_query(query: &ReviewHistoryQuery) -> Result<ValidatedQuery, ReviewHistoryError> {
    if !(1..=50).contains(&query.limit) || query.search.chars().count() > MAX_SEARCH_CHARS {
        return Err(ReviewHistoryError::InvalidQuery);
    }
    let subject = query
        .subject
        .as_ref()
        .map(|subject| subject.trim().to_owned());
    if subject
        .as_ref()
        .is_some_and(|subject| subject.is_empty() || subject.chars().count() > MAX_SUBJECT_CHARS)
    {
        return Err(ReviewHistoryError::InvalidQuery);
    }
    let search = query.search.trim();
    let search_pattern = if search.is_empty() {
        String::new()
    } else {
        format!("%{}%", escape_like(search))
    };
    let start_utc_ms = match query.range {
        ReviewHistoryRange::All => None,
        ReviewHistoryRange::SevenDays => Some(query.now_utc_ms.saturating_sub(7 * DAY_MS)),
        ReviewHistoryRange::ThirtyDays => Some(query.now_utc_ms.saturating_sub(30 * DAY_MS)),
    };
    let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;
    Ok(ValidatedQuery {
        start_utc_ms,
        rating: query.rating.map(rating_label),
        subject,
        search_pattern,
        cursor,
        limit: i64::from(query.limit),
    })
}

fn encode_cursor(cursor: &ReviewHistoryCursor) -> Result<String, ReviewHistoryError> {
    Ok(URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor)?))
}

fn decode_cursor(encoded: &str) -> Result<ReviewHistoryCursor, ReviewHistoryError> {
    if encoded.is_empty() || encoded.len() > MAX_CURSOR_BYTES {
        return Err(ReviewHistoryError::InvalidCursor);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| ReviewHistoryError::InvalidCursor)?;
    let cursor: ReviewHistoryCursor =
        serde_json::from_slice(&decoded).map_err(|_| ReviewHistoryError::InvalidCursor)?;
    if cursor.event_id.is_empty() || cursor.event_id.chars().count() > MAX_EVENT_ID_CHARS {
        return Err(ReviewHistoryError::InvalidCursor);
    }
    Ok(cursor)
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn note_preview(note: &str) -> String {
    let mut characters = note.trim().chars();
    let preview = characters
        .by_ref()
        .take(NOTE_PREVIEW_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

const fn rating_label(rating: FsrsRating) -> &'static str {
    match rating {
        FsrsRating::Again => "again",
        FsrsRating::Hard => "hard",
        FsrsRating::Good => "good",
        FsrsRating::Easy => "easy",
    }
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
