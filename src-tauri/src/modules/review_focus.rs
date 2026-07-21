use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;

use crate::modules::preferences::ReviewFocusPolicy;

const BOARD_SIZE: usize = 25;
const MAX_ELAPSED_MS: u32 = 3_600_000;

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFocusState {
    pub kind: String,
    pub round_index: i32,
    pub numbers: Vec<i32>,
    pub next_number: i32,
    pub elapsed_ms: u32,
}

#[derive(Clone, Debug)]
pub struct FocusNumberSelection {
    pub account_id: String,
    pub profile_id: String,
    pub number: i32,
    pub elapsed_ms: u32,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct SkipReviewFocus {
    pub account_id: String,
    pub profile_id: String,
    pub now_utc_ms: i64,
}

#[derive(Debug, Error)]
pub enum ReviewFocusError {
    #[error("review focus state changed")]
    StateChanged,
    #[error("review focus persistence failed")]
    Database(#[from] rusqlite::Error),
    #[error("review focus state is corrupt")]
    CorruptState,
    #[error("review focus serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub(crate) fn focus_policy_for_profile(
    transaction: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
) -> Result<ReviewFocusPolicy, ReviewFocusError> {
    let value = transaction
        .query_row(
            "SELECT review_focus_policy FROM profile_preferences
             WHERE account_id = ?1 AND profile_id = ?2",
            params![account_id, profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match value.as_deref().unwrap_or("off") {
        "off" => Ok(ReviewFocusPolicy::Off),
        "session_start" => Ok(ReviewFocusPolicy::SessionStart),
        "every_10" => Ok(ReviewFocusPolicy::EveryTen),
        _ => Err(ReviewFocusError::CorruptState),
    }
}

pub(crate) fn initialize_session_focus(
    transaction: &Transaction<'_>,
    session_id: &str,
    policy: ReviewFocusPolicy,
    now_utc_ms: i64,
) -> Result<Option<ReviewFocusState>, ReviewFocusError> {
    if policy != ReviewFocusPolicy::SessionStart {
        return Ok(None);
    }
    let numbers = deterministic_board(session_id, 0);
    let encoded = serde_json::to_string(&numbers)?;
    let changed = transaction.execute(
        "UPDATE review_sessions
         SET focus_order_json = ?1, focus_next_number = 1, focus_elapsed_ms = 0,
             updated_at_utc_ms = ?2
         WHERE id = ?3 AND status = 'active' AND experience = 'review'
           AND focus_policy = 'session_start' AND focus_order_json IS NULL",
        params![encoded, now_utc_ms, session_id],
    )?;
    if changed != 1 {
        return Err(ReviewFocusError::StateChanged);
    }
    Ok(Some(ReviewFocusState {
        kind: "warmup".to_owned(),
        round_index: 0,
        numbers,
        next_number: 1,
        elapsed_ms: 0,
    }))
}

pub(crate) fn active_focus_for_session(
    transaction: &Transaction<'_>,
    session_id: &str,
) -> Result<Option<ReviewFocusState>, ReviewFocusError> {
    let stored = transaction
        .query_row(
            "SELECT focus_policy, focus_round, focus_order_json,
                    focus_next_number, focus_elapsed_ms
             FROM review_sessions WHERE id = ?1 AND status = 'active'",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((policy, round_index, order_json, next_number, elapsed_ms)) = stored else {
        return Ok(None);
    };
    let Some(order_json) = order_json else {
        return Ok(None);
    };
    let numbers = validate_board(&order_json)?;
    let elapsed_ms = u32::try_from(elapsed_ms).map_err(|_| ReviewFocusError::CorruptState)?;
    if !(1..=25).contains(&next_number) || elapsed_ms > MAX_ELAPSED_MS {
        return Err(ReviewFocusError::CorruptState);
    }
    let kind = match policy.as_str() {
        "session_start" => "warmup",
        "every_10" => "break",
        _ => return Err(ReviewFocusError::CorruptState),
    };
    Ok(Some(ReviewFocusState {
        kind: kind.to_owned(),
        round_index,
        numbers,
        next_number,
        elapsed_ms,
    }))
}

pub(crate) fn start_interval_focus_if_due(
    transaction: &Transaction<'_>,
    session_id: &str,
    now_utc_ms: i64,
) -> Result<Option<ReviewFocusState>, ReviewFocusError> {
    let stored = transaction
        .query_row(
            "SELECT current_index, json_array_length(problem_ids_json), focus_round
         FROM review_sessions
         WHERE id = ?1 AND status = 'active' AND experience = 'review'
           AND focus_policy = 'every_10' AND focus_order_json IS NULL",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((current_index, total_count, round_index)) = stored else {
        return Ok(None);
    };
    if current_index <= 0
        || current_index >= total_count
        || current_index % 10 != 0
        || i64::from(round_index) >= current_index / 10
    {
        return Ok(None);
    }

    let numbers = deterministic_board(session_id, round_index);
    let encoded = serde_json::to_string(&numbers)?;
    let changed = transaction.execute(
        "UPDATE review_sessions
         SET focus_order_json = ?1, focus_next_number = 1, focus_elapsed_ms = 0,
             updated_at_utc_ms = ?2
         WHERE id = ?3 AND status = 'active' AND experience = 'review'
           AND focus_policy = 'every_10' AND focus_order_json IS NULL
           AND current_index = ?4 AND focus_round = ?5",
        params![encoded, now_utc_ms, session_id, current_index, round_index],
    )?;
    if changed != 1 {
        return Err(ReviewFocusError::StateChanged);
    }
    Ok(Some(ReviewFocusState {
        kind: "break".to_owned(),
        round_index,
        numbers,
        next_number: 1,
        elapsed_ms: 0,
    }))
}

pub fn select_focus_number(
    connection: &mut Connection,
    input: FocusNumberSelection,
) -> Result<Option<ReviewFocusState>, ReviewFocusError> {
    let transaction = connection.transaction()?;
    let session_id = active_focus_session_id(&transaction, &input.account_id, &input.profile_id)?;
    let current = active_focus_for_session(&transaction, &session_id)?
        .ok_or(ReviewFocusError::StateChanged)?;
    if input.number != current.next_number {
        return Err(ReviewFocusError::StateChanged);
    }
    let elapsed_ms = input.elapsed_ms.min(MAX_ELAPSED_MS);
    if input.number == 25 {
        let changed = transaction.execute(
            "UPDATE review_sessions
             SET focus_order_json = NULL, focus_next_number = 0,
                 focus_elapsed_ms = MAX(focus_elapsed_ms, ?1),
                 focus_round = focus_round + 1, updated_at_utc_ms = ?2
             WHERE id = ?3 AND status = 'active' AND focus_order_json IS NOT NULL
               AND focus_next_number = 25",
            params![i64::from(elapsed_ms), input.now_utc_ms, session_id],
        )?;
        if changed != 1 {
            return Err(ReviewFocusError::StateChanged);
        }
        transaction.commit()?;
        return Ok(None);
    }

    let changed = transaction.execute(
        "UPDATE review_sessions
         SET focus_next_number = focus_next_number + 1,
             focus_elapsed_ms = MAX(focus_elapsed_ms, ?1), updated_at_utc_ms = ?2
         WHERE id = ?3 AND status = 'active' AND focus_order_json IS NOT NULL
           AND focus_next_number = ?4",
        params![
            i64::from(elapsed_ms),
            input.now_utc_ms,
            session_id,
            input.number,
        ],
    )?;
    if changed != 1 {
        return Err(ReviewFocusError::StateChanged);
    }
    let next = ReviewFocusState {
        next_number: input.number + 1,
        elapsed_ms: current.elapsed_ms.max(elapsed_ms),
        ..current
    };
    transaction.commit()?;
    Ok(Some(next))
}

pub fn skip_focus_round(
    connection: &mut Connection,
    input: SkipReviewFocus,
) -> Result<Option<ReviewFocusState>, ReviewFocusError> {
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE review_sessions
         SET focus_order_json = NULL, focus_next_number = 0,
             focus_round = focus_round + 1, updated_at_utc_ms = ?1
         WHERE account_id = ?2 AND profile_id = ?3 AND status = 'active'
           AND focus_order_json IS NOT NULL",
        params![input.now_utc_ms, input.account_id, input.profile_id],
    )?;
    if changed != 1 {
        return Err(ReviewFocusError::StateChanged);
    }
    transaction.commit()?;
    Ok(None)
}

fn active_focus_session_id(
    transaction: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
) -> Result<String, ReviewFocusError> {
    transaction
        .query_row(
            "SELECT id FROM review_sessions
             WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'
               AND focus_order_json IS NOT NULL",
            params![account_id, profile_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ReviewFocusError::StateChanged)
}

fn deterministic_board(session_id: &str, round_index: i32) -> Vec<i32> {
    let mut ranked = (1..=25)
        .map(|number| {
            let seed = format!("{session_id}:{round_index}:{number}");
            let digest: [u8; 32] = Sha256::digest(seed.as_bytes()).into();
            (digest, number)
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(digest, _)| *digest);
    ranked.into_iter().map(|(_, number)| number).collect()
}

fn validate_board(encoded: &str) -> Result<Vec<i32>, ReviewFocusError> {
    let numbers = serde_json::from_str::<Vec<i32>>(encoded)?;
    let unique = numbers.iter().copied().collect::<HashSet<_>>();
    if numbers.len() != BOARD_SIZE
        || unique.len() != BOARD_SIZE
        || numbers.iter().any(|number| !(1..=25).contains(number))
    {
        return Err(ReviewFocusError::CorruptState);
    }
    Ok(numbers)
}
