use std::path::Path;

use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    application::ports::sync::{CloudError, CloudPullTransport, DownloadedRemoteAsset},
    infrastructure::assets::{encrypt_asset, plaintext_sha256},
    modules::{
        review::ReviewUseCaseError,
        sync_conflicts::SyncConflictError,
        sync_store::{SyncStoreError, WireAsset, pull_cursor},
    },
};

#[path = "sync_pull_asset_staging.rs"]
mod asset_staging;
#[path = "sync_pull_decoder.rs"]
mod sync_pull_decoder;
#[path = "sync_pull_transaction.rs"]
mod sync_pull_transaction;

use asset_staging::{StagedAsset, cleanup_page, stage_encrypted_asset};
use sync_pull_decoder::{DecodedChange, decode_page, validate_page};
use sync_pull_transaction::apply_page;

const PAGE_SIZE: usize = 500;
const MAX_ASSET_BYTES: usize = 25 * 1024 * 1024;
const MAX_EDGE: u32 = 12_000;
const MAX_PIXELS: u64 = 80_000_000;

#[derive(Debug, Error)]
pub enum SyncPullError {
    #[error("sync state operation failed")]
    Store(#[from] SyncStoreError),
    #[error("sync database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("cloud pull failed")]
    Cloud(#[from] CloudError),
    #[error("remote change payload is invalid")]
    InvalidChange,
    #[error("remote asset is invalid")]
    InvalidAsset,
    #[error("remote asset download failed validation")]
    AssetMismatch,
    #[error("local blob staging failed")]
    Blob(#[from] std::io::Error),
    #[error("local asset encryption failed")]
    Encryption,
    #[error("review schedule rebuild failed")]
    Review(#[from] ReviewUseCaseError),
    #[error("sync conflict merge failed")]
    Conflict(#[from] SyncConflictError),
}

impl SyncPullError {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::Store(_) => "sync_store_failed",
            Self::Database(_) => "sync_database_failed",
            Self::Cloud(CloudError::AuthenticationRejected) => "cloud_auth_rejected",
            Self::Cloud(CloudError::RateLimited) => "cloud_rate_limited",
            Self::Cloud(CloudError::ServiceUnavailable) => "cloud_unavailable",
            Self::Cloud(CloudError::Timeout) => "cloud_timeout",
            Self::Cloud(CloudError::Network) => "cloud_network",
            Self::Cloud(_) => "cloud_request_rejected",
            Self::InvalidChange => "cloud_change_invalid",
            Self::InvalidAsset => "cloud_asset_invalid",
            Self::AssetMismatch => "cloud_asset_mismatch",
            Self::Blob(_) => "sync_blob_failed",
            Self::Encryption => "sync_encryption_failed",
            Self::Review(_) => "sync_schedule_rebuild_failed",
            Self::Conflict(_) => "sync_conflict_merge_failed",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullReport {
    pub applied_count: u32,
    pub downloaded_asset_count: u32,
    pub final_cursor: i64,
}

pub async fn pull_until_current<T: CloudPullTransport>(
    connection: &mut Connection,
    transport: &T,
    account_id: &str,
    remote_user_id: &str,
    access_token: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
    now_utc_ms: i64,
) -> Result<PullReport, SyncPullError> {
    validate_uuid(account_id)?;
    validate_uuid(remote_user_id)?;
    let mut cursor = pull_cursor(connection, account_id)?;
    let mut report = PullReport {
        final_cursor: cursor,
        ..PullReport::default()
    };
    loop {
        let page = transport
            .pull_changes(access_token, cursor, PAGE_SIZE)
            .await?;
        if page.is_empty() {
            return Ok(report);
        }
        validate_page(&page, cursor, remote_user_id)?;
        let page_cursor = page
            .last()
            .map(|change| change.change_seq)
            .ok_or(SyncPullError::InvalidChange)?;
        let decoded = decode_page(&page, remote_user_id)?;
        let mut staged_assets = Vec::new();
        for change in &decoded {
            if let DecodedChange::Asset(asset) = change {
                if local_asset_matches(connection, account_id, asset)? {
                    continue;
                }
                match stage_remote_asset(
                    transport,
                    access_token,
                    blob_root,
                    asset_key,
                    asset,
                    &format!("{page_cursor}-{}", report.applied_count),
                )
                .await
                {
                    Ok(staged) => staged_assets.push(staged),
                    Err(error) => {
                        cleanup_page(&staged_assets, true);
                        return Err(error);
                    }
                }
            }
        }
        match apply_page(
            connection,
            account_id,
            &decoded,
            &mut staged_assets,
            blob_root,
            page_cursor,
            now_utc_ms,
        ) {
            Ok(applied) => {
                report.applied_count = report
                    .applied_count
                    .saturating_add(u32::try_from(applied).unwrap_or(u32::MAX));
                report.downloaded_asset_count = report
                    .downloaded_asset_count
                    .saturating_add(u32::try_from(staged_assets.len()).unwrap_or(u32::MAX));
                report.final_cursor = page_cursor;
                cursor = page_cursor;
                cleanup_page(&staged_assets, false);
            }
            Err(error) => {
                cleanup_page(&staged_assets, true);
                return Err(error);
            }
        }
        if page.len() < PAGE_SIZE {
            return Ok(report);
        }
    }
}

async fn stage_remote_asset<T: CloudPullTransport>(
    transport: &T,
    access_token: &str,
    blob_root: &Path,
    asset_key: &[u8; 32],
    asset: &WireAsset,
    page_id: &str,
) -> Result<StagedAsset, SyncPullError> {
    let downloaded = transport
        .download_object(access_token, &asset.storage_object)
        .await?;
    validate_download(asset, &downloaded)?;
    let encrypted =
        encrypt_asset(&downloaded.bytes, asset_key).map_err(|_| SyncPullError::Encryption)?;
    stage_encrypted_asset(blob_root, asset, page_id, &encrypted)
}

fn validate_download(
    asset: &WireAsset,
    downloaded: &DownloadedRemoteAsset,
) -> Result<(), SyncPullError> {
    if downloaded.media_type != asset.media_type
        || downloaded.bytes.len() != usize::try_from(asset.byte_length).unwrap_or(0)
        || plaintext_sha256(&downloaded.bytes) != asset.plaintext_sha256
    {
        return Err(SyncPullError::AssetMismatch);
    }
    let format = match downloaded.media_type.as_str() {
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/png" => image::ImageFormat::Png,
        "image/webp" => image::ImageFormat::WebP,
        _ => return Err(SyncPullError::InvalidAsset),
    };
    let image = image::load_from_memory_with_format(&downloaded.bytes, format)
        .map_err(|_| SyncPullError::AssetMismatch)?;
    let (width, height) = image.dimensions();
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width).saturating_mul(u64::from(height)) > MAX_PIXELS
    {
        return Err(SyncPullError::InvalidAsset);
    }
    Ok(())
}

fn local_asset_matches(
    connection: &Connection,
    account_id: &str,
    asset: &WireAsset,
) -> Result<bool, SyncPullError> {
    let row = connection.query_row(
        "SELECT id, plaintext_sha256, byte_length, media_type FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
        params![account_id, asset.plaintext_sha256],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?)),
    ).optional()?;
    Ok(row.is_some_and(|row| {
        row.0 == asset.id
            && row.1 == asset.plaintext_sha256
            && row.2 == asset.byte_length
            && row.3 == asset.media_type
    }))
}

fn validate_uuid(value: &str) -> Result<(), SyncPullError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| SyncPullError::InvalidChange)
}
