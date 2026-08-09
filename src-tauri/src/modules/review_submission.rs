use fsrs::{FSRS, ItemState, MemoryState};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use uuid::Uuid;

use crate::{domain::review::FsrsRating, modules::review_focus::start_interval_focus_if_due};

use super::{
    ALGORITHM_VERSION, PARAMETER_VERSION, ReviewSubmission, ReviewUseCaseError, SubmitReview,
};

const DESIRED_RETENTION: f32 = 0.90;
const MILLIS_PER_DAY: i64 = 86_400_000;

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

pub(super) fn submit_review(
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
    let exam_answer_correct = i32::from(matches!(rating, FsrsRating::Good | FsrsRating::Easy));
    let transaction = connection.transaction()?;
    let session_id = transaction
        .query_row(
            "SELECT id FROM review_sessions
             WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'
               AND focus_order_json IS NULL
               AND (experience != 'exam' OR exam_phase = 'grading')
               AND json_extract(problem_ids_json, '$[' || current_index || ']') = ?3",
            params![input.account_id, input.profile_id, input.problem_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(ReviewUseCaseError::SessionOutOfSync)?;
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
             exam_correct_count = exam_correct_count
                 + CASE WHEN experience = 'exam' AND ?5 = 1 THEN 1 ELSE 0 END,
             exam_wrong_count = exam_wrong_count
                 + CASE WHEN experience = 'exam' AND ?5 = 0 THEN 1 ELSE 0 END,
             updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'
           AND id = ?6 AND focus_order_json IS NULL
           AND (experience != 'exam' OR exam_phase = 'grading')
           AND json_extract(problem_ids_json, '$[' || current_index || ']') = ?4",
        params![
            input.occurred_at_utc_ms,
            input.account_id,
            input.profile_id,
            input.problem_id,
            exam_answer_correct,
            session_id,
        ],
    )?;
    if advanced != 1 {
        return Err(ReviewUseCaseError::SessionOutOfSync);
    }
    let focus = start_interval_focus_if_due(&transaction, &session_id, input.occurred_at_utc_ms)?;
    let result = ReviewSubmission {
        event_id: event_id.clone(),
        problem_id: input.problem_id.clone(),
        rating: rating_label.to_owned(),
        due_at_utc_ms: due_at_utc_ms as f64,
        stability: state.stability,
        difficulty: state.difficulty,
        algorithm_version: ALGORITHM_VERSION.to_owned(),
        parameter_version: PARAMETER_VERSION.to_owned(),
        focus,
    };
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

pub(super) fn rebuild_schedule_for_problem(
    transaction: &Transaction<'_>,
    account_id: &str,
    problem_id: &str,
    rebuilt_at_utc_ms: i64,
) -> Result<(), ReviewUseCaseError> {
    let mut statement = transaction.prepare(
        "SELECT id, rating, occurred_at_utc_ms
         FROM review_events
         WHERE account_id = ?1 AND problem_id = ?2
         ORDER BY occurred_at_utc_ms, id",
    )?;
    let events = statement
        .query_map(params![account_id, problem_id], |row| {
            let label: String = row.get(1)?;
            Ok(StoredEvent {
                id: row.get(0)?,
                rating: parse_rating(&label),
                occurred_at_utc_ms: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if events.is_empty() {
        transaction.execute(
            "DELETE FROM schedule_states
             WHERE problem_id = ?1
               AND EXISTS (SELECT 1 FROM problems WHERE id = ?1 AND account_id = ?2)",
            params![problem_id, account_id],
        )?;
        return Ok(());
    }

    let (state, due_at_utc_ms) = rebuild_schedule(&events)?;
    let last_reviewed_at_utc_ms = events
        .last()
        .map(|event| event.occurred_at_utc_ms)
        .expect("non-empty review events");
    transaction.execute(
        "INSERT INTO schedule_states(
            problem_id, due_at_utc_ms, stability, difficulty,
            last_reviewed_at_utc_ms, algorithm_version, parameter_version,
            rebuilt_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(problem_id) DO UPDATE SET
            due_at_utc_ms = excluded.due_at_utc_ms,
            stability = excluded.stability,
            difficulty = excluded.difficulty,
            last_reviewed_at_utc_ms = excluded.last_reviewed_at_utc_ms,
            algorithm_version = excluded.algorithm_version,
            parameter_version = excluded.parameter_version,
            rebuilt_at_utc_ms = excluded.rebuilt_at_utc_ms",
        params![
            problem_id,
            due_at_utc_ms,
            f64::from(state.stability),
            f64::from(state.difficulty),
            last_reviewed_at_utc_ms,
            ALGORITHM_VERSION,
            PARAMETER_VERSION,
            rebuilt_at_utc_ms,
        ],
    )?;
    Ok(())
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
