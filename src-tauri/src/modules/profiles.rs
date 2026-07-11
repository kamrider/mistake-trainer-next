use rusqlite::{Connection, params};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::profile::{ProfileName, ProfileNameError};

#[derive(Clone, Debug)]
pub struct CreateProfile {
    pub account_id: String,
    pub name: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
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
