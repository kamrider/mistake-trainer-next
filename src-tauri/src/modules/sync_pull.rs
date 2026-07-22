use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        supabase::{CloudError, CloudPullTransport, DownloadedRemoteAsset, RemotePullChange},
    },
    modules::{
        review::{ReviewUseCaseError, rebuild_schedule_for_problem},
        sync_store::{
            SyncStoreError, WireAsset, WireExportSnapshot, WireProblemAggregate, WireProfile,
            WireReviewEvent, WireTombstone, pull_cursor, record_pull_success_tx,
        },
    },
};

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
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullReport {
    pub applied_count: u32,
    pub downloaded_asset_count: u32,
    pub final_cursor: i64,
}

#[derive(Clone, Debug)]
enum DecodedChange {
    Profile(WireProfile),
    Asset(WireAsset),
    Problem(WireProblemAggregate),
    Review(WireReviewEvent),
    Export(WireExportSnapshot),
    Tombstone(WireTombstone),
}

#[derive(Clone, Debug)]
struct StagedAsset {
    asset: WireAsset,
    relative_path: String,
    staged_path: PathBuf,
    final_path: PathBuf,
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
                staged_assets.push(
                    stage_remote_asset(
                        transport,
                        access_token,
                        blob_root,
                        asset_key,
                        asset,
                        &format!("{page_cursor}-{}", report.applied_count),
                    )
                    .await?,
                );
            }
        }
        match apply_page(
            connection,
            account_id,
            &decoded,
            &staged_assets,
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
                cleanup_staged(&staged_assets, false);
            }
            Err(error) => {
                cleanup_staged(&staged_assets, true);
                return Err(error);
            }
        }
        if page.len() < PAGE_SIZE {
            return Ok(report);
        }
    }
}

fn validate_page(
    page: &[RemotePullChange],
    after: i64,
    remote_user_id: &str,
) -> Result<(), SyncPullError> {
    if page.len() > PAGE_SIZE {
        return Err(SyncPullError::InvalidChange);
    }
    let mut previous = after;
    for change in page {
        if change.change_seq <= previous
            || change.change_seq < 1
            || change.entity_id.is_empty()
            || change.entity_id.len() > 80
            || !matches!(change.operation.as_str(), "upsert" | "append" | "delete")
        {
            return Err(SyncPullError::InvalidChange);
        }
        previous = change.change_seq;
        let object = change
            .payload
            .as_object()
            .ok_or(SyncPullError::InvalidChange)?;
        let account = object
            .get("accountId")
            .or_else(|| object.get("account_id"))
            .and_then(Value::as_str)
            .ok_or(SyncPullError::InvalidChange)?;
        if account != remote_user_id {
            return Err(SyncPullError::InvalidChange);
        }
    }
    Ok(())
}

fn decode_page(
    page: &[RemotePullChange],
    remote_user_id: &str,
) -> Result<Vec<DecodedChange>, SyncPullError> {
    let mut decoded = Vec::with_capacity(page.len());
    for change in page {
        let payload = without_account(&change.payload, remote_user_id)?;
        let value = match change.entity_type.as_str() {
            "learner_profile" if change.operation == "upsert" => {
                DecodedChange::Profile(from_value(payload)?)
            }
            "asset" if change.operation == "upsert" => {
                let asset: WireAsset = from_value(payload)?;
                validate_remote_asset(&asset, remote_user_id)?;
                DecodedChange::Asset(asset)
            }
            "problem" if change.operation == "upsert" => {
                DecodedChange::Problem(from_value(payload)?)
            }
            "review_event" if matches!(change.operation.as_str(), "upsert" | "append") => {
                DecodedChange::Review(from_value(payload)?)
            }
            "export_snapshot" if change.operation == "upsert" => {
                DecodedChange::Export(from_value(payload)?)
            }
            "problem" | "learner_profile" | "asset" | "review_event" | "export_snapshot"
                if change.operation == "delete" =>
            {
                DecodedChange::Tombstone(from_value(payload)?)
            }
            _ => return Err(SyncPullError::InvalidChange),
        };
        decoded.push(value);
    }
    Ok(decoded)
}

fn without_account(value: &Value, remote_user_id: &str) -> Result<Value, SyncPullError> {
    let object = value.as_object().ok_or(SyncPullError::InvalidChange)?;
    let account = object
        .get("accountId")
        .or_else(|| object.get("account_id"))
        .and_then(Value::as_str)
        .ok_or(SyncPullError::InvalidChange)?;
    if account != remote_user_id {
        return Err(SyncPullError::InvalidChange);
    }
    let mut clean = object.clone();
    clean.remove("accountId");
    clean.remove("account_id");
    Ok(Value::Object(clean))
}

fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, SyncPullError> {
    serde_json::from_value(value).map_err(|_| SyncPullError::InvalidChange)
}

fn validate_remote_asset(asset: &WireAsset, remote_user_id: &str) -> Result<(), SyncPullError> {
    validate_uuid(&asset.id)?;
    if asset.plaintext_sha256.len() != 64
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
        || asset.storage_object != format!("{remote_user_id}/{}", asset.plaintext_sha256)
    {
        return Err(SyncPullError::InvalidAsset);
    }
    Ok(())
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
    let shard = &asset.id[..2];
    let relative_path = format!("blobs/{shard}/{}.mtb", asset.id);
    let final_path = blob_root.join(&relative_path);
    let staged_root = blob_root.join(".sync-pull").join(page_id);
    fs::create_dir_all(&staged_root)?;
    let staged_path = staged_root.join(format!("{}.mtb", asset.id));
    fs::write(&staged_path, encrypted)?;
    Ok(StagedAsset {
        asset: asset.clone(),
        relative_path,
        staged_path,
        final_path,
    })
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

fn apply_page(
    connection: &mut Connection,
    account_id: &str,
    changes: &[DecodedChange],
    staged_assets: &[StagedAsset],
    page_cursor: i64,
    now_utc_ms: i64,
) -> Result<usize, SyncPullError> {
    let transaction = connection.transaction()?;
    let mut asset_ids = HashMap::<String, String>::new();
    let staged_by_id = staged_assets
        .iter()
        .map(|item| (item.asset.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    for change in changes {
        if let DecodedChange::Asset(asset) = change {
            let local_id = upsert_asset(
                &transaction,
                account_id,
                asset,
                staged_by_id.get(asset.id.as_str()).copied(),
            )?;
            asset_ids.insert(asset.id.clone(), local_id);
        }
    }
    let mut affected_problems = HashSet::new();
    for change in changes {
        match change {
            DecodedChange::Profile(profile) => upsert_profile(&transaction, account_id, profile)?,
            DecodedChange::Asset(_) => {}
            DecodedChange::Problem(problem) => {
                let local_id = upsert_problem(&transaction, account_id, problem, &asset_ids)?;
                affected_problems.insert(local_id);
            }
            DecodedChange::Review(event) => {
                insert_review_event(&transaction, account_id, event)?;
                affected_problems.insert(event.problem_id.clone());
            }
            DecodedChange::Export(snapshot) => upsert_export(&transaction, account_id, snapshot)?,
            DecodedChange::Tombstone(tombstone) => {
                apply_tombstone(&transaction, account_id, tombstone)?;
                if tombstone.entity_type == "problem" {
                    affected_problems.insert(tombstone.entity_id.clone());
                }
            }
        }
    }
    for problem_id in &affected_problems {
        rebuild_schedule_for_problem(&transaction, account_id, problem_id, now_utc_ms)?;
    }
    record_pull_success_tx(&transaction, account_id, page_cursor, now_utc_ms)?;
    transaction.commit()?;
    Ok(changes.len())
}

fn upsert_profile(
    tx: &Transaction<'_>,
    account_id: &str,
    profile: &WireProfile,
) -> Result<(), SyncPullError> {
    validate_uuid(&profile.id)?;
    tx.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at_utc_ms = excluded.updated_at_utc_ms, revision = excluded.revision
         WHERE learner_profiles.account_id = excluded.account_id AND excluded.revision > learner_profiles.revision",
        params![profile.id, account_id, profile.name, profile.created_at_utc_ms, profile.updated_at_utc_ms, profile.revision],
    )?;
    Ok(())
}

fn upsert_asset(
    tx: &Transaction<'_>,
    account_id: &str,
    asset: &WireAsset,
    staged: Option<&StagedAsset>,
) -> Result<String, SyncPullError> {
    let existing = tx.query_row(
        "SELECT id, plaintext_sha256, byte_length, media_type FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
        params![account_id, asset.plaintext_sha256],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?)),
    ).optional()?;
    if let Some((id, hash, length, media_type)) = existing {
        if id != asset.id
            || hash != asset.plaintext_sha256
            || length != asset.byte_length
            || media_type != asset.media_type
        {
            return Err(SyncPullError::AssetMismatch);
        }
        return Ok(id);
    }
    let staged = staged.ok_or(SyncPullError::InvalidAsset)?;
    if let Some(parent) = staged.final_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if staged.final_path.exists() {
        return Err(SyncPullError::AssetMismatch);
    }
    fs::rename(&staged.staged_path, &staged.final_path)?;
    tx.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![asset.id, account_id, asset.plaintext_sha256, staged.relative_path, asset.byte_length, asset.media_type, asset.created_at_utc_ms],
    )?;
    Ok(asset.id.clone())
}

fn upsert_problem(
    tx: &Transaction<'_>,
    account_id: &str,
    problem: &WireProblemAggregate,
    asset_ids: &HashMap<String, String>,
) -> Result<String, SyncPullError> {
    validate_uuid(&problem.id)?;
    validate_uuid(&problem.profile_id)?;
    let profile_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![problem.profile_id, account_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(SyncPullError::InvalidChange);
    }
    let changed = tx.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, tags_json, note, status, time_limit_seconds, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET subject=excluded.subject, tags_json=excluded.tags_json, note=excluded.note, status=excluded.status, time_limit_seconds=excluded.time_limit_seconds, updated_at_utc_ms=excluded.updated_at_utc_ms, revision=excluded.revision
         WHERE problems.account_id = excluded.account_id AND excluded.revision > problems.revision",
        params![problem.id, account_id, problem.profile_id, problem.subject, serde_json::to_string(&problem.tags).map_err(|_| SyncPullError::InvalidChange)?, problem.note, problem.status, problem.time_limit_seconds, problem.created_at_utc_ms, problem.updated_at_utc_ms, problem.revision],
    )?;
    if changed == 0 {
        return Ok(problem.id.clone());
    }
    tx.execute(
        "DELETE FROM problem_assets WHERE problem_id = ?1",
        [&problem.id],
    )?;
    for link in &problem.assets {
        validate_uuid(&link.asset_id)?;
        let asset_id = asset_ids
            .get(&link.asset_id)
            .cloned()
            .unwrap_or_else(|| link.asset_id.clone());
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1 AND account_id = ?2)",
            params![asset_id, account_id],
            |row| row.get(0),
        )?;
        if !exists || !matches!(link.role.as_str(), "question" | "answer") || link.position < 0 {
            return Err(SyncPullError::InvalidChange);
        }
        tx.execute("INSERT INTO problem_assets(problem_id, asset_id, role, position) VALUES(?1, ?2, ?3, ?4)", params![problem.id, asset_id, link.role, link.position])?;
    }
    Ok(problem.id.clone())
}

fn insert_review_event(
    tx: &Transaction<'_>,
    account_id: &str,
    event: &WireReviewEvent,
) -> Result<(), SyncPullError> {
    for id in [
        &event.id,
        &event.profile_id,
        &event.problem_id,
        &event.device_id,
    ] {
        validate_uuid(id)?;
    }
    if !matches!(event.rating.as_str(), "again" | "hard" | "good" | "easy") || event.duration_ms < 0
    {
        return Err(SyncPullError::InvalidChange);
    }
    tx.execute(
        "INSERT OR IGNORE INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![event.id, account_id, event.profile_id, event.problem_id, event.device_id, event.rating, event.duration_ms, event.occurred_at_utc_ms, event.algorithm_version, event.parameter_version],
    )?;
    Ok(())
}

fn upsert_export(
    tx: &Transaction<'_>,
    account_id: &str,
    snapshot: &WireExportSnapshot,
) -> Result<(), SyncPullError> {
    validate_uuid(&snapshot.id)?;
    validate_uuid(&snapshot.profile_id)?;
    tx.execute(
        "INSERT INTO export_snapshots(id, account_id, profile_id, title, problem_ids_json, configuration_json, created_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title, problem_ids_json=excluded.problem_ids_json, configuration_json=excluded.configuration_json, revision=excluded.revision
         WHERE export_snapshots.account_id = excluded.account_id AND excluded.revision > export_snapshots.revision",
        params![snapshot.id, account_id, snapshot.profile_id, snapshot.title, serde_json::to_string(&snapshot.problem_ids).map_err(|_| SyncPullError::InvalidChange)?, snapshot.configuration.to_string(), snapshot.created_at_utc_ms, snapshot.revision],
    )?;
    Ok(())
}

fn apply_tombstone(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
) -> Result<(), SyncPullError> {
    validate_uuid(&tombstone.tombstone_id)?;
    validate_uuid(&tombstone.entity_id)?;
    if let Some(profile_id) = &tombstone.profile_id {
        validate_uuid(profile_id)?;
    }
    if tombstone.purge_after_utc_ms <= tombstone.deleted_at_utc_ms {
        return Err(SyncPullError::InvalidChange);
    }
    tx.execute(
        "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET deleted_at_utc_ms=excluded.deleted_at_utc_ms, purge_after_utc_ms=excluded.purge_after_utc_ms, revision=excluded.revision
         WHERE tombstones.account_id = excluded.account_id AND excluded.revision > tombstones.revision",
        params![tombstone.tombstone_id, account_id, tombstone.profile_id, tombstone.entity_type, tombstone.entity_id, tombstone.deleted_at_utc_ms, tombstone.purge_after_utc_ms, tombstone.deleted_revision],
    )?;
    if tombstone.entity_type == "problem" {
        tx.execute(
            "UPDATE problems SET status = 'trashed', updated_at_utc_ms = ?1, revision = max(revision, ?2) WHERE id = ?3 AND account_id = ?4",
            params![tombstone.deleted_at_utc_ms, tombstone.deleted_revision, tombstone.entity_id, account_id],
        )?;
    }
    Ok(())
}

fn cleanup_staged(staged: &[StagedAsset], remove_final: bool) {
    let mut roots = HashSet::new();
    for asset in staged {
        let _ = fs::remove_file(&asset.staged_path);
        if let Some(root) = asset.staged_path.parent() {
            roots.insert(root.to_owned());
        }
        if remove_final && asset.final_path.exists() {
            let _ = fs::remove_file(&asset.final_path);
        }
    }
    for root in roots {
        let _ = fs::remove_dir_all(root);
    }
}

fn validate_uuid(value: &str) -> Result<(), SyncPullError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| SyncPullError::InvalidChange)
}
