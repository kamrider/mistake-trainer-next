use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::{
        assets::{decrypt_asset, plaintext_sha256},
        supabase::{CloudError, CloudPushTransport, ObjectUploadResult, RemoteObjectMetadata},
    },
    modules::sync_store::{
        LeasedPushBatch, PendingAssetTransfer, SyncStoreError, acknowledge_push_batch,
        fail_push_batch, lease_push_batch,
    },
};

const STANDARD_UPLOAD_LIMIT: usize = 6 * 1024 * 1024;
const TUS_CHUNK_BYTES: usize = 6 * 1024 * 1024;
const MAX_ASSET_BYTES: usize = 25 * 1024 * 1024;
const ENCRYPTED_ASSET_OVERHEAD_BUDGET: u64 = 64;
const TUS_SESSION_LIFETIME_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Error)]
pub enum SyncPushError {
    #[error("sync state operation failed")]
    Store(#[from] SyncStoreError),
    #[error("sync database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("cloud push failed")]
    Cloud(#[from] CloudError),
    #[error("local encrypted asset is missing or invalid")]
    InvalidLocalAsset,
    #[error("remote asset metadata does not match the local asset")]
    RemoteAssetMismatch,
    #[error("cloud acknowledgement did not match the leased operations")]
    InvalidAcknowledgement,
    #[error("sync payload serialization failed")]
    Serialization,
}

impl SyncPushError {
    fn stable_code(&self) -> &'static str {
        match self {
            Self::Store(_) => "sync_store_failed",
            Self::Database(_) => "sync_database_failed",
            Self::Cloud(CloudError::AuthenticationRejected) => "cloud_auth_rejected",
            Self::Cloud(CloudError::RateLimited) => "cloud_rate_limited",
            Self::Cloud(CloudError::ServiceUnavailable) => "cloud_unavailable",
            Self::Cloud(CloudError::Timeout) => "cloud_timeout",
            Self::Cloud(CloudError::Network) => "cloud_network",
            Self::Cloud(_) => "cloud_request_rejected",
            Self::InvalidLocalAsset => "local_asset_invalid",
            Self::RemoteAssetMismatch => "remote_asset_mismatch",
            Self::InvalidAcknowledgement => "cloud_ack_invalid",
            Self::Serialization => "sync_payload_invalid",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushReport {
    pub acknowledged_operation_ids: Vec<String>,
    pub uploaded_asset_ids: Vec<String>,
}

struct SensitiveBytes(Vec<u8>);

impl SensitiveBytes {
    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

pub async fn push_once<T: CloudPushTransport>(
    connection: &mut Connection,
    transport: &T,
    account_id: &str,
    remote_user_id: &str,
    access_token: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
    now_utc_ms: i64,
) -> Result<PushReport, SyncPushError> {
    let batch = lease_push_batch(connection, account_id, remote_user_id, now_utc_ms, 100)?;
    if batch.operations.is_empty() {
        return Ok(PushReport {
            acknowledged_operation_ids: Vec::new(),
            uploaded_asset_ids: Vec::new(),
        });
    }

    let result = push_leased_batch(
        connection,
        transport,
        access_token,
        blob_root,
        asset_key,
        now_utc_ms,
        &batch,
    )
    .await;
    match result {
        Ok(report) => Ok(report),
        Err(error) => {
            fail_push_batch(connection, &batch.lease_id, error.stable_code(), now_utc_ms)?;
            Err(error)
        }
    }
}

async fn push_leased_batch<T: CloudPushTransport>(
    connection: &mut Connection,
    transport: &T,
    access_token: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
    now_utc_ms: i64,
    batch: &LeasedPushBatch,
) -> Result<PushReport, SyncPushError> {
    let mut uploaded_asset_ids = Vec::new();
    for asset in &batch.assets {
        let uploaded = ensure_remote_asset(
            connection,
            transport,
            access_token,
            blob_root,
            asset_key,
            now_utc_ms,
            asset,
        )
        .await?;
        if uploaded {
            uploaded_asset_ids.push(asset.asset_id.clone());
        }
    }

    let operations =
        serde_json::to_value(&batch.operations).map_err(|_| SyncPushError::Serialization)?;
    let acknowledgements = transport.push_operations(access_token, &operations).await?;
    validate_acknowledgements(batch, &acknowledgements)?;
    let operation_ids = batch
        .operations
        .iter()
        .map(|operation| operation.operation_id.clone())
        .collect::<Vec<_>>();
    let deleted = acknowledge_push_batch(connection, &batch.lease_id, &operation_ids)?;
    if deleted != operation_ids.len() {
        return Err(SyncPushError::InvalidAcknowledgement);
    }
    Ok(PushReport {
        acknowledged_operation_ids: operation_ids,
        uploaded_asset_ids,
    })
}

async fn ensure_remote_asset<T: CloudPushTransport>(
    connection: &Connection,
    transport: &T,
    access_token: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
    now_utc_ms: i64,
    asset: &PendingAssetTransfer,
) -> Result<bool, SyncPushError> {
    validate_asset_metadata(asset)?;
    if let Some(metadata) = transport
        .object_metadata(access_token, &asset.storage_object)
        .await?
    {
        require_matching_metadata(asset, &metadata)?;
        return Ok(false);
    }

    let plaintext = read_verified_asset(blob_root, asset_key, asset)?;
    if plaintext.as_slice().len() <= STANDARD_UPLOAD_LIMIT {
        let result = transport
            .upload_small_object(
                access_token,
                &asset.storage_object,
                &asset.media_type,
                plaintext.as_slice(),
            )
            .await?;
        let metadata = transport
            .object_metadata(access_token, &asset.storage_object)
            .await?
            .ok_or_else(|| match result {
                ObjectUploadResult::AlreadyExists => SyncPushError::RemoteAssetMismatch,
                ObjectUploadResult::Created => SyncPushError::RemoteAssetMismatch,
            })?;
        require_matching_metadata(asset, &metadata)?;
        return Ok(matches!(result, ObjectUploadResult::Created));
    }

    upload_resumable(
        connection,
        transport,
        access_token,
        now_utc_ms,
        asset,
        plaintext.as_slice(),
    )
    .await?;
    let metadata = transport
        .object_metadata(access_token, &asset.storage_object)
        .await?
        .ok_or(SyncPushError::RemoteAssetMismatch)?;
    require_matching_metadata(asset, &metadata)?;
    Ok(true)
}

async fn upload_resumable<T: CloudPushTransport>(
    connection: &Connection,
    transport: &T,
    access_token: &str,
    now_utc_ms: i64,
    asset: &PendingAssetTransfer,
    plaintext: &[u8],
) -> Result<(), SyncPushError> {
    let mut restarted = false;
    loop {
        let session = load_transfer(connection, &asset.asset_id)?
            .filter(|transfer| transfer.expires_at_utc_ms > now_utc_ms);
        if session.is_none() {
            connection.execute(
                "DELETE FROM cloud_asset_transfers WHERE asset_id = ?1",
                [&asset.asset_id],
            )?;
        }
        let (upload_url, mut offset) = if let Some(session) = session {
            match transport
                .resumable_offset(access_token, &session.upload_url)
                .await?
            {
                Some(server_offset) => (session.upload_url, server_offset),
                None if !restarted => {
                    connection.execute(
                        "DELETE FROM cloud_asset_transfers WHERE asset_id = ?1",
                        [&asset.asset_id],
                    )?;
                    restarted = true;
                    continue;
                }
                None => return Err(SyncPushError::RemoteAssetMismatch),
            }
        } else {
            let upload_url = transport
                .create_resumable_upload(
                    access_token,
                    &asset.storage_object,
                    &asset.media_type,
                    asset.byte_length,
                )
                .await?;
            connection.execute(
                "INSERT INTO cloud_asset_transfers(
                     asset_id, upload_url, confirmed_offset, expires_at_utc_ms, updated_at_utc_ms
                 ) VALUES(?1, ?2, 0, ?3, ?4)
                 ON CONFLICT(asset_id) DO UPDATE SET upload_url = excluded.upload_url,
                     confirmed_offset = 0, expires_at_utc_ms = excluded.expires_at_utc_ms,
                     updated_at_utc_ms = excluded.updated_at_utc_ms",
                params![
                    asset.asset_id,
                    upload_url,
                    now_utc_ms.saturating_add(TUS_SESSION_LIFETIME_MS),
                    now_utc_ms
                ],
            )?;
            (upload_url, 0)
        };
        if offset < 0
            || usize::try_from(offset)
                .ok()
                .is_none_or(|value| value > plaintext.len())
        {
            return Err(SyncPushError::RemoteAssetMismatch);
        }
        while usize::try_from(offset).unwrap_or(usize::MAX) < plaintext.len() {
            let start = usize::try_from(offset).map_err(|_| SyncPushError::RemoteAssetMismatch)?;
            let end = start.saturating_add(TUS_CHUNK_BYTES).min(plaintext.len());
            let Some(next_offset) = transport
                .upload_resumable_chunk(access_token, &upload_url, offset, &plaintext[start..end])
                .await?
            else {
                if restarted {
                    return Err(SyncPushError::RemoteAssetMismatch);
                }
                connection.execute(
                    "DELETE FROM cloud_asset_transfers WHERE asset_id = ?1",
                    [&asset.asset_id],
                )?;
                restarted = true;
                break;
            };
            let expected_next_offset = offset
                .checked_add(
                    i64::try_from(end - start).map_err(|_| SyncPushError::RemoteAssetMismatch)?,
                )
                .ok_or(SyncPushError::RemoteAssetMismatch)?;
            if next_offset != expected_next_offset {
                return Err(SyncPushError::RemoteAssetMismatch);
            }
            offset = next_offset;
            connection.execute(
                "UPDATE cloud_asset_transfers
                 SET confirmed_offset = ?1, updated_at_utc_ms = ?2
                 WHERE asset_id = ?3 AND upload_url = ?4",
                params![offset, now_utc_ms, asset.asset_id, upload_url],
            )?;
        }
        if usize::try_from(offset).ok() == Some(plaintext.len()) {
            connection.execute(
                "DELETE FROM cloud_asset_transfers WHERE asset_id = ?1",
                [&asset.asset_id],
            )?;
            return Ok(());
        }
    }
}

#[derive(Debug)]
struct StoredTransfer {
    upload_url: String,
    expires_at_utc_ms: i64,
}

fn load_transfer(
    connection: &Connection,
    asset_id: &str,
) -> Result<Option<StoredTransfer>, SyncPushError> {
    Ok(connection
        .query_row(
            "SELECT upload_url, expires_at_utc_ms FROM cloud_asset_transfers WHERE asset_id = ?1",
            [asset_id],
            |row| {
                Ok(StoredTransfer {
                    upload_url: row.get(0)?,
                    expires_at_utc_ms: row.get(1)?,
                })
            },
        )
        .optional()?)
}

fn read_verified_asset(
    blob_root: &Path,
    asset_key: &[u8; 32],
    asset: &PendingAssetTransfer,
) -> Result<SensitiveBytes, SyncPushError> {
    let path = resolve_asset_path(blob_root, &asset.encrypted_path)?;
    let metadata = std::fs::metadata(&path).map_err(|_| SyncPushError::InvalidLocalAsset)?;
    let max_encrypted = (MAX_ASSET_BYTES as u64).saturating_add(ENCRYPTED_ASSET_OVERHEAD_BUDGET);
    if !metadata.is_file() || metadata.len() > max_encrypted {
        return Err(SyncPushError::InvalidLocalAsset);
    }
    let encrypted = std::fs::read(path).map_err(|_| SyncPushError::InvalidLocalAsset)?;
    let plaintext =
        decrypt_asset(&encrypted, asset_key).map_err(|_| SyncPushError::InvalidLocalAsset)?;
    if plaintext.len() > MAX_ASSET_BYTES
        || i64::try_from(plaintext.len()).ok() != Some(asset.byte_length)
        || plaintext_sha256(&plaintext) != asset.plaintext_sha256
    {
        return Err(SyncPushError::InvalidLocalAsset);
    }
    Ok(SensitiveBytes(plaintext))
}

fn resolve_asset_path(blob_root: &Path, encrypted_path: &str) -> Result<PathBuf, SyncPushError> {
    let relative = Path::new(encrypted_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(SyncPushError::InvalidLocalAsset);
    }
    let canonical_root =
        std::fs::canonicalize(blob_root).map_err(|_| SyncPushError::InvalidLocalAsset)?;
    let canonical_path = std::fs::canonicalize(blob_root.join(relative))
        .map_err(|_| SyncPushError::InvalidLocalAsset)?;
    if !canonical_path.starts_with(canonical_root) {
        return Err(SyncPushError::InvalidLocalAsset);
    }
    Ok(canonical_path)
}

fn validate_asset_metadata(asset: &PendingAssetTransfer) -> Result<(), SyncPushError> {
    if Uuid::parse_str(&asset.asset_id).is_err()
        || asset.plaintext_sha256.len() != 64
        || !asset
            .plaintext_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || asset.byte_length <= 0
        || usize::try_from(asset.byte_length)
            .ok()
            .is_none_or(|length| length > MAX_ASSET_BYTES)
        || !matches!(
            asset.media_type.as_str(),
            "image/jpeg" | "image/png" | "image/webp"
        )
    {
        return Err(SyncPushError::InvalidLocalAsset);
    }
    Ok(())
}

fn require_matching_metadata(
    asset: &PendingAssetTransfer,
    metadata: &RemoteObjectMetadata,
) -> Result<(), SyncPushError> {
    if metadata.byte_length != asset.byte_length || metadata.media_type != asset.media_type {
        Err(SyncPushError::RemoteAssetMismatch)
    } else {
        Ok(())
    }
}

fn validate_acknowledgements(
    batch: &LeasedPushBatch,
    acknowledgements: &[crate::infrastructure::supabase::PushAcknowledgement],
) -> Result<(), SyncPushError> {
    if acknowledgements.len() != batch.operations.len() {
        return Err(SyncPushError::InvalidAcknowledgement);
    }
    let expected = batch
        .operations
        .iter()
        .map(|operation| {
            (
                operation.operation_id.as_str(),
                (operation.entity_type.as_str(), operation.entity_id.as_str()),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    for acknowledgement in acknowledgements {
        if acknowledgement.change_seq < 1
            || !seen.insert(acknowledgement.operation_id.as_str())
            || expected.get(acknowledgement.operation_id.as_str()).copied()
                != Some((
                    acknowledgement.entity_type.as_str(),
                    acknowledgement.entity_id.as_str(),
                ))
        {
            return Err(SyncPushError::InvalidAcknowledgement);
        }
    }
    Ok(())
}
