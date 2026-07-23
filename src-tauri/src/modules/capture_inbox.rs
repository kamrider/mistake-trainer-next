use std::{
    collections::{BTreeSet, HashMap},
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::assets::{decrypt_asset, encrypt_asset, plaintext_sha256},
    modules::capture::{CaptureImageError, inspect_capture_image},
};

pub const MAX_CAPTURE_BATCH_ITEMS: i64 = 150;
pub const MAX_CAPTURE_BATCH_BYTES: i64 = 1024 * 1024 * 1024;
const CAPTURE_PREVIEW_MAX_DIMENSION: u32 = 960;
const CROP_SOURCE_PREVIEW_MAX_DIMENSION: u32 = 2_400;
const MAX_ENCRYPTED_CAPTURE_BYTES: u64 = 25 * 1024 * 1024 + 64;
const MAX_CROP_REGIONS: usize = 10;
const CROP_SOURCE_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

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
pub struct CaptureBatchDetail {
    pub batch: CaptureBatchSummary,
    pub items: Vec<CaptureItemSummary>,
    pub drafts: Vec<CaptureDraftSummary>,
    pub unassigned_item_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItemPreview {
    pub item_id: String,
    pub media_type: String,
    pub data_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCropRecipe {
    pub rect: NormalizedCropRect,
    pub rotation_degrees: u16,
    pub output_media_type: String,
    pub max_edge: u32,
    pub jpeg_quality: u8,
}

#[derive(Clone, Debug)]
pub struct ApplyCaptureCrop {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub item_id: String,
    pub recipes: Vec<CaptureCropRecipe>,
    pub allow_collecting: bool,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct RevertCaptureCrop {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub expected_revision: u32,
    pub derivation_id: String,
    pub allow_collecting: bool,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCropApplyReport {
    pub detail: CaptureBatchDetail,
    pub operation_id: String,
    pub source_item_id: String,
    pub derived_item_ids: Vec<String>,
    pub derivation_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommitReport {
    pub committed_problem_ids: Vec<String>,
    pub committed_count: u32,
    pub remaining_draft_count: u32,
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

pub fn list_capture_batches(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<CaptureBatchSummary>, CaptureInboxError> {
    let mut statement = connection.prepare(
        "SELECT b.id, b.subject, b.state,
                (SELECT COUNT(*) FROM capture_items i WHERE i.batch_id = b.id AND i.superseded_by_derivation_id IS NULL),
                (SELECT COUNT(*) FROM capture_drafts d WHERE d.batch_id = b.id),
                (SELECT COUNT(*) FROM capture_drafts d
                 WHERE d.batch_id = b.id
                   AND trim(COALESCE(NULLIF(d.subject_override, ''), b.subject)) <> ''
                   AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
                   AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')),
                b.updated_at_utc_ms, b.revision
         FROM capture_batches b
         WHERE b.account_id = ?1 AND b.profile_id = ?2
         ORDER BY CASE b.state WHEN 'collecting' THEN 0 WHEN 'organizing' THEN 1 ELSE 2 END,
                  b.updated_at_utc_ms DESC, b.id",
    )?;
    statement
        .query_map(params![account_id, profile_id], map_batch_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CaptureInboxError::from)
}

pub fn get_capture_batch_detail(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    let mut item_statement = connection.prepare(
        "SELECT i.id, i.source_name, i.source_sequence, a.media_type, a.byte_length,
                i.width, i.height, i.staged_role, di.draft_id, di.role, di.position,
                derivation.id, derivation.source_capture_item_id
         FROM capture_items i
         JOIN assets a ON a.id = i.asset_id AND a.account_id = ?1
         LEFT JOIN capture_draft_items di ON di.item_id = i.id
         LEFT JOIN asset_derivations derivation ON derivation.derived_capture_item_id = i.id
         WHERE i.batch_id = ?2 AND i.superseded_by_derivation_id IS NULL
         ORDER BY i.source_sequence, i.id",
    )?;
    let items = item_statement
        .query_map(params![account_id, batch_id], |row| {
            Ok(CaptureItemSummary {
                id: row.get(0)?,
                source_name: row.get(1)?,
                source_sequence: row.get(2)?,
                media_type: row.get(3)?,
                byte_length: row.get::<_, i64>(4)? as f64,
                width: row.get(5)?,
                height: row.get(6)?,
                staged_role: row.get(7)?,
                draft_id: row.get(8)?,
                role: row.get(9)?,
                position: row.get(10)?,
                crop_derivation_id: row.get(11)?,
                crop_source_item_id: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut draft_statement = connection.prepare(
        "SELECT id, position, COALESCE(NULLIF(subject_override, ''), ?2), tags_json, note
         FROM capture_drafts WHERE batch_id = ?1 ORDER BY position, id",
    )?;
    let mut drafts = draft_statement
        .query_map(params![batch_id, batch.subject], |row| {
            let tags_json: String = row.get(3)?;
            Ok(CaptureDraftSummary {
                id: row.get(0)?,
                position: row.get(1)?,
                subject: row.get(2)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                note: row.get(4)?,
                question_item_ids: Vec::new(),
                answer_item_ids: Vec::new(),
                ready: false,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let draft_indexes = drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| (draft.id.clone(), index))
        .collect::<HashMap<_, _>>();
    for item in &items {
        let (Some(draft_id), Some(role)) = (&item.draft_id, &item.role) else {
            continue;
        };
        if let Some(index) = draft_indexes.get(draft_id) {
            if role == "question" {
                drafts[*index].question_item_ids.push(item.id.clone());
            } else if role == "answer" {
                drafts[*index].answer_item_ids.push(item.id.clone());
            }
        }
    }
    for draft in &mut drafts {
        draft.ready = !draft.subject.trim().is_empty()
            && !draft.question_item_ids.is_empty()
            && !draft.answer_item_ids.is_empty();
    }
    let unassigned_item_ids = items
        .iter()
        .filter(|item| item.draft_id.is_none())
        .map(|item| item.id.clone())
        .collect();
    Ok(CaptureBatchDetail {
        batch,
        items,
        drafts,
        unassigned_item_ids,
    })
}

fn map_batch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaptureBatchSummary> {
    let state: String = row.get(2)?;
    Ok(CaptureBatchSummary {
        id: row.get(0)?,
        subject: row.get(1)?,
        state: parse_state(&state),
        item_count: row.get(3)?,
        draft_count: row.get(4)?,
        ready_count: row.get(5)?,
        updated_at_utc_ms: row.get::<_, i64>(6)? as f64,
        revision: row.get(7)?,
    })
}

fn query_batch(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchSummary, CaptureInboxError> {
    connection
        .query_row(
            "SELECT b.id, b.subject, b.state,
                    (SELECT COUNT(*) FROM capture_items i WHERE i.batch_id = b.id AND i.superseded_by_derivation_id IS NULL),
                    (SELECT COUNT(*) FROM capture_drafts d WHERE d.batch_id = b.id),
                    (SELECT COUNT(*) FROM capture_drafts d
                     WHERE d.batch_id = b.id
                       AND trim(COALESCE(NULLIF(d.subject_override, ''), b.subject)) <> ''
                       AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
                       AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')),
                    b.updated_at_utc_ms, b.revision
             FROM capture_batches b
             WHERE b.id = ?1 AND b.account_id = ?2 AND b.profile_id = ?3",
            params![batch_id, account_id, profile_id],
            map_batch_row,
        )
        .optional()?
        .ok_or(CaptureInboxError::BatchNotFound)
}

fn parse_state(value: &str) -> CaptureBatchState {
    match value {
        "collecting" => CaptureBatchState::Collecting,
        "completed" => CaptureBatchState::Completed,
        _ => CaptureBatchState::Organizing,
    }
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
    asset_key: &[u8; 32],
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
    let mut staged_asset: Option<(String, PathBuf, PathBuf, String)> = None;
    let asset_id = if let Some(asset_id) = known_asset {
        asset_id
    } else {
        let asset_id = Uuid::now_v7().to_string();
        let relative = PathBuf::from("blobs")
            .join(&asset_id[..2])
            .join(format!("{asset_id}.mtb"));
        let staging_root = blob_root.join(".staging");
        std::fs::create_dir_all(&staging_root)?;
        let staged_path = staging_root.join(format!("{asset_id}.capture.tmp"));
        let final_path = blob_root.join(&relative);
        let encrypted =
            encrypt_asset(&input.bytes, asset_key).map_err(|_| CaptureInboxError::Crypto)?;
        std::fs::write(&staged_path, encrypted)?;
        staged_asset = Some((
            asset_id.clone(),
            staged_path,
            final_path,
            relative.to_string_lossy().replace('\\', "/"),
        ));
        asset_id
    };
    let item_id = Uuid::now_v7().to_string();
    let source_name = sanitize_source_name(&input.source_name);
    let transaction = connection.transaction()?;
    let persist_result = (|| -> Result<(), CaptureInboxError> {
        if let Some((new_asset_id, _, final_path, relative)) = &staged_asset {
            transaction.execute(
                "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![new_asset_id, input.account_id, hash, relative, byte_length, metadata.media_type, input.now_utc_ms],
            )?;
            if let Some(parent) = final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let staged_path = &staged_asset.as_ref().expect("staged asset").1;
            std::fs::rename(staged_path, final_path)?;
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
        Ok(())
    })();
    if let Err(error) = persist_result {
        if let Some((_, staged_path, final_path, _)) = staged_asset {
            let _ = std::fs::remove_file(staged_path);
            let _ = std::fs::remove_file(final_path);
        }
        return Err(error);
    }
    get_capture_item(connection, &input.account_id, &input.batch_id, &item_id)
}

fn get_capture_item(
    connection: &Connection,
    account_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemSummary, CaptureInboxError> {
    connection
        .query_row(
            "SELECT i.id, i.source_name, i.source_sequence, a.media_type, a.byte_length,
                    i.width, i.height, i.staged_role, di.draft_id, di.role, di.position,
                    derivation.id, derivation.source_capture_item_id
             FROM capture_items i
             JOIN assets a ON a.id = i.asset_id AND a.account_id = ?1
             LEFT JOIN capture_draft_items di ON di.item_id = i.id
             LEFT JOIN asset_derivations derivation ON derivation.derived_capture_item_id = i.id
             WHERE i.batch_id = ?2 AND i.id = ?3",
            params![account_id, batch_id, item_id],
            |row| {
                Ok(CaptureItemSummary {
                    id: row.get(0)?,
                    source_name: row.get(1)?,
                    source_sequence: row.get(2)?,
                    media_type: row.get(3)?,
                    byte_length: row.get::<_, i64>(4)? as f64,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    staged_role: row.get(7)?,
                    draft_id: row.get(8)?,
                    role: row.get(9)?,
                    position: row.get(10)?,
                    crop_derivation_id: row.get(11)?,
                    crop_source_item_id: row.get(12)?,
                })
            },
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)
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

pub fn apply_capture_layout(
    connection: &mut Connection,
    input: ApplyCaptureLayout,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    if batch.state != CaptureBatchState::Organizing {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != input.expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    let question_count = usize::try_from(input.question_images_per_draft).unwrap_or(0);
    let answer_count = usize::try_from(input.answer_images_per_draft).unwrap_or(0);
    if matches!(input.mode, CaptureLayoutMode::Alternating)
        && (question_count == 0 || answer_count == 0 || question_count > 10 || answer_count > 10)
    {
        return Err(CaptureInboxError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    let item_ids = query_batch_item_ids(&transaction, &input.batch_id)?;
    transaction.execute(
        "DELETE FROM capture_drafts WHERE batch_id = ?1",
        [input.batch_id.as_str()],
    )?;
    match input.mode {
        CaptureLayoutMode::Manual => {}
        CaptureLayoutMode::QuestionsOnly => {
            for (position, item_id) in item_ids.iter().enumerate() {
                let draft_id =
                    insert_draft(&transaction, &input.batch_id, position, input.now_utc_ms)?;
                insert_link(&transaction, &draft_id, item_id, "question", 0)?;
            }
        }
        CaptureLayoutMode::Alternating => {
            let group_size = question_count + answer_count;
            for (draft_position, group) in item_ids.chunks(group_size).enumerate() {
                let draft_id = insert_draft(
                    &transaction,
                    &input.batch_id,
                    draft_position,
                    input.now_utc_ms,
                )?;
                for (position, item_id) in group.iter().take(question_count).enumerate() {
                    insert_link(&transaction, &draft_id, item_id, "question", position)?;
                }
                for (position, item_id) in group.iter().skip(question_count).enumerate() {
                    insert_link(&transaction, &draft_id, item_id, "answer", position)?;
                }
            }
        }
        CaptureLayoutMode::Split => {
            let default_split = item_ids.len().div_ceil(2);
            let split = input
                .split_index
                .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
                .unwrap_or(default_split)
                .min(item_ids.len());
            let (questions, answers) = item_ids.split_at(split);
            for (position, question_id) in questions.iter().enumerate() {
                let draft_id =
                    insert_draft(&transaction, &input.batch_id, position, input.now_utc_ms)?;
                insert_link(&transaction, &draft_id, question_id, "question", 0)?;
                if let Some(answer_id) = answers.get(position) {
                    insert_link(&transaction, &draft_id, answer_id, "answer", 0)?;
                }
            }
        }
    }
    transaction.execute(
        "UPDATE capture_batches SET updated_at_utc_ms = ?2, revision = revision + 1 WHERE id = ?1",
        params![input.batch_id, input.now_utc_ms],
    )?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

fn query_batch_item_ids(
    transaction: &Transaction<'_>,
    batch_id: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT id FROM capture_items
         WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL
         ORDER BY source_sequence, id",
    )?;
    statement
        .query_map([batch_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
}

fn insert_draft(
    transaction: &Transaction<'_>,
    batch_id: &str,
    position: usize,
    now_utc_ms: i64,
) -> Result<String, rusqlite::Error> {
    insert_draft_with_subject(transaction, batch_id, position, None, now_utc_ms)
}

fn insert_draft_with_subject(
    transaction: &Transaction<'_>,
    batch_id: &str,
    position: usize,
    subject_override: Option<&str>,
    now_utc_ms: i64,
) -> Result<String, rusqlite::Error> {
    let id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO capture_drafts(
             id, batch_id, position, subject_override, created_at_utc_ms, updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            id,
            batch_id,
            i64::try_from(position).unwrap_or(i64::MAX),
            subject_override,
            now_utc_ms,
        ],
    )?;
    Ok(id)
}

fn repack_draft_positions(
    transaction: &Transaction<'_>,
    batch_id: &str,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_drafts SET position = position + 1000 WHERE batch_id = ?1",
        [batch_id],
    )?;
    let draft_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM capture_drafts WHERE batch_id = ?1 ORDER BY position, id")?;
        statement
            .query_map([batch_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (position, draft_id) in draft_ids.iter().enumerate() {
        transaction.execute(
            "UPDATE capture_drafts SET position = ?1 WHERE id = ?2 AND batch_id = ?3",
            params![
                i64::try_from(position).unwrap_or(i64::MAX),
                draft_id,
                batch_id,
            ],
        )?;
    }
    Ok(())
}

fn insert_link(
    transaction: &Transaction<'_>,
    draft_id: &str,
    item_id: &str,
    role: &str,
    position: usize,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "INSERT INTO capture_draft_items(draft_id, item_id, role, position) VALUES(?1, ?2, ?3, ?4)",
        params![
            draft_id,
            item_id,
            role,
            i64::try_from(position).unwrap_or(i64::MAX)
        ],
    )?;
    transaction.execute(
        "UPDATE capture_items SET staged_role = ?2 WHERE id = ?1",
        params![item_id, role],
    )?;
    Ok(())
}

pub fn move_capture_item(
    connection: &mut Connection,
    input: MoveCaptureItem,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let target_role = match (&input.target_draft_id, input.target_role.as_deref()) {
        (None, None) => None,
        (Some(_), Some("question")) => Some("question"),
        (Some(_), Some("answer")) => Some("answer"),
        _ => return Err(CaptureInboxError::InvalidInput),
    };
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let transaction = connection.transaction()?;
    let item_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM capture_items
         WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id IS NULL)",
        params![input.item_id, input.batch_id],
        |row| row.get(0),
    )?;
    if !item_exists {
        return Err(CaptureInboxError::ItemNotFound);
    }
    let source = transaction
        .query_row(
            "SELECT draft_id, role FROM capture_draft_items WHERE item_id = ?1",
            [input.item_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    transaction.execute(
        "DELETE FROM capture_draft_items WHERE item_id = ?1",
        [input.item_id.as_str()],
    )?;
    if let Some((source_draft_id, source_role)) = source {
        repack_link_positions(&transaction, &source_draft_id, &source_role)?;
    }
    if let (Some(target_draft_id), Some(role)) = (&input.target_draft_id, target_role) {
        let target_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM capture_drafts WHERE id = ?1 AND batch_id = ?2)",
            params![target_draft_id, input.batch_id],
            |row| row.get(0),
        )?;
        if !target_exists {
            return Err(CaptureInboxError::DraftNotFound);
        }
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM capture_draft_items WHERE draft_id = ?1 AND role = ?2",
            params![target_draft_id, role],
            |row| row.get(0),
        )?;
        let position = i64::from(input.target_position).min(count);
        transaction.execute(
            "UPDATE capture_draft_items SET position = position + 1
             WHERE draft_id = ?1 AND role = ?2 AND position >= ?3",
            params![target_draft_id, role, position],
        )?;
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![target_draft_id, input.item_id, role, position],
        )?;
        transaction.execute(
            "UPDATE capture_items SET staged_role = ?2 WHERE id = ?1",
            params![input.item_id, role],
        )?;
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn stage_capture_item_role(
    connection: &mut Connection,
    input: StageCaptureItemRole,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    if input.staged_role != "question" && input.staged_role != "answer" {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE capture_items SET staged_role = ?1
         WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NULL",
        params![input.staged_role, input.item_id, input.batch_id],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::ItemNotFound);
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn merge_capture_card(
    connection: &mut Connection,
    input: MergeCaptureCard,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    if input.item_ids.is_empty() || input.item_ids.len() > MAX_CAPTURE_BATCH_ITEMS as usize {
        return Err(CaptureInboxError::InvalidInput);
    }
    let unique_item_ids = input.item_ids.iter().collect::<BTreeSet<_>>();
    if unique_item_ids.len() != input.item_ids.len() {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let new_draft_subject = normalize_subject(input.new_draft_subject.as_deref().unwrap_or(""))?;
    let subject_override = if new_draft_subject.is_empty() || new_draft_subject == batch.subject {
        None
    } else {
        Some(new_draft_subject.as_str())
    };
    let transaction = connection.transaction()?;
    let target_draft_id = if let Some(target_draft_id) = input.target_draft_id {
        let target_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM capture_drafts WHERE id = ?1 AND batch_id = ?2)",
            params![target_draft_id, input.batch_id],
            |row| row.get(0),
        )?;
        if !target_exists {
            return Err(CaptureInboxError::DraftNotFound);
        }
        target_draft_id
    } else {
        let next_position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM capture_drafts WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?;
        insert_draft_with_subject(
            &transaction,
            &input.batch_id,
            usize::try_from(next_position).unwrap_or(usize::MAX),
            subject_override,
            input.now_utc_ms,
        )?
    };

    for item_id in &input.item_ids {
        let staged_role = transaction
            .query_row(
                "SELECT staged_role FROM capture_items
                 WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id IS NULL",
                params![item_id, input.batch_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(CaptureInboxError::ItemNotFound)?;
        let source = transaction
            .query_row(
                "SELECT draft_id, role FROM capture_draft_items WHERE item_id = ?1",
                [item_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        transaction.execute(
            "DELETE FROM capture_draft_items WHERE item_id = ?1",
            [item_id.as_str()],
        )?;
        if let Some((source_draft_id, source_role)) = source {
            repack_link_positions(&transaction, &source_draft_id, &source_role)?;
        }
        let position: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM capture_draft_items WHERE draft_id = ?1 AND role = ?2",
            params![target_draft_id, staged_role],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![target_draft_id, item_id, staged_role, position],
        )?;
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn delete_capture_draft(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    draft_id: &str,
    expected_revision: u32,
    now_utc_ms: i64,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    ensure_organizing_revision(&batch, expected_revision)?;
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "DELETE FROM capture_drafts WHERE id = ?1 AND batch_id = ?2",
        params![draft_id, batch_id],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::DraftNotFound);
    }
    repack_draft_positions(&transaction, batch_id)?;
    touch_batch(&transaction, batch_id, now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(connection, account_id, profile_id, batch_id)
}

pub fn update_capture_draft(
    connection: &mut Connection,
    input: UpdateCaptureDraft,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let subject = normalize_subject(&input.subject)?;
    let note = input.note.trim();
    if note.chars().count() > 500 || input.tags.len() > 20 {
        return Err(CaptureInboxError::InvalidInput);
    }
    let mut seen = BTreeSet::new();
    let tags = input
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            if tag.chars().count() > 30 {
                Err(CaptureInboxError::InvalidInput)
            } else {
                Ok(tag)
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|tag| seen.insert(tag.clone()))
        .collect::<Vec<_>>();
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE capture_drafts
         SET subject_override = ?1, tags_json = ?2, note = ?3, updated_at_utc_ms = ?4
         WHERE id = ?5 AND batch_id = ?6",
        params![
            subject,
            serde_json::to_string(&tags)?,
            note,
            input.now_utc_ms,
            input.draft_id,
            input.batch_id
        ],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::DraftNotFound);
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn remove_capture_item(
    connection: &mut Connection,
    blob_root: &Path,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    item_id: &str,
    now_utc_ms: i64,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    if batch.state == CaptureBatchState::Completed {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    let is_crop_derivative: bool = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM asset_derivations
           WHERE derived_capture_item_id = ?1 AND batch_id = ?2
         )",
        params![item_id, batch_id],
        |row| row.get(0),
    )?;
    if is_crop_derivative {
        return Err(CaptureInboxError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    let asset = transaction
        .query_row(
            "SELECT a.id, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    validate_relative_asset_path(&asset.1)?;
    transaction.execute("DELETE FROM capture_items WHERE id = ?1", [item_id])?;
    touch_batch(&transaction, batch_id, now_utc_ms)?;
    let orphan = delete_asset_row_if_orphan(&transaction, &asset.0)?;
    transaction.commit()?;
    if orphan {
        let _ = remove_encrypted_blob(blob_root, &asset.1);
    }
    get_capture_batch_detail(connection, account_id, profile_id, batch_id)
}

pub fn discard_capture_batch(
    connection: &mut Connection,
    blob_root: &Path,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<(), CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let transaction = connection.transaction()?;
    let candidates = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT a.id, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.batch_id = ?1 AND a.account_id = ?2",
        )?;
        statement
            .query_map(params![batch_id, account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (_, encrypted_path) in &candidates {
        validate_relative_asset_path(encrypted_path)?;
    }
    transaction.execute(
        "DELETE FROM capture_batches WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
        params![batch_id, account_id, profile_id],
    )?;
    let mut orphan_paths = Vec::new();
    for (asset_id, encrypted_path) in candidates {
        if delete_asset_row_if_orphan(&transaction, &asset_id)? {
            orphan_paths.push(encrypted_path);
        }
    }
    transaction.commit()?;
    for encrypted_path in orphan_paths {
        let _ = remove_encrypted_blob(blob_root, &encrypted_path);
    }
    Ok(())
}

pub fn get_capture_item_preview(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemPreview, CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let (media_type, encrypted_path) = connection
        .query_row(
            "SELECT a.media_type, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let encrypted = read_encrypted_blob(blob_root, &encrypted_path)?;
    let plaintext = decrypt_asset(&encrypted, key).map_err(|_| CaptureInboxError::Crypto)?;
    let format = image_format_for_media_type(&media_type)?;
    let image = image::load_from_memory_with_format(&plaintext, format)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    let thumbnail = image.thumbnail(CAPTURE_PREVIEW_MAX_DIMENSION, CAPTURE_PREVIEW_MAX_DIMENSION);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    Ok(CaptureItemPreview {
        item_id: item_id.to_owned(),
        media_type: "image/png".to_owned(),
        data_url: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(output.into_inner())
        ),
    })
}

pub fn get_capture_crop_source_preview(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemPreview, CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let (media_type, encrypted_path) = connection
        .query_row(
            "SELECT a.media_type, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let plaintext = decrypt_asset(&read_encrypted_blob(blob_root, &encrypted_path)?, key)
        .map_err(|_| CaptureInboxError::Crypto)?;
    let image =
        image::load_from_memory_with_format(&plaintext, image_format_for_media_type(&media_type)?)
            .map_err(|_| CaptureInboxError::InvalidImage)?;
    let preview = image.thumbnail(
        CROP_SOURCE_PREVIEW_MAX_DIMENSION,
        CROP_SOURCE_PREVIEW_MAX_DIMENSION,
    );
    let mut output = Cursor::new(Vec::new());
    preview
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| CaptureInboxError::InvalidImage)?;
    Ok(CaptureItemPreview {
        item_id: item_id.to_owned(),
        media_type: "image/png".to_owned(),
        data_url: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(output.into_inner())
        ),
    })
}

struct CropSource {
    asset_id: String,
    media_type: String,
    encrypted_path: String,
    source_name: String,
    source_sequence: i64,
    staged_role: String,
}

struct EncodedCrop {
    bytes: Vec<u8>,
    media_type: String,
    width: u32,
    height: u32,
    recipe_json: String,
}

struct StagedCropAsset {
    id: String,
    hash: String,
    media_type: String,
    byte_length: i64,
    relative: String,
    staged_path: PathBuf,
    final_path: PathBuf,
}

fn validate_crop_recipe(recipe: &CaptureCropRecipe) -> Result<(), CaptureInboxError> {
    let rect = &recipe.rect;
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f64::is_finite)
        || rect.x < 0.0
        || rect.y < 0.0
        || rect.width <= 0.0
        || rect.height <= 0.0
        || rect.x + rect.width > 1.0 + f64::EPSILON
        || rect.y + rect.height > 1.0 + f64::EPSILON
        || !matches!(recipe.rotation_degrees, 0 | 90 | 180 | 270)
        || !matches!(
            recipe.output_media_type.as_str(),
            "image/png" | "image/jpeg"
        )
        || !(320..=12_000).contains(&recipe.max_edge)
        || !(70..=100).contains(&recipe.jpeg_quality)
    {
        return Err(CaptureInboxError::InvalidCrop);
    }
    Ok(())
}

fn encode_crop(
    source: &image::DynamicImage,
    recipe: &CaptureCropRecipe,
) -> Result<EncodedCrop, CaptureInboxError> {
    validate_crop_recipe(recipe)?;
    let rotated = match recipe.rotation_degrees {
        90 => source.rotate90(),
        180 => source.rotate180(),
        270 => source.rotate270(),
        _ => source.clone(),
    };
    let image_width = rotated.width();
    let image_height = rotated.height();
    let left = (recipe.rect.x * f64::from(image_width)).floor() as u32;
    let top = (recipe.rect.y * f64::from(image_height)).floor() as u32;
    let right = ((recipe.rect.x + recipe.rect.width) * f64::from(image_width)).ceil() as u32;
    let bottom = ((recipe.rect.y + recipe.rect.height) * f64::from(image_height)).ceil() as u32;
    let right = right.min(image_width);
    let bottom = bottom.min(image_height);
    if left >= right || top >= bottom {
        return Err(CaptureInboxError::InvalidCrop);
    }
    let mut cropped = rotated.crop_imm(left, top, right - left, bottom - top);
    if cropped.width() > recipe.max_edge || cropped.height() > recipe.max_edge {
        cropped = cropped.thumbnail(recipe.max_edge, recipe.max_edge);
    }
    let mut output = Cursor::new(Vec::new());
    match recipe.output_media_type.as_str() {
        "image/png" => cropped
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|_| CaptureInboxError::InvalidImage)?,
        "image/jpeg" => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, recipe.jpeg_quality)
                .encode_image(&cropped)
                .map_err(|_| CaptureInboxError::InvalidImage)?
        }
        _ => return Err(CaptureInboxError::InvalidCrop),
    }
    let bytes = output.into_inner();
    if bytes.len() > 25 * 1024 * 1024 {
        return Err(CaptureInboxError::InvalidImage);
    }
    Ok(EncodedCrop {
        width: cropped.width(),
        height: cropped.height(),
        recipe_json: serde_json::to_string(recipe)?,
        media_type: recipe.output_media_type.clone(),
        bytes,
    })
}

pub fn apply_capture_crop(
    connection: &mut Connection,
    blob_root: &Path,
    asset_key: &[u8; 32],
    input: ApplyCaptureCrop,
) -> Result<CaptureCropApplyReport, CaptureInboxError> {
    if input.recipes.is_empty() || input.recipes.len() > MAX_CROP_REGIONS {
        return Err(CaptureInboxError::InvalidCrop);
    }
    for recipe in &input.recipes {
        validate_crop_recipe(recipe)?;
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_crop_revision(&batch, input.expected_revision, input.allow_collecting)?;
    let source = connection
        .query_row(
            "SELECT i.asset_id, a.media_type, a.encrypted_path, i.source_name,
                    i.source_sequence, i.staged_role
             FROM capture_items i JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![input.item_id, input.batch_id, input.account_id],
            |row| {
                Ok(CropSource {
                    asset_id: row.get(0)?,
                    media_type: row.get(1)?,
                    encrypted_path: row.get(2)?,
                    source_name: row.get(3)?,
                    source_sequence: row.get(4)?,
                    staged_role: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let plaintext = decrypt_asset(
        &read_encrypted_blob(blob_root, &source.encrypted_path)?,
        asset_key,
    )
    .map_err(|_| CaptureInboxError::Crypto)?;
    let source_image = image::load_from_memory_with_format(
        &plaintext,
        image_format_for_media_type(&source.media_type)?,
    )
    .map_err(|_| CaptureInboxError::InvalidImage)?;
    let encoded = input
        .recipes
        .iter()
        .map(|recipe| encode_crop(&source_image, recipe))
        .collect::<Result<Vec<_>, _>>()?;
    let (active_count, stored_bytes): (i64, i64) = connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL),
           (SELECT COALESCE(SUM(a.byte_length), 0) FROM capture_items i JOIN assets a ON a.id = i.asset_id WHERE i.batch_id = ?1)",
        [input.batch_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let resulting_count = active_count - 1 + i64::try_from(encoded.len()).unwrap_or(i64::MAX);
    let added_bytes = encoded.iter().fold(0_i64, |total, crop| {
        total.saturating_add(i64::try_from(crop.bytes.len()).unwrap_or(i64::MAX))
    });
    if resulting_count > MAX_CAPTURE_BATCH_ITEMS
        || stored_bytes.saturating_add(added_bytes) > MAX_CAPTURE_BATCH_BYTES
    {
        return Err(CaptureInboxError::CapacityReached);
    }

    let staging_root = blob_root.join(".staging");
    std::fs::create_dir_all(&staging_root)?;
    let mut asset_by_hash = HashMap::<String, String>::new();
    let mut staged_assets = Vec::<StagedCropAsset>::new();
    let mut derived_asset_ids = Vec::with_capacity(encoded.len());
    let stage_result = (|| -> Result<(), CaptureInboxError> {
        for crop in &encoded {
            let hash = plaintext_sha256(&crop.bytes);
            if let Some(asset_id) = asset_by_hash.get(&hash) {
                derived_asset_ids.push(asset_id.clone());
                continue;
            }
            if let Some(asset_id) = connection
                .query_row(
                    "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
                    params![input.account_id, hash],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                asset_by_hash.insert(hash, asset_id.clone());
                derived_asset_ids.push(asset_id);
                continue;
            }
            let asset_id = Uuid::now_v7().to_string();
            let relative_path = PathBuf::from("blobs")
                .join(&asset_id[..2])
                .join(format!("{asset_id}.mtb"));
            let staged_path = staging_root.join(format!("{asset_id}.crop.tmp"));
            let final_path = blob_root.join(&relative_path);
            let encrypted =
                encrypt_asset(&crop.bytes, asset_key).map_err(|_| CaptureInboxError::Crypto)?;
            if let Err(error) = std::fs::write(&staged_path, encrypted) {
                let _ = std::fs::remove_file(&staged_path);
                return Err(error.into());
            }
            asset_by_hash.insert(hash.clone(), asset_id.clone());
            derived_asset_ids.push(asset_id.clone());
            staged_assets.push(StagedCropAsset {
                id: asset_id,
                hash,
                media_type: crop.media_type.clone(),
                byte_length: i64::try_from(crop.bytes.len()).unwrap_or(i64::MAX),
                relative: relative_path.to_string_lossy().replace('\\', "/"),
                staged_path,
                final_path,
            });
        }
        Ok(())
    })();
    if let Err(error) = stage_result {
        for asset in &staged_assets {
            let _ = std::fs::remove_file(&asset.staged_path);
        }
        return Err(error);
    }

    let operation_id = Uuid::now_v7().to_string();
    let derived_item_ids = (0..encoded.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let derivation_ids = (0..encoded.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let transaction = connection.transaction()?;
    let persist_result = (|| -> Result<(), CaptureInboxError> {
        for asset in &staged_assets {
            transaction.execute(
                "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![asset.id, input.account_id, asset.hash, asset.relative, asset.byte_length, asset.media_type, input.now_utc_ms],
            )?;
            if let Some(parent) = asset.final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&asset.staged_path, &asset.final_path)?;
        }
        let source_link = transaction
            .query_row(
                "SELECT draft_id, role, position FROM capture_draft_items WHERE item_id = ?1",
                [input.item_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        transaction.execute(
            "DELETE FROM capture_draft_items WHERE item_id = ?1",
            [input.item_id.as_str()],
        )?;
        let sequence_shift = i64::try_from(encoded.len()).unwrap_or(i64::MAX);
        let sequence_offset: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(source_sequence), 0) + 1000 FROM capture_items WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE capture_items SET source_sequence = source_sequence + ?1
             WHERE batch_id = ?2 AND source_sequence >= ?3",
            params![sequence_offset, input.batch_id, source.source_sequence],
        )?;
        transaction.execute(
            "UPDATE capture_items SET source_sequence = source_sequence - ?1 + ?2
             WHERE batch_id = ?3 AND source_sequence >= ?4",
            params![
                sequence_offset,
                sequence_shift,
                input.batch_id,
                source.source_sequence + sequence_offset
            ],
        )?;
        let extension = if encoded[0].media_type == "image/jpeg" {
            "jpg"
        } else {
            "png"
        };
        let base_name = Path::new(&source.source_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("image");
        for (position, crop) in encoded.iter().enumerate() {
            let item_id = &derived_item_ids[position];
            let derivation_id = &derivation_ids[position];
            let source_name = sanitize_source_name(&format!(
                "{base_name}-crop-{}.{}",
                position + 1,
                if crop.media_type == "image/jpeg" {
                    "jpg"
                } else {
                    extension
                }
            ));
            transaction.execute(
                "INSERT INTO capture_items(
                   id, batch_id, asset_id, client_upload_id, source_name, source_sequence,
                   width, height, created_at_utc_ms, staged_role
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    item_id,
                    input.batch_id,
                    derived_asset_ids[position],
                    format!("crop:{operation_id}:{position}"),
                    source_name,
                    source.source_sequence + i64::try_from(position).unwrap_or(i64::MAX),
                    crop.width,
                    crop.height,
                    input.now_utc_ms,
                    source.staged_role
                ],
            )?;
            transaction.execute(
                "INSERT INTO asset_derivations(
                   id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                   source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                   engine, engine_version, confidence, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'crop', ?10, 'mistake-trainer-crop', '1', NULL, ?11)",
                params![derivation_id, operation_id, input.account_id, input.batch_id, source.asset_id, derived_asset_ids[position], input.item_id, item_id, i64::try_from(position).unwrap_or(i64::MAX), crop.recipe_json, input.now_utc_ms],
            )?;
        }
        if let Some((draft_id, role, position)) = source_link {
            transaction.execute(
                "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
                 VALUES(?1, ?2, ?3, ?4)",
                params![draft_id, derived_item_ids[0], role, position],
            )?;
        }
        transaction.execute(
            "UPDATE capture_items SET superseded_by_derivation_id = ?1
             WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NULL",
            params![derivation_ids[0], input.item_id, input.batch_id],
        )?;
        transaction.execute(
            "INSERT INTO capture_source_retention(batch_id, source_asset_id, retain_until_utc_ms, reason, created_at_utc_ms)
             VALUES(?1, ?2, ?3, 'crop_recovery', ?4)
             ON CONFLICT(batch_id, source_asset_id) DO UPDATE SET retain_until_utc_ms = excluded.retain_until_utc_ms",
            params![input.batch_id, source.asset_id, input.now_utc_ms.saturating_add(CROP_SOURCE_RETENTION_MS), input.now_utc_ms],
        )?;
        touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = persist_result {
        for asset in &staged_assets {
            let _ = std::fs::remove_file(&asset.staged_path);
            let _ = std::fs::remove_file(&asset.final_path);
        }
        return Err(error);
    }
    let detail = get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    Ok(CaptureCropApplyReport {
        detail,
        operation_id,
        source_item_id: input.item_id,
        derived_item_ids,
        derivation_ids,
    })
}

pub fn revert_capture_crop(
    connection: &mut Connection,
    blob_root: &Path,
    input: RevertCaptureCrop,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_crop_revision(&batch, input.expected_revision, input.allow_collecting)?;
    let (operation_id, source_item_id, source_asset_id): (String, String, String) = connection
        .query_row(
            "SELECT operation_id, source_capture_item_id, source_asset_id
             FROM asset_derivations
             WHERE id = ?1 AND batch_id = ?2 AND account_id = ?3",
            params![input.derivation_id, input.batch_id, input.account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::CropNotRevertible)?;
    let transaction = connection.transaction()?;
    let derived = {
        let mut statement = transaction.prepare(
            "SELECT d.derived_capture_item_id, d.derived_asset_id, a.encrypted_path, d.position,
                    i.source_sequence
             FROM asset_derivations d
             JOIN assets a ON a.id = d.derived_asset_id
             JOIN capture_items i ON i.id = d.derived_capture_item_id
             WHERE d.operation_id = ?1 AND d.batch_id = ?2
             ORDER BY d.position",
        )?;
        statement
            .query_map(params![operation_id, input.batch_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if derived.is_empty() {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    let existing_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_items
         WHERE batch_id = ?1 AND id IN (
           SELECT derived_capture_item_id FROM asset_derivations WHERE operation_id = ?2
         )",
        params![input.batch_id, operation_id],
        |row| row.get(0),
    )?;
    if existing_count != i64::try_from(derived.len()).unwrap_or(i64::MAX) {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    let first_link = transaction
        .query_row(
            "SELECT di.draft_id, di.role, di.position
             FROM asset_derivations d
             JOIN capture_draft_items di ON di.item_id = d.derived_capture_item_id
             WHERE d.operation_id = ?1 AND d.position = 0",
            [operation_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let affected_links = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT di.draft_id, di.role
             FROM asset_derivations d JOIN capture_draft_items di ON di.item_id = d.derived_capture_item_id
             WHERE d.operation_id = ?1",
        )?;
        statement
            .query_map([operation_id.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let first_sequence = derived
        .iter()
        .map(|entry| entry.4)
        .min()
        .ok_or(CaptureInboxError::CropNotRevertible)?;
    let hidden_sequence: i64 = transaction.query_row(
        "SELECT source_sequence FROM capture_items WHERE id = ?1 AND batch_id = ?2",
        params![source_item_id, input.batch_id],
        |row| row.get(0),
    )?;
    for (item_id, _, _, _, _) in &derived {
        transaction.execute("DELETE FROM capture_items WHERE id = ?1", [item_id])?;
    }
    transaction.execute(
        "DELETE FROM asset_derivations WHERE operation_id = ?1 AND batch_id = ?2",
        params![operation_id, input.batch_id],
    )?;
    transaction.execute(
        "DELETE FROM capture_source_retention WHERE batch_id = ?1 AND source_asset_id = ?2",
        params![input.batch_id, source_asset_id],
    )?;
    let sequence_offset: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(source_sequence), 0) + 1000 FROM capture_items WHERE batch_id = ?1",
        [input.batch_id.as_str()],
        |row| row.get(0),
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence + ?1 WHERE id = ?2 AND batch_id = ?3",
        params![sequence_offset, source_item_id, input.batch_id],
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence + ?1
         WHERE batch_id = ?2 AND superseded_by_derivation_id IS NULL AND source_sequence > ?3",
        params![sequence_offset, input.batch_id, hidden_sequence],
    )?;
    transaction.execute(
        "UPDATE capture_items SET source_sequence = source_sequence - ?1 - ?2
         WHERE batch_id = ?3 AND superseded_by_derivation_id IS NULL AND source_sequence > ?4",
        params![
            sequence_offset,
            i64::try_from(derived.len()).unwrap_or(i64::MAX),
            input.batch_id,
            hidden_sequence + sequence_offset
        ],
    )?;
    let restored = transaction.execute(
        "UPDATE capture_items SET superseded_by_derivation_id = NULL, source_sequence = ?1
         WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NOT NULL",
        params![first_sequence, source_item_id, input.batch_id],
    )?;
    if restored != 1 {
        return Err(CaptureInboxError::CropNotRevertible);
    }
    if let Some((draft_id, role, position)) = first_link {
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![draft_id, source_item_id, role, position],
        )?;
    }
    for (draft_id, role) in affected_links {
        repack_link_positions(&transaction, &draft_id, &role)?;
    }
    let mut orphan_paths = Vec::new();
    let mut seen_assets = BTreeSet::new();
    for (_, asset_id, encrypted_path, _, _) in &derived {
        if seen_assets.insert(asset_id.clone())
            && delete_asset_row_if_orphan(&transaction, asset_id)?
        {
            orphan_paths.push(encrypted_path.clone());
        }
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    for path in orphan_paths {
        let _ = remove_encrypted_blob(blob_root, &path);
    }
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn commit_ready_capture_drafts(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    now_utc_ms: i64,
) -> Result<CaptureCommitReport, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    ensure_organizing_revision(&batch, expected_revision)?;
    let transaction = connection.transaction()?;
    let ready_drafts = query_ready_drafts(&transaction, batch_id, &batch.subject)?;
    let mut committed_problem_ids = Vec::with_capacity(ready_drafts.len());
    for draft in ready_drafts {
        let links = query_draft_asset_links(&transaction, &draft.id)?;
        let problem_id = Uuid::now_v7().to_string();
        transaction.execute(
            "INSERT INTO problems(id, account_id, profile_id, subject, tags_json, note, status,
                                  created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?7, 1)",
            params![
                problem_id,
                account_id,
                profile_id,
                draft.subject,
                draft.tags_json,
                draft.note,
                now_utc_ms
            ],
        )?;
        let mut seen_links = BTreeSet::new();
        let mut asset_ids = Vec::new();
        for link in &links {
            if seen_links.insert((link.role.clone(), link.asset_id.clone())) {
                let position = asset_ids_for_role(&links, &link.role, &link.asset_id);
                transaction.execute(
                    "INSERT INTO problem_assets(problem_id, asset_id, role, position)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![problem_id, link.asset_id, link.role, position],
                )?;
                asset_ids.push(link.asset_id.clone());
            }
        }
        for asset_id in asset_ids.iter().collect::<BTreeSet<_>>() {
            let metadata = query_asset_sync_payload(&transaction, account_id, asset_id)?;
            transaction.execute(
                "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id,
                                             operation, payload_json, status, attempt_count,
                                             created_at_utc_ms, next_attempt_at_utc_ms)
                 VALUES(?1, ?2, ?3, 'asset', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
                params![
                    Uuid::now_v7().to_string(),
                    account_id,
                    profile_id,
                    asset_id,
                    metadata,
                    now_utc_ms
                ],
            )?;
        }
        let tags: Vec<String> = serde_json::from_str(&draft.tags_json)?;
        let problem_payload = serde_json::to_string(&serde_json::json!({
            "id": problem_id,
            "accountId": account_id,
            "profileId": profile_id,
            "subject": draft.subject,
            "tags": tags,
            "note": draft.note,
            "assetIds": asset_ids,
            "createdAtUtcMs": now_utc_ms,
            "updatedAtUtcMs": now_utc_ms,
            "revision": 1
        }))?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id,
                                         operation, payload_json, status, attempt_count,
                                         created_at_utc_ms, next_attempt_at_utc_ms)
             VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
            params![
                Uuid::now_v7().to_string(),
                account_id,
                profile_id,
                problem_id,
                problem_payload,
                now_utc_ms
            ],
        )?;
        transaction.execute(
            "DELETE FROM capture_items WHERE id IN
             (SELECT item_id FROM capture_draft_items WHERE draft_id = ?1)",
            [draft.id.as_str()],
        )?;
        transaction.execute(
            "DELETE FROM capture_drafts WHERE id = ?1",
            [draft.id.as_str()],
        )?;
        committed_problem_ids.push(problem_id);
    }
    let remaining_draft_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_drafts WHERE batch_id = ?1",
        [batch_id],
        |row| row.get(0),
    )?;
    let remaining_item_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_items
         WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL",
        [batch_id],
        |row| row.get(0),
    )?;
    let next_state = if remaining_draft_count == 0 && remaining_item_count == 0 {
        CaptureBatchState::Completed
    } else {
        CaptureBatchState::Organizing
    };
    transaction.execute(
        "UPDATE capture_batches SET state = ?2, updated_at_utc_ms = ?3, revision = revision + 1
         WHERE id = ?1",
        params![batch_id, next_state.as_str(), now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(CaptureCommitReport {
        committed_count: u32::try_from(committed_problem_ids.len()).unwrap_or(u32::MAX),
        committed_problem_ids,
        remaining_draft_count: u32::try_from(remaining_draft_count).unwrap_or(u32::MAX),
    })
}

fn ensure_organizing_revision(
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

fn ensure_crop_revision(
    batch: &CaptureBatchSummary,
    expected_revision: u32,
    allow_collecting: bool,
) -> Result<(), CaptureInboxError> {
    let state_allowed = batch.state == CaptureBatchState::Organizing
        || (allow_collecting && batch.state == CaptureBatchState::Collecting);
    if !state_allowed {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    Ok(())
}

fn touch_batch(
    transaction: &Transaction<'_>,
    batch_id: &str,
    now_utc_ms: i64,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_batches SET updated_at_utc_ms = ?2, revision = revision + 1 WHERE id = ?1",
        params![batch_id, now_utc_ms],
    )?;
    Ok(())
}

fn repack_link_positions(
    transaction: &Transaction<'_>,
    draft_id: &str,
    role: &str,
) -> Result<(), rusqlite::Error> {
    let item_ids = {
        let mut statement = transaction.prepare(
            "SELECT item_id FROM capture_draft_items
             WHERE draft_id = ?1 AND role = ?2 ORDER BY position, item_id",
        )?;
        statement
            .query_map(params![draft_id, role], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (position, item_id) in item_ids.iter().enumerate() {
        transaction.execute(
            "UPDATE capture_draft_items SET position = ?1 WHERE item_id = ?2",
            params![i64::try_from(position).unwrap_or(i64::MAX), item_id],
        )?;
    }
    Ok(())
}

fn delete_asset_row_if_orphan(
    transaction: &Transaction<'_>,
    asset_id: &str,
) -> Result<bool, rusqlite::Error> {
    let referenced: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM capture_items WHERE asset_id = ?1
            UNION ALL SELECT 1 FROM problem_assets WHERE asset_id = ?1
         )",
        [asset_id],
        |row| row.get(0),
    )?;
    if referenced {
        return Ok(false);
    }
    Ok(transaction.execute("DELETE FROM assets WHERE id = ?1", [asset_id])? == 1)
}

fn validate_relative_asset_path(encrypted_path: &str) -> Result<&Path, CaptureInboxError> {
    let relative = Path::new(encrypted_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CaptureInboxError::InvalidAssetPath);
    }
    Ok(relative)
}

fn remove_encrypted_blob(blob_root: &Path, encrypted_path: &str) -> Result<(), CaptureInboxError> {
    let path = blob_root.join(validate_relative_asset_path(encrypted_path)?);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn read_encrypted_blob(
    blob_root: &Path,
    encrypted_path: &str,
) -> Result<Vec<u8>, CaptureInboxError> {
    let file = std::fs::File::open(blob_root.join(validate_relative_asset_path(encrypted_path)?))?;
    let mut reader = file.take(MAX_ENCRYPTED_CAPTURE_BYTES + 1);
    let mut encrypted = Vec::new();
    reader.read_to_end(&mut encrypted)?;
    if u64::try_from(encrypted.len()).unwrap_or(u64::MAX) > MAX_ENCRYPTED_CAPTURE_BYTES {
        return Err(CaptureInboxError::InvalidImage);
    }
    Ok(encrypted)
}

fn image_format_for_media_type(media_type: &str) -> Result<image::ImageFormat, CaptureInboxError> {
    match media_type {
        "image/png" => Ok(image::ImageFormat::Png),
        "image/jpeg" => Ok(image::ImageFormat::Jpeg),
        "image/webp" => Ok(image::ImageFormat::WebP),
        _ => Err(CaptureInboxError::InvalidImage),
    }
}

struct ReadyDraft {
    id: String,
    subject: String,
    tags_json: String,
    note: String,
}

struct DraftAssetLink {
    asset_id: String,
    role: String,
}

fn query_ready_drafts(
    transaction: &Transaction<'_>,
    batch_id: &str,
    batch_subject: &str,
) -> Result<Vec<ReadyDraft>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT d.id, COALESCE(NULLIF(trim(d.subject_override), ''), ?2), d.tags_json, d.note
         FROM capture_drafts d
         WHERE d.batch_id = ?1
           AND trim(COALESCE(NULLIF(d.subject_override, ''), ?2)) <> ''
           AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
           AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')
         ORDER BY d.position, d.id",
    )?;
    statement
        .query_map(params![batch_id, batch_subject], |row| {
            Ok(ReadyDraft {
                id: row.get(0)?,
                subject: row.get(1)?,
                tags_json: row.get(2)?,
                note: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn query_draft_asset_links(
    transaction: &Transaction<'_>,
    draft_id: &str,
) -> Result<Vec<DraftAssetLink>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT i.asset_id, di.role FROM capture_draft_items di
         JOIN capture_items i ON i.id = di.item_id
         WHERE di.draft_id = ?1
         ORDER BY CASE di.role WHEN 'question' THEN 0 ELSE 1 END, di.position, di.item_id",
    )?;
    statement
        .query_map([draft_id], |row| {
            Ok(DraftAssetLink {
                asset_id: row.get(0)?,
                role: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn asset_ids_for_role(links: &[DraftAssetLink], role: &str, target_asset_id: &str) -> i64 {
    let mut seen = BTreeSet::new();
    for link in links.iter().filter(|link| link.role == role) {
        if !seen.insert(link.asset_id.as_str()) {
            continue;
        }
        if link.asset_id == target_asset_id {
            return i64::try_from(seen.len() - 1).unwrap_or(i64::MAX);
        }
    }
    i64::try_from(seen.len()).unwrap_or(i64::MAX)
}

fn query_asset_sync_payload(
    transaction: &Transaction<'_>,
    account_id: &str,
    asset_id: &str,
) -> Result<String, CaptureInboxError> {
    let value = transaction.query_row(
        "SELECT id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
                created_at_utc_ms FROM assets WHERE id = ?1 AND account_id = ?2",
        params![asset_id, account_id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "accountId": row.get::<_, String>(1)?,
                "plaintextSha256": row.get::<_, String>(2)?,
                "encryptedPath": row.get::<_, String>(3)?,
                "byteLength": row.get::<_, i64>(4)?,
                "mediaType": row.get::<_, String>(5)?,
                "createdAtUtcMs": row.get::<_, i64>(6)?,
            }))
        },
    )?;
    Ok(serde_json::to_string(&value)?)
}
