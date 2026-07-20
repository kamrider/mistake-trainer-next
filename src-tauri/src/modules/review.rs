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

#[derive(Clone, Debug)]
pub struct StartExamReview {
    pub account_id: String,
    pub profile_id: String,
    pub problem_ids: Vec<String>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct NavigateExam {
    pub account_id: String,
    pub profile_id: String,
    pub position: i32,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct BeginExamGrading {
    pub account_id: String,
    pub profile_id: String,
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
    pub exam_phase: Option<String>,
    pub exam_question_index: i32,
    pub exam_correct_count: i32,
    pub exam_wrong_count: i32,
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
    #[error("exam selection is invalid")]
    InvalidExamSelection,
    #[error("exam session is missing or in the wrong phase")]
    InvalidExamState,
}

#[derive(Debug)]
struct ActiveSession {
    id: String,
    source_mode: String,
    ids_json: String,
    current_index: i64,
    experience: String,
    exam_phase: Option<String>,
    exam_question_index: i64,
    exam_correct_count: i64,
    exam_wrong_count: i64,
}

fn active_queue_state(
    transaction: &Transaction<'_>,
    query: &ReviewQueueQuery,
    resumed: bool,
) -> Result<Option<ReviewQueueState>, ReviewUseCaseError> {
    let active = transaction
        .query_row(
            "SELECT id, mode, problem_ids_json, current_index, experience, exam_phase,
                    exam_question_index, exam_correct_count, exam_wrong_count
             FROM review_sessions
             WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'",
            params![query.account_id, query.profile_id],
            |row| {
                Ok(ActiveSession {
                    id: row.get(0)?,
                    source_mode: row.get(1)?,
                    ids_json: row.get(2)?,
                    current_index: row.get(3)?,
                    experience: row.get(4)?,
                    exam_phase: row.get(5)?,
                    exam_question_index: row.get(6)?,
                    exam_correct_count: row.get(7)?,
                    exam_wrong_count: row.get(8)?,
                })
            },
        )
        .optional()?;
    let Some(active) = active else {
        return Ok(None);
    };

    let ids: Vec<String> = serde_json::from_str(&active.ids_json)?;
    let is_exam = active.experience == "exam";
    let answering = is_exam && active.exam_phase.as_deref() == Some("answering");
    let selected_question_id = if answering {
        usize::try_from(active.exam_question_index)
            .ok()
            .and_then(|index| ids.get(index))
            .cloned()
    } else {
        None
    };
    let stored_index = usize::try_from(active.current_index).unwrap_or(ids.len());
    let safe_index = if answering {
        0
    } else {
        stored_index.min(ids.len())
    };
    let entries = queue_entries_for_ids(
        transaction,
        &query.account_id,
        &query.profile_id,
        &ids[safe_index..],
    )?;
    let mut cleaned_ids = ids[..safe_index].to_vec();
    cleaned_ids.extend(entries.iter().map(|entry| entry.problem_id.clone()));
    let completed_count = count_for_ui(safe_index);
    let total_count = count_for_ui(cleaned_ids.len());
    let has_remaining_items = !entries.is_empty();
    let question_index = if answering && has_remaining_items {
        let fallback = usize::try_from(active.exam_question_index)
            .unwrap_or_default()
            .min(entries.len().saturating_sub(1));
        let selected = selected_question_id
            .as_ref()
            .and_then(|selected_id| {
                entries
                    .iter()
                    .position(|entry| &entry.problem_id == selected_id)
            })
            .unwrap_or(fallback);
        i64::try_from(selected).unwrap_or(i64::MAX)
    } else {
        0
    };
    transaction.execute(
        "UPDATE review_sessions
         SET problem_ids_json = ?1,
             current_index = ?2,
             exam_question_index = ?3,
             status = CASE WHEN ?4 = 0 THEN 'completed' ELSE 'active' END,
             updated_at_utc_ms = ?5
         WHERE id = ?6",
        params![
            serde_json::to_string(&cleaned_ids)?,
            completed_count,
            question_index,
            i32::from(has_remaining_items),
            query.now_utc_ms,
            active.id,
        ],
    )?;

    Ok(Some(ReviewQueueState {
        session_id: Some(active.id),
        mode: if is_exam {
            "exam".to_owned()
        } else {
            active.source_mode
        },
        resumed,
        completed_count,
        total_count,
        exam_phase: active.exam_phase,
        exam_question_index: count_for_ui(usize::try_from(question_index).unwrap_or_default()),
        exam_correct_count: count_for_ui(
            usize::try_from(active.exam_correct_count).unwrap_or_default(),
        ),
        exam_wrong_count: count_for_ui(
            usize::try_from(active.exam_wrong_count).unwrap_or_default(),
        ),
        items: entries,
    }))
}

pub fn list_review_queue(
    connection: &mut Connection,
    query: ReviewQueueQuery,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    let transaction = connection.transaction()?;
    if let Some(state) = active_queue_state(&transaction, &query, true)? {
        transaction.commit()?;
        return Ok(state);
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
        exam_phase: None,
        exam_question_index: 0,
        exam_correct_count: 0,
        exam_wrong_count: 0,
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
        exam_phase: None,
        exam_question_index: 0,
        exam_correct_count: 0,
        exam_wrong_count: 0,
        items: entries,
    })
}

pub fn start_exam_review_queue(
    connection: &mut Connection,
    input: StartExamReview,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    if !(1..=100).contains(&input.problem_ids.len())
        || input.problem_ids.iter().collect::<HashSet<_>>().len() != input.problem_ids.len()
    {
        return Err(ReviewUseCaseError::InvalidExamSelection);
    }
    let transaction = connection.transaction()?;
    let entries = queue_entries_for_ids(
        &transaction,
        &input.account_id,
        &input.profile_id,
        &input.problem_ids,
    )?;
    if entries.len() != input.problem_ids.len() {
        return Err(ReviewUseCaseError::InvalidExamSelection);
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
             status, created_at_utc_ms, updated_at_utc_ms, experience, exam_phase,
             exam_question_index, exam_correct_count, exam_wrong_count
         ) VALUES(?1, ?2, ?3, 'manual', ?4, 0, 'active', ?5, ?5,
                  'exam', 'answering', 0, 0, 0)",
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
        mode: "exam".to_owned(),
        resumed: false,
        completed_count: 0,
        total_count: count_for_ui(entries.len()),
        exam_phase: Some("answering".to_owned()),
        exam_question_index: 0,
        exam_correct_count: 0,
        exam_wrong_count: 0,
        items: entries,
    })
}

pub fn navigate_exam(
    connection: &mut Connection,
    input: NavigateExam,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    if input.position < 0 {
        return Err(ReviewUseCaseError::InvalidExamState);
    }
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE review_sessions
         SET exam_question_index = ?1, updated_at_utc_ms = ?2
         WHERE account_id = ?3 AND profile_id = ?4 AND status = 'active'
           AND experience = 'exam' AND exam_phase = 'answering'
           AND ?1 < json_array_length(problem_ids_json)",
        params![
            input.position,
            input.now_utc_ms,
            input.account_id,
            input.profile_id,
        ],
    )?;
    if changed != 1 {
        return Err(ReviewUseCaseError::InvalidExamState);
    }
    let query = ReviewQueueQuery {
        account_id: input.account_id,
        profile_id: input.profile_id,
        now_utc_ms: input.now_utc_ms,
    };
    let state = active_queue_state(&transaction, &query, false)?
        .ok_or(ReviewUseCaseError::InvalidExamState)?;
    transaction.commit()?;
    Ok(state)
}

pub fn begin_exam_grading(
    connection: &mut Connection,
    input: BeginExamGrading,
) -> Result<ReviewQueueState, ReviewUseCaseError> {
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE review_sessions
         SET exam_phase = 'grading', current_index = 0, exam_question_index = 0,
             updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'
           AND experience = 'exam' AND exam_phase = 'answering'
           AND exam_question_index + 1 = json_array_length(problem_ids_json)",
        params![input.now_utc_ms, input.account_id, input.profile_id],
    )?;
    if changed != 1 {
        return Err(ReviewUseCaseError::InvalidExamState);
    }
    let query = ReviewQueueQuery {
        account_id: input.account_id,
        profile_id: input.profile_id,
        now_utc_ms: input.now_utc_ms,
    };
    let state = active_queue_state(&transaction, &query, false)?
        .ok_or(ReviewUseCaseError::InvalidExamState)?;
    transaction.commit()?;
    Ok(state)
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
    let exam_answer_correct = i32::from(matches!(rating, FsrsRating::Good | FsrsRating::Easy));
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
             exam_correct_count = exam_correct_count
                 + CASE WHEN experience = 'exam' AND ?5 = 1 THEN 1 ELSE 0 END,
             exam_wrong_count = exam_wrong_count
                 + CASE WHEN experience = 'exam' AND ?5 = 0 THEN 1 ELSE 0 END,
             updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'
           AND (experience != 'exam' OR exam_phase = 'grading')
           AND json_extract(problem_ids_json, '$[' || current_index || ']') = ?4",
        params![
            input.occurred_at_utc_ms,
            input.account_id,
            input.profile_id,
            input.problem_id,
            exam_answer_correct,
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
