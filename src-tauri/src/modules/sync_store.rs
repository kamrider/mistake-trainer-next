use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[path = "sync_store_canonical.rs"]
mod canonical;

const MAX_PUSH_OPERATIONS: usize = 100;
const LEASE_DURATION_MS: i64 = 5 * 60 * 1000;
const MAX_RETRY_DELAY_MS: i64 = 30 * 60 * 1000;
const REMOTE_FINGERPRINT_DOMAIN: &[u8] = b"mistake-trainer-next/cloud-user/v1\0";

#[derive(Debug, Error)]
pub enum SyncStoreError {
    #[error("sync database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("sync entity contains invalid JSON")]
    Json(#[from] serde_json::Error),
    #[error("sync identifier is invalid")]
    InvalidIdentifier,
    #[error("sync operation crosses an account or profile boundary")]
    BoundaryViolation,
    #[error("sync operation has no canonical local entity")]
    MissingCanonicalEntity,
    #[error("local library is bound to another remote account")]
    RemoteBindingMismatch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireProfile {
    pub id: String,
    pub name: String,
    pub revision: i64,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireAsset {
    pub id: String,
    pub plaintext_sha256: String,
    pub storage_object: String,
    pub byte_length: i64,
    pub media_type: String,
    pub revision: i64,
    pub created_at_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireProblemAsset {
    pub asset_id: String,
    pub role: String,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireProblemAggregate {
    pub id: String,
    pub profile_id: String,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
    pub status: String,
    pub time_limit_seconds: Option<i64>,
    pub assets: Vec<WireProblemAsset>,
    pub revision: i64,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireReviewEvent {
    pub id: String,
    pub profile_id: String,
    pub problem_id: String,
    pub device_id: String,
    pub rating: String,
    pub duration_ms: i64,
    pub occurred_at_utc_ms: i64,
    pub algorithm_version: String,
    pub parameter_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireExportSnapshot {
    pub id: String,
    pub profile_id: String,
    pub title: String,
    pub problem_ids: Vec<String>,
    pub configuration: serde_json::Value,
    pub revision: i64,
    pub created_at_utc_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTombstone {
    pub tombstone_id: String,
    pub profile_id: Option<String>,
    pub entity_type: String,
    pub entity_id: String,
    pub deleted_at_utc_ms: i64,
    pub purge_after_utc_ms: i64,
    pub deleted_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireEntity {
    LearnerProfile(WireProfile),
    Asset(WireAsset),
    Problem(WireProblemAggregate),
    ReviewEvent(WireReviewEvent),
    ExportSnapshot(WireExportSnapshot),
    Tombstone(WireTombstone),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireOperation {
    pub operation_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: String,
    pub payload: WireEntity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingAssetTransfer {
    pub asset_id: String,
    pub plaintext_sha256: String,
    pub encrypted_path: String,
    pub byte_length: i64,
    pub media_type: String,
    pub storage_object: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeasedPushBatch {
    pub lease_id: String,
    pub lease_expires_at_utc_ms: i64,
    pub operations: Vec<WireOperation>,
    pub assets: Vec<PendingAssetTransfer>,
}

pub fn bind_remote_identity(
    connection: &Connection,
    account_id: &str,
    remote_user_id: &str,
) -> Result<(), SyncStoreError> {
    validate_uuid(account_id)?;
    validate_uuid(remote_user_id)?;
    let fingerprint = remote_fingerprint(remote_user_id);
    let existing = connection
        .query_row(
            "SELECT remote_user_fingerprint FROM cloud_sync_state WHERE account_id = ?1",
            [account_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?;
    if existing.flatten().is_some_and(|value| value != fingerprint) {
        return Err(SyncStoreError::RemoteBindingMismatch);
    }
    connection.execute(
        "INSERT INTO cloud_sync_state(account_id, remote_user_fingerprint)
         VALUES(?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET remote_user_fingerprint = excluded.remote_user_fingerprint
         WHERE cloud_sync_state.remote_user_fingerprint IS NULL
            OR cloud_sync_state.remote_user_fingerprint = excluded.remote_user_fingerprint",
        params![account_id, fingerprint],
    )?;
    Ok(())
}

pub fn lease_push_batch(
    connection: &mut Connection,
    account_id: &str,
    remote_user_id: &str,
    now_utc_ms: i64,
    requested_limit: usize,
) -> Result<LeasedPushBatch, SyncStoreError> {
    validate_uuid(account_id)?;
    validate_uuid(remote_user_id)?;
    let limit = requested_limit.clamp(1, MAX_PUSH_OPERATIONS);
    let lease_id = Uuid::now_v7().to_string();
    let lease_expires_at_utc_ms = now_utc_ms.saturating_add(LEASE_DURATION_MS);
    let transaction = connection.transaction()?;
    bind_remote_identity(&transaction, account_id, remote_user_id)?;
    transaction.execute(
        "UPDATE sync_operations
         SET status = 'pending', lease_id = NULL, lease_expires_at_utc_ms = NULL
         WHERE account_id = ?1 AND status = 'processing'
           AND lease_expires_at_utc_ms IS NOT NULL AND lease_expires_at_utc_ms <= ?2",
        params![account_id, now_utc_ms],
    )?;

    let rows = canonical::select_due_operations(&transaction, account_id, now_utc_ms, limit)?;
    let mut operations = Vec::with_capacity(rows.len());
    let mut assets = Vec::new();
    let mut seen_asset_ids = HashSet::new();
    for row in &rows {
        let (payload, transfer) =
            canonical::canonical_entity(&transaction, account_id, remote_user_id, row)?;
        if let Some(transfer) = transfer
            && seen_asset_ids.insert(transfer.asset_id.clone())
        {
            assets.push(transfer);
        }
        operations.push(WireOperation {
            operation_id: row.id.clone(),
            entity_type: row.entity_type.clone(),
            entity_id: row.entity_id.clone(),
            operation: row.operation.clone(),
            payload,
        });
    }
    for row in &rows {
        let changed = transaction.execute(
            "UPDATE sync_operations
             SET status = 'processing', lease_id = ?1, lease_expires_at_utc_ms = ?2,
                 last_error_code = NULL
             WHERE id = ?3 AND account_id = ?4 AND status IN ('pending', 'failed')
               AND next_attempt_at_utc_ms <= ?5",
            params![
                lease_id,
                lease_expires_at_utc_ms,
                row.id,
                account_id,
                now_utc_ms
            ],
        )?;
        if changed != 1 {
            return Err(SyncStoreError::BoundaryViolation);
        }
    }
    transaction.commit()?;
    Ok(LeasedPushBatch {
        lease_id,
        lease_expires_at_utc_ms,
        operations,
        assets,
    })
}

pub fn acknowledge_push_batch(
    connection: &mut Connection,
    lease_id: &str,
    operation_ids: &[String],
) -> Result<usize, SyncStoreError> {
    validate_uuid(lease_id)?;
    if operation_ids.len() > MAX_PUSH_OPERATIONS {
        return Err(SyncStoreError::BoundaryViolation);
    }
    let transaction = connection.transaction()?;
    let mut deleted = 0;
    for operation_id in operation_ids {
        validate_uuid(operation_id)?;
        deleted += transaction.execute(
            "DELETE FROM sync_operations WHERE id = ?1 AND lease_id = ?2 AND status = 'processing'",
            params![operation_id, lease_id],
        )?;
    }
    transaction.commit()?;
    Ok(deleted)
}

pub fn fail_push_batch(
    connection: &mut Connection,
    lease_id: &str,
    error_code: &str,
    now_utc_ms: i64,
) -> Result<usize, SyncStoreError> {
    validate_uuid(lease_id)?;
    if error_code.is_empty() || error_code.len() > 80 {
        return Err(SyncStoreError::BoundaryViolation);
    }
    let transaction = connection.transaction()?;
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT id, attempt_count FROM sync_operations
             WHERE lease_id = ?1 AND status = 'processing' ORDER BY id",
        )?;
        statement
            .query_map([lease_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (operation_id, old_attempt) in &rows {
        let attempt = old_attempt.saturating_add(1);
        let delay = retry_delay_ms(attempt, operation_id);
        transaction.execute(
            "UPDATE sync_operations
             SET status = 'pending', attempt_count = ?1, next_attempt_at_utc_ms = ?2,
                 lease_id = NULL, lease_expires_at_utc_ms = NULL, last_error_code = ?3
             WHERE id = ?4 AND lease_id = ?5 AND status = 'processing'",
            params![
                attempt,
                now_utc_ms.saturating_add(delay),
                error_code,
                operation_id,
                lease_id
            ],
        )?;
    }
    transaction.commit()?;
    Ok(rows.len())
}

pub fn pull_cursor(connection: &Connection, account_id: &str) -> Result<i64, SyncStoreError> {
    validate_uuid(account_id)?;
    Ok(connection
        .query_row(
            "SELECT pull_cursor FROM cloud_sync_state WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

pub fn record_pull_success(
    connection: &Connection,
    account_id: &str,
    cursor: i64,
    now_utc_ms: i64,
) -> Result<(), SyncStoreError> {
    validate_uuid(account_id)?;
    if cursor < 0 {
        return Err(SyncStoreError::BoundaryViolation);
    }
    record_pull_success_inner(connection, account_id, cursor, now_utc_ms)
}

pub(crate) fn record_pull_success_tx(
    transaction: &Transaction<'_>,
    account_id: &str,
    cursor: i64,
    now_utc_ms: i64,
) -> Result<(), SyncStoreError> {
    validate_uuid(account_id)?;
    if cursor < 0 {
        return Err(SyncStoreError::BoundaryViolation);
    }
    record_pull_success_inner(transaction, account_id, cursor, now_utc_ms)
}

fn record_pull_success_inner(
    connection: &Connection,
    account_id: &str,
    cursor: i64,
    now_utc_ms: i64,
) -> Result<(), SyncStoreError> {
    connection.execute(
        "INSERT INTO cloud_sync_state(account_id, pull_cursor, last_attempt_at_utc_ms, last_success_at_utc_ms, last_error_code)
         VALUES(?1, ?2, ?3, ?3, NULL)
         ON CONFLICT(account_id) DO UPDATE SET
           pull_cursor = max(cloud_sync_state.pull_cursor, excluded.pull_cursor),
           last_attempt_at_utc_ms = excluded.last_attempt_at_utc_ms,
           last_success_at_utc_ms = excluded.last_success_at_utc_ms,
           last_error_code = NULL",
        params![account_id, cursor, now_utc_ms],
    )?;
    Ok(())
}

pub fn discard_expired_asset_transfers(
    connection: &Connection,
    now_utc_ms: i64,
) -> Result<usize, SyncStoreError> {
    Ok(connection.execute(
        "DELETE FROM cloud_asset_transfers WHERE expires_at_utc_ms <= ?1",
        [now_utc_ms],
    )?)
}

fn remote_fingerprint(remote_user_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(REMOTE_FINGERPRINT_DOMAIN);
    digest.update(remote_user_id.as_bytes());
    format!("{:x}", digest.finalize())
}

fn retry_delay_ms(attempt: i64, operation_id: &str) -> i64 {
    let exponent = u32::try_from(attempt.clamp(0, 20)).unwrap_or(20);
    let base = 5_000_i64
        .saturating_mul(2_i64.saturating_pow(exponent))
        .min(MAX_RETRY_DELAY_MS);
    let digest = Sha256::digest(operation_id.as_bytes());
    let jitter = i64::from(u16::from_be_bytes([digest[0], digest[1]]) % 1_001);
    base.saturating_add(jitter).min(MAX_RETRY_DELAY_MS)
}

fn validate_uuid(value: &str) -> Result<(), SyncStoreError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| SyncStoreError::InvalidIdentifier)
}
