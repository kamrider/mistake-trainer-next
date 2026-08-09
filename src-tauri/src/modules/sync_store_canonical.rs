use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    PendingAssetTransfer, SyncStoreError, WireAsset, WireEntity, WireExportSnapshot,
    WireProblemAggregate, WireProblemAsset, WireProfile, WireReviewEvent, WireTombstone,
    validate_uuid,
};

#[derive(Debug)]
pub(super) struct OperationRow {
    pub(super) id: String,
    pub(super) profile_id: Option<String>,
    pub(super) entity_type: String,
    pub(super) entity_id: String,
    pub(super) operation: String,
}

pub(super) fn select_due_operations(
    transaction: &Transaction<'_>,
    account_id: &str,
    now_utc_ms: i64,
    limit: usize,
) -> Result<Vec<OperationRow>, SyncStoreError> {
    let mut statement = transaction.prepare(
        "SELECT id, profile_id, entity_type, entity_id, operation
         FROM sync_operations
         WHERE account_id = ?1 AND status IN ('pending', 'failed')
           AND next_attempt_at_utc_ms <= ?2
         ORDER BY
           CASE
             WHEN operation = 'delete' THEN 90
             WHEN entity_type = 'learner_profile' THEN 10
             WHEN entity_type = 'asset' THEN 20
             WHEN entity_type = 'problem' THEN 30
             WHEN entity_type = 'review_event' THEN 40
             WHEN entity_type = 'export_snapshot' THEN 50
             ELSE 80
           END,
           created_at_utc_ms, id
         LIMIT ?3",
    )?;
    Ok(statement
        .query_map(params![account_id, now_utc_ms, limit as i64], |row| {
            Ok(OperationRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                entity_type: row.get(2)?,
                entity_id: row.get(3)?,
                operation: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

pub(super) fn canonical_entity(
    transaction: &Transaction<'_>,
    account_id: &str,
    remote_user_id: &str,
    row: &OperationRow,
) -> Result<(WireEntity, Option<PendingAssetTransfer>), SyncStoreError> {
    validate_uuid(&row.id)?;
    validate_uuid(&row.entity_id)?;
    if let Some(profile_id) = &row.profile_id {
        validate_uuid(profile_id)?;
    }
    if row.operation == "delete" {
        return Ok((load_tombstone(transaction, account_id, row)?, None));
    }
    match row.entity_type.as_str() {
        "learner_profile" => Ok((load_profile(transaction, account_id, row)?, None)),
        "asset" => {
            let (entity, transfer) = load_asset(transaction, account_id, remote_user_id, row)?;
            Ok((entity, Some(transfer)))
        }
        "problem" => Ok((load_problem(transaction, account_id, row)?, None)),
        "review_event" => Ok((load_review_event(transaction, account_id, row)?, None)),
        "export_snapshot" => Ok((load_export_snapshot(transaction, account_id, row)?, None)),
        _ => Err(SyncStoreError::MissingCanonicalEntity),
    }
}

fn load_profile(
    transaction: &Transaction<'_>,
    account_id: &str,
    row: &OperationRow,
) -> Result<WireEntity, SyncStoreError> {
    if row.profile_id.as_deref() != Some(row.entity_id.as_str()) {
        return Err(SyncStoreError::BoundaryViolation);
    }
    transaction
        .query_row(
            "SELECT id, name, revision, created_at_utc_ms, updated_at_utc_ms
             FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
            params![row.entity_id, account_id],
            |record| {
                Ok(WireEntity::LearnerProfile(WireProfile {
                    id: record.get(0)?,
                    name: record.get(1)?,
                    revision: record.get(2)?,
                    created_at_utc_ms: record.get(3)?,
                    updated_at_utc_ms: record.get(4)?,
                }))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)
}

fn load_asset(
    transaction: &Transaction<'_>,
    account_id: &str,
    remote_user_id: &str,
    row: &OperationRow,
) -> Result<(WireEntity, PendingAssetTransfer), SyncStoreError> {
    let asset = transaction
        .query_row(
            "SELECT id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms
             FROM assets WHERE id = ?1 AND account_id = ?2",
            params![row.entity_id, account_id],
            |record| {
                Ok((
                    record.get::<_, String>(0)?,
                    record.get::<_, String>(1)?,
                    record.get::<_, String>(2)?,
                    record.get::<_, i64>(3)?,
                    record.get::<_, String>(4)?,
                    record.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)?;
    let storage_object = format!("{remote_user_id}/{}", asset.1);
    Ok((
        WireEntity::Asset(WireAsset {
            id: asset.0.clone(),
            plaintext_sha256: asset.1.clone(),
            storage_object: storage_object.clone(),
            byte_length: asset.3,
            media_type: asset.4.clone(),
            revision: 1,
            created_at_utc_ms: asset.5,
        }),
        PendingAssetTransfer {
            asset_id: asset.0,
            plaintext_sha256: asset.1,
            encrypted_path: asset.2,
            byte_length: asset.3,
            media_type: asset.4,
            storage_object,
        },
    ))
}

fn load_problem(
    transaction: &Transaction<'_>,
    account_id: &str,
    row: &OperationRow,
) -> Result<WireEntity, SyncStoreError> {
    let profile_id = row
        .profile_id
        .as_deref()
        .ok_or(SyncStoreError::BoundaryViolation)?;
    let problem = transaction
        .query_row(
            "SELECT id, profile_id, subject, tags_json, note, status, time_limit_seconds,
                    revision, created_at_utc_ms, updated_at_utc_ms
             FROM problems WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![row.entity_id, account_id, profile_id],
            |record| {
                Ok((
                    record.get::<_, String>(0)?,
                    record.get::<_, String>(1)?,
                    record.get::<_, String>(2)?,
                    record.get::<_, String>(3)?,
                    record.get::<_, String>(4)?,
                    record.get::<_, String>(5)?,
                    record.get::<_, Option<i64>>(6)?,
                    record.get::<_, i64>(7)?,
                    record.get::<_, i64>(8)?,
                    record.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)?;
    let mut statement = transaction.prepare(
        "SELECT link.asset_id, link.role, link.position
         FROM problem_assets link
         JOIN assets asset ON asset.id = link.asset_id AND asset.account_id = ?2
         WHERE link.problem_id = ?1
         ORDER BY CASE link.role WHEN 'question' THEN 0 ELSE 1 END, link.position, link.asset_id",
    )?;
    let assets = statement
        .query_map(params![row.entity_id, account_id], |record| {
            Ok(WireProblemAsset {
                asset_id: record.get(0)?,
                role: record.get(1)?,
                position: record.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WireEntity::Problem(WireProblemAggregate {
        id: problem.0,
        profile_id: problem.1,
        subject: problem.2,
        tags: serde_json::from_str(&problem.3)?,
        note: problem.4,
        status: if problem.5 == "trashed" {
            "deleted".to_owned()
        } else {
            problem.5
        },
        time_limit_seconds: problem.6,
        assets,
        revision: problem.7,
        created_at_utc_ms: problem.8,
        updated_at_utc_ms: problem.9,
    }))
}

fn load_review_event(
    transaction: &Transaction<'_>,
    account_id: &str,
    row: &OperationRow,
) -> Result<WireEntity, SyncStoreError> {
    let profile_id = row
        .profile_id
        .as_deref()
        .ok_or(SyncStoreError::BoundaryViolation)?;
    transaction
        .query_row(
            "SELECT id, profile_id, problem_id, device_id, rating, duration_ms,
                    occurred_at_utc_ms, algorithm_version, parameter_version
             FROM review_events WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![row.entity_id, account_id, profile_id],
            |record| {
                Ok(WireEntity::ReviewEvent(WireReviewEvent {
                    id: record.get(0)?,
                    profile_id: record.get(1)?,
                    problem_id: record.get(2)?,
                    device_id: record.get(3)?,
                    rating: record.get(4)?,
                    duration_ms: record.get(5)?,
                    occurred_at_utc_ms: record.get(6)?,
                    algorithm_version: record.get(7)?,
                    parameter_version: record.get(8)?,
                }))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)
}

fn load_export_snapshot(
    transaction: &Transaction<'_>,
    account_id: &str,
    row: &OperationRow,
) -> Result<WireEntity, SyncStoreError> {
    let profile_id = row
        .profile_id
        .as_deref()
        .ok_or(SyncStoreError::BoundaryViolation)?;
    let export = transaction
        .query_row(
            "SELECT id, profile_id, title, problem_ids_json, configuration_json, revision, created_at_utc_ms
             FROM export_snapshots WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![row.entity_id, account_id, profile_id],
            |record| {
                Ok((record.get::<_, String>(0)?, record.get::<_, String>(1)?,
                    record.get::<_, String>(2)?, record.get::<_, String>(3)?,
                    record.get::<_, String>(4)?, record.get::<_, i64>(5)?, record.get::<_, i64>(6)?))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)?;
    Ok(WireEntity::ExportSnapshot(WireExportSnapshot {
        id: export.0,
        profile_id: export.1,
        title: export.2,
        problem_ids: serde_json::from_str(&export.3)?,
        configuration: serde_json::from_str(&export.4)?,
        revision: export.5,
        created_at_utc_ms: export.6,
    }))
}

fn load_tombstone(
    transaction: &Transaction<'_>,
    account_id: &str,
    row: &OperationRow,
) -> Result<WireEntity, SyncStoreError> {
    transaction
        .query_row(
            "SELECT id, profile_id, entity_type, entity_id, deleted_at_utc_ms,
                    purge_after_utc_ms, revision
             FROM tombstones
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3
               AND (profile_id = ?4 OR (profile_id IS NULL AND ?4 IS NULL))",
            params![account_id, row.entity_type, row.entity_id, row.profile_id],
            |record| {
                Ok(WireEntity::Tombstone(WireTombstone {
                    tombstone_id: record.get(0)?,
                    profile_id: record.get(1)?,
                    entity_type: record.get(2)?,
                    entity_id: record.get(3)?,
                    deleted_at_utc_ms: record.get(4)?,
                    purge_after_utc_ms: record.get(5)?,
                    deleted_revision: record.get(6)?,
                }))
            },
        )
        .optional()?
        .ok_or(SyncStoreError::MissingCanonicalEntity)
}
