use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::profile::{ProfileName, ProfileNameError};

#[derive(Clone, Debug)]
pub struct CreateProfile {
    pub account_id: String,
    pub name: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct RenameProfile {
    pub account_id: String,
    pub profile_id: String,
    pub name: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LearnerProfile {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
    pub revision: i64,
}

#[derive(Debug, Error)]
pub enum ProfileUseCaseError {
    #[error(transparent)]
    InvalidName(#[from] ProfileNameError),
    #[error("profile name already exists")]
    DuplicateName,
    #[error("profile was not found")]
    NotFound,
    #[error("profile persistence failed")]
    Database(#[from] rusqlite::Error),
    #[error("profile outbox serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub fn create_profile(
    connection: &mut Connection,
    input: CreateProfile,
) -> Result<LearnerProfile, ProfileUseCaseError> {
    let name = ProfileName::parse(&input.name)?;
    if profile_name_exists(connection, &input.account_id, name.as_str(), None)? {
        return Err(ProfileUseCaseError::DuplicateName);
    }
    let profile = LearnerProfile {
        id: Uuid::now_v7().to_string(),
        account_id: input.account_id,
        name: name.as_str().to_owned(),
        created_at_utc_ms: input.now_utc_ms,
        updated_at_utc_ms: input.now_utc_ms,
        revision: 1,
    };
    let payload = serde_json::to_string(&profile)?;
    let operation_id = Uuid::now_v7().to_string();

    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            profile.id,
            profile.account_id,
            profile.name,
            profile.created_at_utc_ms,
            profile.updated_at_utc_ms,
            profile.revision,
        ],
    )?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'learner_profile', ?3, 'upsert', ?4, 'pending', 0, ?5, ?5)",
        params![operation_id, profile.account_id, profile.id, payload, input.now_utc_ms],
    )?;
    transaction.commit()?;

    Ok(profile)
}

pub fn list_profiles(
    connection: &Connection,
    account_id: &str,
) -> Result<Vec<LearnerProfile>, ProfileUseCaseError> {
    let mut statement = connection.prepare(
        "SELECT id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
         FROM learner_profiles
         WHERE account_id = ?1
         ORDER BY created_at_utc_ms, id",
    )?;
    Ok(statement
        .query_map([account_id], learner_profile_from_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

pub fn rename_profile(
    connection: &mut Connection,
    input: RenameProfile,
) -> Result<LearnerProfile, ProfileUseCaseError> {
    let name = ProfileName::parse(&input.name)?;
    if profile_name_exists(
        connection,
        &input.account_id,
        name.as_str(),
        Some(&input.profile_id),
    )? {
        return Err(ProfileUseCaseError::DuplicateName);
    }

    let transaction = connection.transaction()?;
    let updated = transaction.execute(
        "UPDATE learner_profiles
         SET name = ?3, updated_at_utc_ms = ?4, revision = revision + 1
         WHERE account_id = ?1 AND id = ?2",
        params![
            input.account_id,
            input.profile_id,
            name.as_str(),
            input.now_utc_ms
        ],
    )?;
    if updated == 0 {
        return Err(ProfileUseCaseError::NotFound);
    }
    let profile = transaction.query_row(
        "SELECT id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
         FROM learner_profiles WHERE account_id = ?1 AND id = ?2",
        params![input.account_id, input.profile_id],
        learner_profile_from_row,
    )?;
    let payload = serde_json::to_string(&profile)?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, ?3, 'learner_profile', ?3, 'upsert', ?4, 'pending', 0, ?5, ?5)",
        params![
            Uuid::now_v7().to_string(),
            profile.account_id,
            profile.id,
            payload,
            input.now_utc_ms
        ],
    )?;
    transaction.commit()?;
    Ok(profile)
}

pub fn persist_active_profile(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
) -> Result<LearnerProfile, ProfileUseCaseError> {
    let transaction = connection.transaction()?;
    let profile = transaction
        .query_row(
            "SELECT id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             FROM learner_profiles WHERE account_id = ?1 AND id = ?2",
            params![account_id, profile_id],
            learner_profile_from_row,
        )
        .optional()?
        .ok_or(ProfileUseCaseError::NotFound)?;
    transaction.execute(
        "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
         VALUES(?1, ?2, ?3)
         ON CONFLICT(account_id) DO UPDATE SET
           active_profile_id = excluded.active_profile_id,
           updated_at_utc_ms = excluded.updated_at_utc_ms",
        params![account_id, profile_id, now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(profile)
}

fn profile_name_exists(
    connection: &Connection,
    account_id: &str,
    name: &str,
    excluded_profile_id: Option<&str>,
) -> Result<bool, rusqlite::Error> {
    connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM learner_profiles
           WHERE account_id = ?1 AND name = ?2
             AND (?3 IS NULL OR id <> ?3)
         )",
        params![account_id, name, excluded_profile_id],
        |row| row.get::<_, i64>(0).map(|exists| exists != 0),
    )
}

fn learner_profile_from_row(row: &rusqlite::Row<'_>) -> Result<LearnerProfile, rusqlite::Error> {
    Ok(LearnerProfile {
        id: row.get(0)?,
        account_id: row.get(1)?,
        name: row.get(2)?,
        created_at_utc_ms: row.get(3)?,
        updated_at_utc_ms: row.get(4)?,
        revision: row.get(5)?,
    })
}
