use fsrs::{ItemState, MemoryState, FSRS};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use specta::Type;
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::review::FsrsRating;

const DESIRED_RETENTION: f32 = 0.90;
const MILLIS_PER_DAY: i64 = 86_400_000;
const ALGORITHM_VERSION: &str = "fsrs-6.6.1";
const PARAMETER_VERSION: &str = "default-6.6.1";

#[derive(Clone, Debug)]
pub struct ReviewQueueQuery {
    pub account_id: String,
    pub profile_id: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct StartManualReview {
    pub account_id: String,
    pub profile_id: String,
    pub problem_ids: Vec<String>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueEntry {
    pub problem_id: String,
    pub due_at_utc_ms: Option<f64>,
    pub review_count: i32,
}

#[derive(Clone, Debug)]
pub struct ReviewQueueState {
    pub session_id: Option<String>,
    pub mode: String,
    pub resumed: bool,
    pub completed_count: i32,
    pub total_count: i32,
    pub items: Vec<ReviewQueueEntry>,
}

#[derive(Clone, Debug)]
pub struct SubmitReview {
    pub account_id: String,
    pub profile_id: String,
    pub problem_id: String,
    pub device_id: String,
    pub rating: FsrsRating,
    pub duration_ms: u32,
    pub occurred_at_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSubmission {
    pub event_id: String,
    pub problem_id: String,
    pub rating: String,
    pub due_at_utc_ms: f64,
    pub stability: f32,
    pub difficulty: f32,
    pub algorithm_version: String,
    pub parameter_version: String,
}

#[derive(Debug, Error)]
pub enum ReviewUseCaseError {
    #[error("problem was not found for this account and profile")]
    ProblemNotFound,
    #[error("review persistence failed")]
    Database(#[from] rusqlite::Error),
    #[error("FSRS scheduling failed")]
    Scheduler(#[from] fsrs::FSRSError),
    #[error("review outbox serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("review session is missing or out of sync")]
    SessionOutOfSync,
    #[error("manual review selection is invalid")]
    InvalidManualSelection,
}

pub fn list_review_queue(
    connection: &mut Connection,
    query: ReviewQueueQuery,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    let transaction = connection.transaction()?;
    let active_session = transaction
        .query_row(
            "SELECT id, mode, problem_ids_json, current_index
             FROM review_sessions
             WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'",
            params![query.account_id, query.profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((session_id, session_mode, ids_json, stored_index)) = active_session {
        let ids: Vec<String> = serde_json::from_str(&ids_json)?;
        let current_index = usize::try_from(stored_index).unwrap_or(ids.len());
        let safe_index = current_index.min(ids.len());
        let entries = queue_entries_for_ids(
            &transaction,
            &query.account_id,
            &query.profile_id,
            &ids[safe_index..],
        )?;
        let mut cleaned_ids = ids[..safe_index].to_vec();
        cleaned_ids.extend(entries.iter().map(|entry| entry.problem_id.clone()));
        let completed_count = count_for_ui(safe_index);
        let total_count = count_for_ui(cleaned_ids.len());
        let has_remaining_items = !entries.is_empty();
        let serialized_ids = serde_json::to_string(&cleaned_ids)?;
        transaction.execute(
            "UPDATE review_sessions
             SET problem_ids_json = ?1,
                 current_index = ?2,
                 status = CASE WHEN ?3 = 0 THEN 'completed' ELSE 'active' END,
                 updated_at_utc_ms = ?4
             WHERE id = ?5",
            params![
                serialized_ids,
                completed_count,
                i32::from(has_remaining_items),
                query.now_utc_ms,
                session_id
            ],
        )?;
        transaction.commit()?;
        return Ok(ReviewQueueState {
            session_id: Some(session_id),
            mode: session_mode,
            resumed: true,
            completed_count,
            total_count,
            items: entries,
        });
    }

    let entries = query_new_review_entries(&transaction, &query)?;
    let total_count = count_for_ui(entries.len());
    let session_id = if entries.is_empty() {
        None
    } else {
        let session_id = Uuid::now_v7().to_string();
        let problem_ids = entries
            .iter()
            .map(|entry| entry.problem_id.as_str())
            .collect::<Vec<_>>();
        transaction.execute(
            "INSERT INTO review_sessions(id, account_id, profile_id, mode, problem_ids_json, current_index, status, created_at_utc_ms, updated_at_utc_ms)
             VALUES(?1, ?2, ?3, ?4, ?5, 0, 'active', ?6, ?6)",
            params![
                &session_id,
                query.account_id,
                query.profile_id,
                "due",
                serde_json::to_string(&problem_ids)?,
                query.now_utc_ms
            ],
        )?;
        Some(session_id)
    };
    transaction.commit()?;
    Ok(ReviewQueueState {
        session_id,
        mode: "due".to_owned(),
        resumed: false,
        completed_count: 0,
        total_count,
        items: entries,
    })
}

pub fn start_manual_review_queue(
    connection: &mut Connection,
    input: StartManualReview,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    if !(1..=100).contains(&input.problem_ids.len()) {
        return Err(ReviewUseCaseError::InvalidManualSelection);
    }
    let unique = input.problem_ids.iter().collect::<HashSet<_>>();
    if unique.len() != input.problem_ids.len() {
        return Err(ReviewUseCaseError::InvalidManualSelection);
    }

    let transaction = connection.transaction()?;
    let entries = queue_entries_for_ids(
        &transaction,
        &input.account_id,
        &input.profile_id,
        &input.problem_ids,
    )?;
    if entries.len() != input.problem_ids.len() {
        return Err(ReviewUseCaseError::InvalidManualSelection);
    }

    transaction.execute(
        "UPDATE review_sessions
         SET status = 'cancelled', updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'",
        params![input.now_utc_ms, input.account_id, input.profile_id],
    )?;
    let session_id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO review_sessions(
             id, account_id, profile_id, mode, problem_ids_json, current_index,
             status, created_at_utc_ms, updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, 'manual', ?4, 0, 'active', ?5, ?5)",
        params![
            session_id,
            input.account_id,
            input.profile_id,
            serde_json::to_string(&input.problem_ids)?,
            input.now_utc_ms,
        ],
    )?;
    transaction.commit()?;

    Ok(ReviewQueueState {
        session_id: Some(session_id),
        mode: "manual".to_owned(),
        resumed: false,
        completed_count: 0,
        total_count: count_for_ui(entries.len()),
        items: entries,
    })
}

fn count_for_ui(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn query_new_review_entries(
    transaction: &Transaction<'_>,
    query: &ReviewQueueQuery,
) -> Result<Vec<ReviewQueueEntry>, ReviewUseCaseError> {
    let mut statement = transaction.prepare(
        "SELECT p.id, s.due_at_utc_ms, COUNT(e.id)
         FROM problems p
         LEFT JOIN schedule_states s ON s.problem_id = p.id
         LEFT JOIN review_events e ON e.problem_id = p.id
         WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
           AND (s.due_at_utc_ms IS NULL OR s.due_at_utc_ms <= ?3)
         GROUP BY p.id
         ORDER BY COALESCE(s.due_at_utc_ms, 0), p.updated_at_utc_ms, p.id
         LIMIT 100",
    )?;
    let rows = statement.query_map(
        params![&query.account_id, &query.profile_id, query.now_utc_ms],
        |row| {
            Ok(ReviewQueueEntry {
                problem_id: row.get(0)?,
                due_at_utc_ms: row.get(1)?,
                review_count: row.get(2)?,
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ReviewUseCaseError::Database)
}

fn queue_entries_for_ids(
    transaction: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
    problem_ids: &[String],
) -> Result<Vec<ReviewQueueEntry>, ReviewUseCaseError> {
    let mut entries = Vec::with_capacity(problem_ids.len());
    for problem_id in problem_ids {
        if let Some(entry) = transaction
            .query_row(
                "SELECT p.id, s.due_at_utc_ms, COUNT(e.id)
                 FROM problems p
                 LEFT JOIN schedule_states s ON s.problem_id = p.id
                 LEFT JOIN review_events e ON e.problem_id = p.id
                 WHERE p.id = ?1 AND p.account_id = ?2 AND p.profile_id = ?3 AND p.status = 'active'
                 GROUP BY p.id",
                params![problem_id, account_id, profile_id],
                |row| {
                    Ok(ReviewQueueEntry {
                        problem_id: row.get(0)?,
                        due_at_utc_ms: row.get(1)?,
                        review_count: row.get(2)?,
                    })
                },
            )
            .optional()?
        {
            entries.push(entry);
        }
    }
    Ok(entries)
}

#[derive(Clone)]
struct StoredEvent {
    id: String,
    rating: FsrsRating,
    occurred_at_utc_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewEventPayload<'a> {
    id: &'a str,
    account_id: &'a str,
    profile_id: &'a str,
    problem_id: &'a str,
    device_id: &'a str,
    rating: &'a str,
    duration_ms: u32,
    occurred_at_utc_ms: i64,
    algorithm_version: &'static str,
    parameter_version: &'static str,
}

pub fn submit_review(
    connection: &mut Connection,
    input: SubmitReview,
) -> Result<ReviewSubmission, ReviewUseCaseError> {
    let problem_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM problems WHERE id = ?1 AND profile_id = ?2 AND account_id = ?3 AND status = 'active')",
        params![input.problem_id, input.profile_id, input.account_id],
        |row| row.get(0),
    )?;
    if !problem_exists {
        return Err(ReviewUseCaseError::ProblemNotFound);
    }

    let event_id = Uuid::now_v7().to_string();
    let rating = input.rating;
    let rating_label = rating_label(rating);
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![event_id, input.account_id, input.profile_id, input.problem_id, input.device_id, rating_label, i64::from(input.duration_ms), input.occurred_at_utc_ms, ALGORITHM_VERSION, PARAMETER_VERSION],
    )?;

    let events = {
        let mut statement = transaction.prepare(
            "SELECT id, rating, occurred_at_utc_ms FROM review_events WHERE problem_id = ?1 ORDER BY occurred_at_utc_ms, id",
        )?;
        statement
            .query_map([&input.problem_id], |row| {
                let label: String = row.get(1)?;
                Ok(StoredEvent {
                    id: row.get(0)?,
                    rating: parse_rating(&label),
                    occurred_at_utc_ms: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    let (state, due_at_utc_ms) = rebuild_schedule(&events)?;
    transaction.execute(
        "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(problem_id) DO UPDATE SET due_at_utc_ms = excluded.due_at_utc_ms, stability = excluded.stability, difficulty = excluded.difficulty, last_reviewed_at_utc_ms = excluded.last_reviewed_at_utc_ms, algorithm_version = excluded.algorithm_version, parameter_version = excluded.parameter_version, rebuilt_at_utc_ms = excluded.rebuilt_at_utc_ms",
        params![input.problem_id, due_at_utc_ms, f64::from(state.stability), f64::from(state.difficulty), input.occurred_at_utc_ms, ALGORITHM_VERSION, PARAMETER_VERSION, input.occurred_at_utc_ms],
    )?;

    let result = ReviewSubmission {
        event_id: event_id.clone(),
        problem_id: input.problem_id.clone(),
        rating: rating_label.to_owned(),
        due_at_utc_ms: due_at_utc_ms as f64,
        stability: state.stability,
        difficulty: state.difficulty,
        algorithm_version: ALGORITHM_VERSION.to_owned(),
        parameter_version: PARAMETER_VERSION.to_owned(),
    };
    let event_payload = serde_json::to_string(&ReviewEventPayload {
        id: &event_id,
        account_id: &input.account_id,
        profile_id: &input.profile_id,
        problem_id: &input.problem_id,
        device_id: &input.device_id,
        rating: rating_label,
        duration_ms: input.duration_ms,
        occurred_at_utc_ms: input.occurred_at_utc_ms,
        algorithm_version: ALGORITHM_VERSION,
        parameter_version: PARAMETER_VERSION,
    })?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'review_event', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), input.account_id, input.profile_id, event_id, event_payload, input.occurred_at_utc_ms],
    )?;
    let advanced = transaction.execute(
        "UPDATE review_sessions
         SET current_index = current_index + 1,
             status = CASE WHEN current_index + 1 >= json_array_length(problem_ids_json) THEN 'completed' ELSE 'active' END,
             updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'
           AND json_extract(problem_ids_json, '$[' || current_index || ']') = ?4",
        params![
            input.occurred_at_utc_ms,
            input.account_id,
            input.profile_id,
            input.problem_id
        ],
    )?;
    if advanced != 1 {
        return Err(ReviewUseCaseError::SessionOutOfSync);
    }
    transaction.commit()?;

    Ok(result)
}

fn rebuild_schedule(events: &[StoredEvent]) -> Result<(MemoryState, i64), fsrs::FSRSError> {
    let fsrs = FSRS::default();
    let mut memory = None;
    let mut previous_at: Option<i64> = None;
    let mut final_state: Option<ItemState> = None;

    for event in events {
        let _stable_tiebreaker = &event.id;
        let elapsed_days = previous_at
            .map(|previous| ((event.occurred_at_utc_ms - previous) / MILLIS_PER_DAY).max(0) as u32)
            .unwrap_or(0);
        let next = fsrs.next_states(memory, DESIRED_RETENTION, elapsed_days)?;
        let selected = match event.rating {
            FsrsRating::Again => next.again,
            FsrsRating::Hard => next.hard,
            FsrsRating::Good => next.good,
            FsrsRating::Easy => next.easy,
        };
        memory = Some(selected.memory);
        final_state = Some(selected);
        previous_at = Some(event.occurred_at_utc_ms);
    }

    let state = final_state.expect("schedule rebuild always receives the inserted event");
    let interval_days = state.interval.round().max(1.0) as i64;
    let due_at =
        previous_at.expect("review event has a timestamp") + interval_days * MILLIS_PER_DAY;
    Ok((state.memory, due_at))
}

const fn rating_label(rating: FsrsRating) -> &'static str {
    match rating {
        FsrsRating::Again => "again",
        FsrsRating::Hard => "hard",
        FsrsRating::Good => "good",
        FsrsRating::Easy => "easy",
    }
}

fn parse_rating(label: &str) -> FsrsRating {
    match label {
        "again" => FsrsRating::Again,
        "hard" => FsrsRating::Hard,
        "easy" => FsrsRating::Easy,
        _ => FsrsRating::Good,
    }
}
