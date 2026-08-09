use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, named_params};
use serde::{Deserialize, Serialize};

use crate::{
    domain::review::FsrsRating,
    modules::review::{ALGORITHM_VERSION, PARAMETER_VERSION},
};

use super::{
    MAX_EVENT_ID_CHARS, ReviewHistoryError, ReviewHistoryItem, ReviewHistoryPage,
    ReviewHistoryQuery, ReviewHistoryRange, bounded_i32, parse_rating,
};

const DAY_MS: i64 = 86_400_000;
const MAX_CURSOR_BYTES: usize = 512;
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

pub(super) fn list_review_history(
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
            AND p.account_id = e.account_id
            AND p.profile_id = e.profile_id
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
         FROM review_events e
         INNER JOIN problems p ON p.id = e.problem_id
            AND p.account_id = e.account_id
            AND p.profile_id = e.profile_id
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
         FROM review_events e
         INNER JOIN problems p ON p.id = e.problem_id
            AND p.account_id = e.account_id
            AND p.profile_id = e.profile_id
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

fn validate_query(query: &ReviewHistoryQuery) -> Result<ValidatedQuery, ReviewHistoryError> {
    let search = query.search.trim();
    if !(1..=50).contains(&query.limit) || search.chars().count() > MAX_SEARCH_CHARS {
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
