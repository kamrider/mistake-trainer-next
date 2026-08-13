use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    application::ports::assets::AssetEncryptor,
    domain::assets::plaintext_sha256,
    modules::{
        capture::{CaptureImageError, inspect_capture_image},
        capture_asset_repository,
    },
};

#[path = "capture_commit.rs"]
mod capture_commit;
#[path = "capture_crop.rs"]
mod capture_crop;
#[path = "capture_inbox_repository.rs"]
mod capture_inbox_repository;
#[path = "capture_inbox_transaction_support.rs"]
mod capture_inbox_transaction_support;
#[path = "capture_organizer_transaction.rs"]
mod capture_organizer_transaction;

use capture_asset_repository::{StagedCaptureAsset, stage_encrypted_capture_asset};
pub(crate) use capture_asset_repository::{
    image_format_for_media_type, read_encrypted_blob, remove_encrypted_blob,
    validate_relative_asset_path,
};
pub use capture_commit::{CaptureCommitReport, commit_ready_capture_drafts};
pub use capture_crop::{
    ApplyCaptureCrop, CaptureCropApplyReport, CaptureCropRecipe, CaptureItemPreview,
    NormalizedCropRect, NormalizedPoint, PerspectiveQuad, RevertCaptureCrop, apply_capture_crop,
    get_capture_crop_source_preview, get_capture_item_preview, revert_capture_crop,
};
pub(crate) use capture_crop::{EncodedCrop, encode_crop};
use capture_inbox_repository::get_capture_item;
pub(crate) use capture_inbox_repository::query_batch;
pub use capture_inbox_repository::{get_capture_batch_detail, list_capture_batches};
pub use capture_organizer_transaction::{
    apply_capture_layout, apply_capture_pair_suggestions, delete_capture_draft,
    discard_capture_batch, merge_capture_card, move_capture_item, remove_capture_item,
    stage_capture_item_role, update_capture_draft,
};

pub const MAX_CAPTURE_BATCH_ITEMS: i64 = 150;
pub const MAX_CAPTURE_BATCH_BYTES: i64 = 1024 * 1024 * 1024;
pub(super) const MAX_ENCRYPTED_CAPTURE_BYTES: u64 = 25 * 1024 * 1024 + 64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureBatchState {
    Collecting,
    Organizing,
    Completed,
}

impl CaptureBatchState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Collecting => "collecting",
            Self::Organizing => "organizing",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLayoutMode {
    Alternating,
    Split,
    QuestionsOnly,
    Manual,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatchSummary {
    pub id: String,
    pub subject: String,
    pub state: CaptureBatchState,
    pub item_count: u32,
    pub draft_count: u32,
    pub ready_count: u32,
    pub updated_at_utc_ms: f64,
    pub revision: u32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItemSummary {
    pub id: String,
    pub source_name: String,
    pub source_sequence: u32,
    pub media_type: String,
    pub byte_length: f64,
    pub width: u32,
    pub height: u32,
    pub staged_role: String,
    pub draft_id: Option<String>,
    pub role: Option<String>,
    pub position: Option<u32>,
    pub crop_derivation_id: Option<String>,
    pub crop_source_item_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDraftSummary {
    pub id: String,
    pub position: u32,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
    pub question_item_ids: Vec<String>,
    pub answer_item_ids: Vec<String>,
    pub ready: bool,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CapturePairSuggestionSummary {
    pub id: String,
    pub question_item_ids: Vec<String>,
    pub answer_item_ids: Vec<String>,
    pub confidence_basis_points: u16,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBatchDetail {
    pub batch: CaptureBatchSummary,
    pub items: Vec<CaptureItemSummary>,
    pub drafts: Vec<CaptureDraftSummary>,
    pub unassigned_item_ids: Vec<String>,
    pub pair_suggestions: Vec<CapturePairSuggestionSummary>,
}

#[derive(Clone, Debug)]
pub struct CreateCaptureBatch {
    pub account_id: String,
    pub profile_id: String,
    pub subject: String,
    pub state: CaptureBatchState,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct IngestCaptureItem {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub client_upload_id: String,
    pub source_name: String,
    pub source_sequence: Option<i64>,
    pub bytes: Vec<u8>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ApplyCaptureLayout {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub mode: CaptureLayoutMode,
    pub question_images_per_draft: u32,
    pub answer_images_per_draft: u32,
    pub split_index: Option<u32>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct MoveCaptureItem {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub target_draft_id: Option<String>,
    pub target_role: Option<String>,
    pub target_position: u32,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct StageCaptureItemRole {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub staged_role: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct MergeCaptureCard {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub target_draft_id: Option<String>,
    pub item_ids: Vec<String>,
    pub new_draft_subject: Option<String>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ApplyCapturePairSuggestions {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub pair_ids: Vec<String>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UpdateCaptureDraft {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub draft_id: String,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
    pub now_utc_ms: i64,
}

#[derive(Debug, Error)]
pub enum CaptureInboxError {
    #[error("capture batch was not found")]
    BatchNotFound,
    #[error("capture draft was not found")]
    DraftNotFound,
    #[error("capture item was not found")]
    ItemNotFound,
    #[error("capture batch changed")]
    RevisionConflict,
    #[error("capture batch is not editable")]
    InvalidState,
    #[error("capture input is invalid")]
    InvalidInput,
    #[error("capture batch capacity was reached")]
    CapacityReached,
    #[error("capture image is invalid")]
    InvalidImage,
    #[error("capture crop is invalid")]
    InvalidCrop,
    #[error("capture crop can no longer be reverted")]
    CropNotRevertible,
    #[error("capture asset path is invalid")]
    InvalidAssetPath,
    #[error("capture filesystem error")]
    Io(#[from] std::io::Error),
    #[error("capture database error")]
    Database(#[from] rusqlite::Error),
    #[error("capture serialization error")]
    Serialization(#[from] serde_json::Error),
    #[error("capture asset encryption failed")]
    Crypto,
}

impl From<CaptureImageError> for CaptureInboxError {
    fn from(_: CaptureImageError) -> Self {
        Self::InvalidImage
    }
}

pub fn create_capture_batch(
    connection: &mut Connection,
    input: CreateCaptureBatch,
) -> Result<CaptureBatchSummary, CaptureInboxError> {
    let subject = normalize_subject(&input.subject)?;
    let profile_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![input.profile_id, input.account_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(CaptureInboxError::BatchNotFound);
    }
    let id = Uuid::now_v7().to_string();
    connection.execute(
        "INSERT INTO capture_batches(id, account_id, profile_id, subject, state, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)",
        params![id, input.account_id, input.profile_id, subject, input.state.as_str(), input.now_utc_ms],
    )?;
    Ok(CaptureBatchSummary {
        id,
        subject,
        state: input.state,
        item_count: 0,
        draft_count: 0,
        ready_count: 0,
        updated_at_utc_ms: input.now_utc_ms as f64,
        revision: 1,
    })
}

fn normalize_subject(value: &str) -> Result<String, CaptureInboxError> {
    let value = value.trim();
    if value.chars().count() > 40 {
        return Err(CaptureInboxError::InvalidInput);
    }
    Ok(value.to_owned())
}

fn sanitize_source_name(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("image")
        .chars()
        .take(255)
        .collect()
}

pub fn ingest_capture_item(
    connection: &mut Connection,
    blob_root: &Path,
    asset_encryptor: &dyn AssetEncryptor,
    input: IngestCaptureItem,
) -> Result<CaptureItemSummary, CaptureInboxError> {
    if input.client_upload_id.trim().is_empty() || input.client_upload_id.chars().count() > 100 {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    if batch.state == CaptureBatchState::Completed {
        return Err(CaptureInboxError::InvalidState);
    }
    if let Some(existing_id) = connection
        .query_row(
            "SELECT id FROM capture_items WHERE batch_id = ?1 AND client_upload_id = ?2",
            params![input.batch_id, input.client_upload_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return get_capture_item(connection, &input.account_id, &input.batch_id, &existing_id);
    }
    let metadata = inspect_capture_image(&input.bytes)?;
    let (item_count, logical_bytes): (i64, i64) = connection.query_row(
        "SELECT COUNT(*), COALESCE(SUM(a.byte_length), 0)
         FROM capture_items i JOIN assets a ON a.id = i.asset_id WHERE i.batch_id = ?1",
        [input.batch_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let byte_length = i64::try_from(input.bytes.len()).unwrap_or(i64::MAX);
    if item_count >= MAX_CAPTURE_BATCH_ITEMS
        || logical_bytes.saturating_add(byte_length) > MAX_CAPTURE_BATCH_BYTES
    {
        return Err(CaptureInboxError::CapacityReached);
    }
    let source_sequence = match input.source_sequence {
        Some(sequence) if sequence >= 0 => sequence,
        Some(_) => return Err(CaptureInboxError::InvalidInput),
        None => connection.query_row(
            "SELECT COALESCE(MAX(source_sequence), -1) + 1 FROM capture_items WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?,
    };
    let hash = plaintext_sha256(&input.bytes);
    let known_asset = connection
        .query_row(
            "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
            params![input.account_id, hash],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let mut staged_asset: Option<StagedCaptureAsset> = None;
    let asset_id = if let Some(asset_id) = known_asset {
        asset_id
    } else {
        let asset_id = Uuid::now_v7().to_string();
        let encrypted = asset_encryptor
            .encrypt(&input.bytes)
            .map_err(|_| CaptureInboxError::Crypto)?;
        staged_asset = Some(stage_encrypted_capture_asset(
            blob_root,
            asset_id.clone(),
            &encrypted,
        )?);
        asset_id
    };
    let item_id = Uuid::now_v7().to_string();
    let source_name = sanitize_source_name(&input.source_name);
    let transaction = connection.transaction()?;
    let persist_result = (|| -> Result<(), CaptureInboxError> {
        if let Some(staged) = &mut staged_asset {
            transaction.execute(
                "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![staged.asset_id(), input.account_id, hash, staged.relative_path(), byte_length, metadata.media_type, input.now_utc_ms],
            )?;
            staged.promote()?;
        }
        transaction.execute(
            "INSERT INTO capture_items(id, batch_id, asset_id, client_upload_id, source_name, source_sequence, width, height, created_at_utc_ms)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![item_id, input.batch_id, asset_id, input.client_upload_id, source_name, source_sequence, metadata.width, metadata.height, input.now_utc_ms],
        )?;
        transaction.execute(
            "UPDATE capture_batches SET updated_at_utc_ms = ?2, revision = revision + 1 WHERE id = ?1",
            params![input.batch_id, input.now_utc_ms],
        )?;
        transaction.commit()?;
        if let Some(staged) = &mut staged_asset {
            staged.mark_committed();
        }
        Ok(())
    })();
    persist_result?;
    get_capture_item(connection, &input.account_id, &input.batch_id, &item_id)
}

pub fn update_capture_batch(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    subject: &str,
    finish_collecting: bool,
    now_utc_ms: i64,
) -> Result<CaptureBatchSummary, CaptureInboxError> {
    let subject = normalize_subject(subject)?;
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    if batch.state == CaptureBatchState::Completed {
        return Err(CaptureInboxError::InvalidState);
    }
    let next_state = if finish_collecting {
        CaptureBatchState::Organizing
    } else {
        batch.state
    };
    connection.execute(
        "UPDATE capture_batches SET subject = ?2, state = ?3, updated_at_utc_ms = ?4, revision = revision + 1
         WHERE id = ?1 AND account_id = ?5 AND profile_id = ?6",
        params![batch_id, subject, next_state.as_str(), now_utc_ms, account_id, profile_id],
    )?;
    query_batch(connection, account_id, profile_id, batch_id)
}

pub fn assign_capture_batch_subject(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    subject: &str,
    now_utc_ms: i64,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let subject = normalize_subject(subject)?;
    if subject.is_empty() {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    ensure_organizing_revision(&batch, expected_revision)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE capture_batches SET subject = ?2, updated_at_utc_ms = ?3, revision = revision + 1
         WHERE id = ?1 AND account_id = ?4 AND profile_id = ?5",
        params![batch_id, subject, now_utc_ms, account_id, profile_id],
    )?;
    transaction.execute(
        "UPDATE capture_drafts SET subject_override = NULL, updated_at_utc_ms = ?2
         WHERE batch_id = ?1",
        params![batch_id, now_utc_ms],
    )?;
    transaction.commit()?;
    get_capture_batch_detail(connection, account_id, profile_id, batch_id)
}

pub(super) fn ensure_organizing_revision(
    batch: &CaptureBatchSummary,
    expected_revision: u32,
) -> Result<(), CaptureInboxError> {
    if batch.state != CaptureBatchState::Organizing {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    Ok(())
}
