use fsrs::{FSRS, ItemState, MemoryState};
use rusqlite::{Connection, params};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::review::{FsrsRating, SimpleRating};

const DESIRED_RETENTION: f32 = 0.90;
const MILLIS_PER_DAY: i64 = 86_400_000;
const ALGORITHM_VERSION: &str = "fsrs-6.6.1";
const PARAMETER_VERSION: &str = "default-6.6.1";

#[derive(Clone, Debug)]
pub struct SubmitReview {
    pub account_id: String,
    pub profile_id: String,
    pub problem_id: String,
    pub device_id: String,
    pub rating: SimpleRating,
    pub duration_ms: u32,
    pub occurred_at_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSubmission {
    pub event_id: String,
    pub problem_id: String,
    pub rating: String,
    pub due_at_utc_ms: i64,
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
}

#[derive(Clone)]
struct StoredEvent {
    id: String,
    rating: FsrsRating,
    occurred_at_utc_ms: i64,
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
    let rating = input.rating.into_fsrs();
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
        due_at_utc_ms,
        stability: state.stability,
        difficulty: state.difficulty,
        algorithm_version: ALGORITHM_VERSION.to_owned(),
        parameter_version: PARAMETER_VERSION.to_owned(),
    };
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'review_event', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), input.account_id, input.profile_id, event_id, serde_json::to_string(&result)?, input.occurred_at_utc_ms],
    )?;
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
