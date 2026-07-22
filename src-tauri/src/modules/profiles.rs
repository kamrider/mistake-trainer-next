use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::profile::{ProfileName, ProfileNameError};

const DELETION_RETENTION_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

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

#[derive(Clone, Debug)]
pub struct DeleteProfile {
    pub account_id: String,
    pub profile_id: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanAsset {
    pub id: String,
    pub encrypted_path: String,
}

#[derive(Clone, Debug)]
pub struct DeleteProfileReceipt {
    pub deleted_profile_id: String,
    pub active_profile: LearnerProfile,
    pub orphan_assets: Vec<OrphanAsset>,
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
    #[error("the last profile cannot be deleted")]
    LastProfile,
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

pub fn delete_profile(
    connection: &mut Connection,
    input: DeleteProfile,
) -> Result<DeleteProfileReceipt, ProfileUseCaseError> {
    let transaction = connection.transaction()?;
    let target = transaction
        .query_row(
            "SELECT id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             FROM learner_profiles WHERE account_id = ?1 AND id = ?2",
            params![input.account_id, input.profile_id],
            learner_profile_from_row,
        )
        .optional()?
        .ok_or(ProfileUseCaseError::NotFound)?;
    let remaining_profiles = {
        let mut statement = transaction.prepare(
            "SELECT id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             FROM learner_profiles
             WHERE account_id = ?1 AND id <> ?2
             ORDER BY created_at_utc_ms, id",
        )?;
        statement
            .query_map(
                params![input.account_id, input.profile_id],
                learner_profile_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?
    };
    let replacement = remaining_profiles
        .first()
        .cloned()
        .ok_or(ProfileUseCaseError::LastProfile)?;
    let preferred_profile_id = transaction
        .query_row(
            "SELECT active_profile_id FROM account_preferences WHERE account_id = ?1",
            [&input.account_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let active_profile = preferred_profile_id
        .as_deref()
        .filter(|profile_id| *profile_id != target.id)
        .and_then(|profile_id| {
            remaining_profiles
                .iter()
                .find(|profile| profile.id == profile_id)
        })
        .cloned()
        .unwrap_or_else(|| replacement.clone());
    let candidate_assets = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT a.id, a.encrypted_path
             FROM assets a
             WHERE a.account_id = ?1 AND a.id IN (
               SELECT link.asset_id
               FROM problem_assets link
               JOIN problems problem ON problem.id = link.problem_id
               WHERE problem.account_id = ?1 AND problem.profile_id = ?2
               UNION
               SELECT item.asset_id
               FROM capture_items item
               JOIN capture_batches batch ON batch.id = item.batch_id
               WHERE batch.account_id = ?1 AND batch.profile_id = ?2
             )
             ORDER BY a.encrypted_path, a.id",
        )?;
        statement
            .query_map(params![input.account_id, input.profile_id], |row| {
                Ok(OrphanAsset {
                    id: row.get(0)?,
                    encrypted_path: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    transaction.execute(
        "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
         VALUES(?1, ?2, ?3)
         ON CONFLICT(account_id) DO UPDATE SET
           active_profile_id = CASE
             WHEN account_preferences.active_profile_id = ?4 THEN excluded.active_profile_id
             ELSE account_preferences.active_profile_id
           END,
           updated_at_utc_ms = CASE
             WHEN account_preferences.active_profile_id = ?4 THEN excluded.updated_at_utc_ms
             ELSE account_preferences.updated_at_utc_ms
           END",
        params![
            input.account_id,
            active_profile.id,
            input.now_utc_ms,
            input.profile_id
        ],
    )?;
    transaction.execute(
        "DELETE FROM sync_operations
         WHERE account_id = ?1 AND (
           (entity_type = 'learner_profile' AND entity_id = ?2)
           OR (profile_id = ?2 AND entity_type <> 'asset')
         )",
        params![input.account_id, input.profile_id],
    )?;
    transaction.execute(
        "UPDATE sync_operations SET profile_id = NULL
         WHERE account_id = ?1 AND profile_id = ?2 AND entity_type = 'asset'",
        params![input.account_id, input.profile_id],
    )?;
    transaction.execute(
        "DELETE FROM sync_conflicts WHERE account_id = ?1 AND profile_id = ?2",
        params![input.account_id, input.profile_id],
    )?;
    transaction.execute(
        "DELETE FROM tombstones WHERE account_id = ?1 AND profile_id = ?2",
        params![input.account_id, input.profile_id],
    )?;
    let deleted = transaction.execute(
        "DELETE FROM learner_profiles WHERE account_id = ?1 AND id = ?2",
        params![input.account_id, input.profile_id],
    )?;
    if deleted != 1 {
        return Err(ProfileUseCaseError::NotFound);
    }

    let purge_after_utc_ms = input.now_utc_ms.saturating_add(DELETION_RETENTION_MILLIS);
    let deleted_revision = target.revision.saturating_add(1);
    transaction.execute(
        "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
         VALUES(?1, ?2, NULL, 'learner_profile', ?3, ?4, ?5, ?6)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
           profile_id = NULL,
           deleted_at_utc_ms = excluded.deleted_at_utc_ms,
           purge_after_utc_ms = excluded.purge_after_utc_ms,
           revision = excluded.revision
         WHERE tombstones.account_id = excluded.account_id",
        params![
            Uuid::now_v7().to_string(),
            input.account_id,
            target.id,
            input.now_utc_ms,
            purge_after_utc_ms,
            deleted_revision
        ],
    )?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, NULL, 'learner_profile', ?3, 'delete', '{}', 'pending', 0, ?4, ?4)",
        params![
            Uuid::now_v7().to_string(),
            input.account_id,
            target.id,
            input.now_utc_ms
        ],
    )?;

    let mut orphan_assets = Vec::new();
    for candidate in candidate_assets {
        let removed = transaction.execute(
            "DELETE FROM assets
             WHERE id = ?1 AND account_id = ?2
               AND NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)
               AND NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)",
            params![candidate.id, input.account_id],
        )?;
        if removed == 0 {
            continue;
        }
        transaction.execute(
            "DELETE FROM sync_operations
             WHERE account_id = ?1 AND entity_type = 'asset' AND entity_id = ?2",
            params![input.account_id, candidate.id],
        )?;
        transaction.execute(
            "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
             VALUES(?1, ?2, NULL, 'asset', ?3, ?4, ?5, 1)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET
               profile_id = NULL,
               deleted_at_utc_ms = excluded.deleted_at_utc_ms,
               purge_after_utc_ms = excluded.purge_after_utc_ms,
               revision = max(tombstones.revision, excluded.revision)
             WHERE tombstones.account_id = excluded.account_id",
            params![
                Uuid::now_v7().to_string(),
                input.account_id,
                candidate.id,
                input.now_utc_ms,
                purge_after_utc_ms
            ],
        )?;
        let asset_operation_time = input.now_utc_ms.saturating_add(1);
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
             VALUES(?1, ?2, NULL, 'asset', ?3, 'delete', '{}', 'pending', 0, ?4, ?4)",
            params![
                Uuid::now_v7().to_string(),
                input.account_id,
                candidate.id,
                asset_operation_time
            ],
        )?;
        orphan_assets.push(candidate);
    }

    transaction.commit()?;
    Ok(DeleteProfileReceipt {
        deleted_profile_id: target.id,
        active_profile,
        orphan_assets,
    })
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
