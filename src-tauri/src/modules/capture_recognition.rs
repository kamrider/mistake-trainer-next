use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::assets::{decrypt_asset, encrypt_asset, plaintext_sha256},
    modules::capture_inbox::{
        CaptureBatchDetail, CaptureCropRecipe, CaptureInboxError, EncodedCrop,
        MAX_CAPTURE_BATCH_BYTES, MAX_CAPTURE_BATCH_ITEMS, NormalizedCropRect, encode_crop,
        get_capture_batch_detail, image_format_for_media_type, read_encrypted_blob,
        remove_encrypted_blob,
    },
};

const MAX_JOB_ITEMS: usize = 150;
const MAX_REGIONS_PER_ITEM: usize = MAX_CAPTURE_BATCH_ITEMS as usize;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionJobState {
    Queued,
    Running,
    Review,
    Applied,
    Cancelled,
    Failed,
}

impl CaptureRecognitionJobState {
    fn from_database(value: &str) -> rusqlite::Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "review" => Ok(Self::Review),
            "applied" => Ok(Self::Applied),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionReviewBand {
    High,
    Review,
    Low,
}

impl CaptureRecognitionReviewBand {
    fn from_database(value: &str) -> rusqlite::Result<Self> {
        match value {
            "high" => Ok(Self::High),
            "review" => Ok(Self::Review),
            "low" => Ok(Self::Low),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Review => "review",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionSuggestionState {
    Proposed,
    Accepted,
    Rejected,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionDecision {
    Accepted,
    Rejected,
}

impl CaptureRecognitionSuggestionState {
    fn from_database(value: &str) -> rusqlite::Result<Self> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "stale" => Ok(Self::Stale),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionRole {
    Question,
    Answer,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRecognitionReasonCode {
    ClearQuestionAnchor,
    MatchedQuestionAnswerAnchor,
    ConsistentReadingOrder,
    WeakAnchor,
    AmbiguousColumns,
    PossibleContentCut,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionRegionProposal {
    pub rect: NormalizedCropRect,
    pub role: CaptureRecognitionRole,
    pub group_slot: Option<u32>,
    pub confidence_basis_points: u16,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionSuggestion {
    pub id: String,
    pub item_id: String,
    pub regions: Vec<CaptureRecognitionRegionProposal>,
    pub confidence_basis_points: u16,
    pub review_band: CaptureRecognitionReviewBand,
    pub state: CaptureRecognitionSuggestionState,
    pub reason_codes: Vec<CaptureRecognitionReasonCode>,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionJob {
    pub id: String,
    pub batch_id: String,
    pub state: CaptureRecognitionJobState,
    pub total_items: u32,
    pub processed_items: u32,
    pub suggestions: Vec<CaptureRecognitionSuggestion>,
    pub created_at_utc_ms: f64,
    pub updated_at_utc_ms: f64,
}

#[derive(Clone, Debug)]
pub struct CreateCaptureRecognitionJob {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub item_ids: Vec<String>,
    pub engine: String,
    pub engine_version: String,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct StoreCaptureRecognitionSuggestion {
    pub account_id: String,
    pub profile_id: String,
    pub job_id: String,
    pub item_id: String,
    pub regions: Vec<CaptureRecognitionRegionProposal>,
    pub confidence_basis_points: u16,
    pub reason_codes: Vec<CaptureRecognitionReasonCode>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ClaimedCaptureRecognitionItem {
    pub item_id: String,
    pub batch_id: String,
    pub encrypted_path: String,
    pub media_type: String,
    pub staged_role: CaptureRecognitionRole,
    pub source_snapshot_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct ReviewCaptureRecognitionSuggestion {
    pub account_id: String,
    pub profile_id: String,
    pub job_id: String,
    pub suggestion_id: String,
    pub decision: CaptureRecognitionDecision,
    pub edited_regions: Option<Vec<CaptureRecognitionRegionProposal>>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ApplyCaptureRecognition {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub accepted_suggestion_ids: Vec<String>,
    pub blob_root: PathBuf,
    pub asset_key: [u8; 32],
    pub now_utc_ms: i64,
    pub failure_point: Option<CaptureRecognitionFailurePoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureRecognitionFailurePoint {
    BeforeStaging,
    AfterStaging,
    InTransaction,
}

#[derive(Clone, Debug)]
pub struct RevertCaptureRecognition {
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub operation_id: String,
    pub expected_revision: u32,
    pub blob_root: PathBuf,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionApplyReport {
    pub operation_id: String,
    pub applied_suggestion_count: u32,
    pub created_draft_count: u32,
    pub created_item_count: u32,
    pub unmatched_answer_count: u32,
    pub stale_suggestion_count: u32,
    pub detail: CaptureBatchDetail,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionRevertReport {
    pub operation_id: String,
    pub reverted_item_count: u32,
    pub detail: CaptureBatchDetail,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionOperationSummary {
    pub operation_id: String,
    pub batch_id: String,
    pub after_revision: u32,
    pub created_item_count: u32,
    pub reverted: bool,
}

#[derive(Debug, Error)]
pub enum CaptureRecognitionError {
    #[error("capture recognition batch was not found")]
    BatchNotFound,
    #[error("capture recognition job was not found")]
    JobNotFound,
    #[error("capture recognition item was not found")]
    ItemNotFound,
    #[error("capture recognition batch is not organizing")]
    InvalidState,
    #[error("capture recognition input is invalid")]
    InvalidInput,
    #[error("capture recognition suggestion is invalid")]
    InvalidSuggestion,
    #[error("capture recognition batch changed")]
    RevisionConflict,
    #[error("capture recognition suggestions became stale")]
    Stale,
    #[error("capture recognition apply capacity was reached")]
    CapacityReached,
    #[error("capture recognition operation can no longer be reverted")]
    RevertConflict,
    #[error("capture recognition filesystem error")]
    Io(#[from] std::io::Error),
    #[error("capture recognition asset encryption failed")]
    Crypto,
    #[error("capture recognition injected failure")]
    InjectedFailure,
    #[error("capture recognition inbox operation failed")]
    Inbox(#[from] CaptureInboxError),
    #[error("capture recognition database error")]
    Database(#[from] rusqlite::Error),
    #[error("capture recognition serialization error")]
    Serialization(#[from] serde_json::Error),
}

impl CaptureRecognitionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BatchNotFound => "capture_recognition_batch_not_found",
            Self::JobNotFound => "capture_recognition_job_not_found",
            Self::ItemNotFound => "capture_recognition_item_not_found",
            Self::InvalidState => "capture_recognition_invalid_state",
            Self::InvalidInput | Self::InvalidSuggestion => "capture_recognition_invalid_input",
            Self::RevisionConflict => "capture_recognition_revision_conflict",
            Self::Stale => "capture_recognition_stale",
            Self::CapacityReached => "capture_recognition_capacity_reached",
            Self::RevertConflict => "capture_recognition_revert_conflict",
            Self::Database(_)
            | Self::Serialization(_)
            | Self::Io(_)
            | Self::Crypto
            | Self::InjectedFailure
            | Self::Inbox(_) => "capture_recognition_failed",
        }
    }
}

pub fn create_or_resume_recognition_job(
    connection: &mut Connection,
    input: CreateCaptureRecognitionJob,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if input.item_ids.is_empty()
        || input.item_ids.len() > MAX_JOB_ITEMS
        || input.engine.trim().is_empty()
        || input.engine.len() > 60
        || input.engine_version.trim().is_empty()
        || input.engine_version.len() > 60
    {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let unique = input.item_ids.iter().collect::<BTreeSet<_>>();
    if unique.len() != input.item_ids.len() {
        return Err(CaptureRecognitionError::InvalidInput);
    }

    let batch_state = connection
        .query_row(
            "SELECT state FROM capture_batches
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.batch_id, input.account_id, input.profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::BatchNotFound)?;
    if batch_state != "organizing" {
        return Err(CaptureRecognitionError::InvalidState);
    }

    if let Some(existing) = get_active_recognition_job(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )? {
        return Ok(existing);
    }

    let mut snapshots = Vec::with_capacity(input.item_ids.len());
    for item_id in &input.item_ids {
        snapshots.push(capture_item_snapshot_hash(
            connection,
            &input.account_id,
            &input.profile_id,
            &input.batch_id,
            item_id,
        )?);
    }

    let id = Uuid::now_v7().to_string();
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO capture_recognition_jobs(
           id, account_id, profile_id, batch_id, state, engine, engine_version,
           model_component_id, total_items, processed_items, created_at_utc_ms,
           updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, 'queued', ?5, ?6, 'ppocrv6_small', ?7, 0, ?8, ?8)",
        params![
            id,
            input.account_id,
            input.profile_id,
            input.batch_id,
            input.engine,
            input.engine_version,
            i64::try_from(input.item_ids.len()).unwrap_or(i64::MAX),
            input.now_utc_ms,
        ],
    )?;
    for (position, (item_id, snapshot)) in input.item_ids.iter().zip(snapshots).enumerate() {
        transaction.execute(
            "INSERT INTO capture_recognition_job_items(
               job_id, item_id, source_snapshot_hash, position, state
             ) VALUES(?1, ?2, ?3, ?4, 'pending')",
            params![
                id,
                item_id,
                snapshot.as_slice(),
                i64::try_from(position).unwrap_or(i64::MAX),
            ],
        )?;
    }
    transaction.commit()?;
    get_recognition_job_by_id(connection, &input.account_id, &input.profile_id, &id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn get_active_recognition_job(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionError> {
    let id = connection
        .query_row(
            "SELECT id FROM capture_recognition_jobs
             WHERE account_id = ?1 AND profile_id = ?2 AND batch_id = ?3
               AND state IN ('queued', 'running', 'review')
             ORDER BY updated_at_utc_ms DESC, id DESC
             LIMIT 1",
            params![account_id, profile_id, batch_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match id {
        Some(id) => get_recognition_job_by_id(connection, account_id, profile_id, &id),
        None => Ok(None),
    }
}

pub fn get_recognition_job_by_id(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionError> {
    let row = connection
        .query_row(
            "SELECT id, batch_id, state, total_items, processed_items,
                    created_at_utc_ms, updated_at_utc_ms
             FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![job_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((id, batch_id, state, total_items, processed_items, created, updated)) = row else {
        return Ok(None);
    };
    Ok(Some(CaptureRecognitionJob {
        suggestions: list_suggestions(connection, &id)?,
        id,
        batch_id,
        state: CaptureRecognitionJobState::from_database(&state)?,
        total_items: u32::try_from(total_items).unwrap_or(u32::MAX),
        processed_items: u32::try_from(processed_items).unwrap_or(u32::MAX),
        created_at_utc_ms: created as f64,
        updated_at_utc_ms: updated as f64,
    }))
}

pub fn store_recognition_suggestion(
    connection: &mut Connection,
    input: StoreCaptureRecognitionSuggestion,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    validate_regions(&input.regions, input.confidence_basis_points)?;
    let job_item_exists: bool = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM capture_recognition_jobs j
           JOIN capture_recognition_job_items ji ON ji.job_id = j.id
           WHERE j.id = ?1 AND j.account_id = ?2 AND j.profile_id = ?3
             AND ji.item_id = ?4 AND j.state IN ('queued', 'running')
         )",
        params![
            input.job_id,
            input.account_id,
            input.profile_id,
            input.item_id
        ],
        |row| row.get(0),
    )?;
    if !job_item_exists {
        return Err(CaptureRecognitionError::ItemNotFound);
    }
    let band = review_band(input.confidence_basis_points);
    let suggestion_id = Uuid::now_v7().to_string();
    let regions_json = serde_json::to_string(&input.regions)?;
    let reasons_json = serde_json::to_string(&input.reason_codes)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO capture_recognition_suggestions(
           id, job_id, item_id, regions_json, confidence_basis_points,
           review_band, state, reason_codes_json
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'proposed', ?7)
         ON CONFLICT(job_id, item_id) DO UPDATE SET
           regions_json = excluded.regions_json,
           confidence_basis_points = excluded.confidence_basis_points,
           review_band = excluded.review_band,
           state = 'proposed',
           reason_codes_json = excluded.reason_codes_json,
           reviewed_at_utc_ms = NULL",
        params![
            suggestion_id,
            input.job_id,
            input.item_id,
            regions_json,
            input.confidence_basis_points,
            band.as_str(),
            reasons_json,
        ],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = 'complete'
         WHERE job_id = ?1 AND item_id = ?2",
        params![input.job_id, input.item_id],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET processed_items = (
           SELECT COUNT(*) FROM capture_recognition_job_items
           WHERE job_id = ?1 AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
         ),
         state = CASE
           WHEN (SELECT COUNT(*) FROM capture_recognition_job_items
                 WHERE job_id = ?1
                   AND state NOT IN ('complete', 'no_suggestion', 'stale', 'failed')) = 0
           THEN 'review'
           ELSE 'running'
         END,
         updated_at_utc_ms = ?2
         WHERE id = ?1",
        params![input.job_id, input.now_utc_ms],
    )?;
    transaction.commit()?;
    get_recognition_job_by_id(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.job_id,
    )?
    .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn review_recognition_suggestion(
    connection: &mut Connection,
    input: ReviewCaptureRecognitionSuggestion,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if let Some(regions) = input.edited_regions.as_deref() {
        let confidence = regions
            .iter()
            .map(|region| region.confidence_basis_points)
            .min()
            .unwrap_or_default();
        validate_regions(regions, confidence)?;
    }
    let job_state = connection
        .query_row(
            "SELECT state FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.job_id, input.account_id, input.profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::JobNotFound)?;
    if job_state != "review" {
        return Err(CaptureRecognitionError::InvalidState);
    }
    let suggestion = connection
        .query_row(
            "SELECT state, review_band, regions_json
             FROM capture_recognition_suggestions
             WHERE id = ?1 AND job_id = ?2",
            params![input.suggestion_id, input.job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(CaptureRecognitionError::ItemNotFound)?;
    if suggestion.0 == "stale"
        || (input.decision == CaptureRecognitionDecision::Accepted && suggestion.1 == "low")
    {
        return Err(CaptureRecognitionError::InvalidSuggestion);
    }
    let regions_json = match input.edited_regions {
        Some(regions) => serde_json::to_string(&regions)?,
        None => suggestion.2,
    };
    connection.execute(
        "UPDATE capture_recognition_suggestions
         SET state = ?1, regions_json = ?2, reviewed_at_utc_ms = ?3
         WHERE id = ?4 AND job_id = ?5",
        params![
            match input.decision {
                CaptureRecognitionDecision::Accepted => "accepted",
                CaptureRecognitionDecision::Rejected => "rejected",
            },
            regions_json,
            input.now_utc_ms,
            input.suggestion_id,
            input.job_id,
        ],
    )?;
    connection.execute(
        "UPDATE capture_recognition_jobs SET updated_at_utc_ms = ?1 WHERE id = ?2",
        params![input.now_utc_ms, input.job_id],
    )?;
    get_recognition_job_by_id(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.job_id,
    )?
    .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn cancel_recognition_job(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    let changed = connection.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'cancelled', updated_at_utc_ms = ?1
         WHERE id = ?2 AND account_id = ?3 AND profile_id = ?4
           AND state IN ('queued', 'running')",
        params![now_utc_ms, job_id, account_id, profile_id],
    )?;
    if changed == 0 {
        let state = connection
            .query_row(
                "SELECT state FROM capture_recognition_jobs
                 WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
                params![job_id, account_id, profile_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(CaptureRecognitionError::JobNotFound)?;
        if state != "cancelled" {
            return Err(CaptureRecognitionError::InvalidState);
        }
    }
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn reset_abandoned_recognition_work(
    connection: &mut Connection,
    now_utc_ms: i64,
) -> Result<u32, CaptureRecognitionError> {
    let transaction = connection.transaction()?;
    // Pairing anchors are intentionally memory-only. Re-run every item in an
    // interrupted job so a restart cannot reuse an opaque slot for a different
    // question or leave a matching answer detached from its question.
    transaction.execute(
        "DELETE FROM capture_recognition_suggestions
         WHERE job_id IN (
           SELECT id FROM capture_recognition_jobs WHERE state = 'running'
         )",
        [],
    )?;
    let reset_items = transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = 'pending'
         WHERE state != 'pending'
           AND job_id IN (
             SELECT id FROM capture_recognition_jobs WHERE state = 'running'
           )",
        [],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'queued',
             processed_items = 0,
             updated_at_utc_ms = ?1
         WHERE state = 'running'",
        [now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(u32::try_from(reset_items).unwrap_or(u32::MAX))
}

pub fn claim_next_recognition_item(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    now_utc_ms: i64,
) -> Result<Option<ClaimedCaptureRecognitionItem>, CaptureRecognitionError> {
    let job_state = connection
        .query_row(
            "SELECT state FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![job_id, account_id, profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::JobNotFound)?;
    if !matches!(job_state.as_str(), "queued" | "running") {
        return Ok(None);
    }
    let transaction = connection.transaction()?;
    let row = transaction
        .query_row(
            "SELECT ji.item_id, j.batch_id, a.encrypted_path, a.media_type,
                    i.staged_role, ji.source_snapshot_hash
             FROM capture_recognition_job_items ji
             JOIN capture_recognition_jobs j ON j.id = ji.job_id
             JOIN capture_items i ON i.id = ji.item_id AND i.batch_id = j.batch_id
             JOIN assets a ON a.id = i.asset_id AND a.account_id = j.account_id
             WHERE ji.job_id = ?1 AND ji.state = 'pending'
               AND j.account_id = ?2 AND j.profile_id = ?3
             ORDER BY ji.position
             LIMIT 1",
            params![job_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((item_id, batch_id, encrypted_path, media_type, staged_role, snapshot)) = row else {
        transaction.execute(
            "UPDATE capture_recognition_jobs
             SET processed_items = (
               SELECT COUNT(*) FROM capture_recognition_job_items
               WHERE job_id = ?1
                 AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
             ),
             state = CASE
               WHEN EXISTS(
                 SELECT 1 FROM capture_recognition_job_items
                 WHERE job_id = ?1 AND state = 'running'
               ) THEN 'running'
               ELSE 'review'
             END,
             updated_at_utc_ms = ?2
             WHERE id = ?1 AND state IN ('queued', 'running')",
            params![job_id, now_utc_ms],
        )?;
        transaction.commit()?;
        return Ok(None);
    };
    let snapshot: [u8; 32] = snapshot
        .try_into()
        .map_err(|_| CaptureRecognitionError::InvalidInput)?;
    let role = match staged_role.as_str() {
        "answer" => CaptureRecognitionRole::Answer,
        "question" => CaptureRecognitionRole::Question,
        _ => return Err(CaptureRecognitionError::InvalidInput),
    };
    let changed = transaction.execute(
        "UPDATE capture_recognition_job_items SET state = 'running'
         WHERE job_id = ?1 AND item_id = ?2 AND state = 'pending'",
        params![job_id, item_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::InvalidState);
    }
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'running', updated_at_utc_ms = ?1
         WHERE id = ?2 AND state IN ('queued', 'running')",
        params![now_utc_ms, job_id],
    )?;
    transaction.commit()?;
    Ok(Some(ClaimedCaptureRecognitionItem {
        item_id,
        batch_id,
        encrypted_path,
        media_type,
        staged_role: role,
        source_snapshot_hash: snapshot,
    }))
}

pub fn finish_recognition_item_without_suggestion(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    item_id: &str,
    state: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if !matches!(state, "no_suggestion" | "stale" | "failed") {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = ?1
         WHERE job_id = ?2 AND item_id = ?3 AND state IN ('pending', 'running')",
        params![state, job_id, item_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::ItemNotFound);
    }
    let owned: bool = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM capture_recognition_jobs
           WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3
             AND state IN ('queued', 'running')
         )",
        params![job_id, account_id, profile_id],
        |row| row.get(0),
    )?;
    if !owned {
        return Err(CaptureRecognitionError::JobNotFound);
    }
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET processed_items = (
           SELECT COUNT(*) FROM capture_recognition_job_items
           WHERE job_id = ?1
             AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
         ),
         state = CASE
           WHEN EXISTS(
             SELECT 1 FROM capture_recognition_job_items
             WHERE job_id = ?1 AND state IN ('pending', 'running')
           ) THEN 'running'
           ELSE 'review'
         END,
         updated_at_utc_ms = ?2
         WHERE id = ?1",
        params![job_id, now_utc_ms],
    )?;
    transaction.commit()?;
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn fail_recognition_job(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    failure_code: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if failure_code.trim().is_empty() || failure_code.len() > 80 {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE capture_recognition_job_items SET state = 'failed'
         WHERE job_id = ?1 AND state IN ('pending', 'running')",
        [job_id],
    )?;
    let changed = transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'failed', processed_items = total_items,
             failure_code = ?1, updated_at_utc_ms = ?2
         WHERE id = ?3 AND account_id = ?4 AND profile_id = ?5
           AND state IN ('queued', 'running')",
        params![failure_code, now_utc_ms, job_id, account_id, profile_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::JobNotFound);
    }
    transaction.commit()?;
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

#[derive(Debug)]
struct RecognitionSource {
    item_id: String,
    asset_id: String,
    media_type: String,
    encrypted_path: String,
    source_name: String,
    snapshot: Vec<u8>,
}

#[derive(Debug)]
struct SelectedSuggestion {
    id: String,
    source: RecognitionSource,
    regions: Vec<CaptureRecognitionRegionProposal>,
}

#[derive(Debug)]
struct PreparedRegion {
    source_item_id: String,
    source_asset_id: String,
    source_name: String,
    role: CaptureRecognitionRole,
    confidence_basis_points: u16,
    position_in_source: usize,
    encoded: EncodedCrop,
}

#[derive(Debug)]
struct StagedRecognitionAsset {
    id: String,
    hash: String,
    media_type: String,
    byte_length: i64,
    relative: String,
    staged_path: PathBuf,
    final_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognitionOperationLedger {
    source_items: Vec<RecognitionLedgerSource>,
    created_items: Vec<RecognitionLedgerItem>,
    created_drafts: Vec<RecognitionLedgerDraft>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognitionLedgerSource {
    item_id: String,
    asset_id: String,
    superseded_by_derivation_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognitionLedgerItem {
    item_id: String,
    asset_id: String,
    derivation_id: String,
    source_sequence: i64,
    staged_role: String,
    draft_id: Option<String>,
    role: Option<String>,
    position: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognitionLedgerDraft {
    draft_id: String,
    position: i64,
}

pub fn apply_capture_recognition(
    connection: &mut Connection,
    input: ApplyCaptureRecognition,
) -> Result<CaptureRecognitionApplyReport, CaptureRecognitionError> {
    if input.accepted_suggestion_ids.is_empty()
        || input.accepted_suggestion_ids.len() > MAX_JOB_ITEMS
        || input
            .accepted_suggestion_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != input.accepted_suggestion_ids.len()
    {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let (batch_state, batch_revision): (String, i64) = connection
        .query_row(
            "SELECT state, revision FROM capture_batches
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.batch_id, input.account_id, input.profile_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::BatchNotFound)?;
    if batch_state != "organizing" {
        return Err(CaptureRecognitionError::InvalidState);
    }
    if u32::try_from(batch_revision).unwrap_or(u32::MAX) != input.expected_revision {
        return Err(CaptureRecognitionError::RevisionConflict);
    }
    let (job_state, engine, engine_version): (String, String, String) = connection
        .query_row(
            "SELECT state, engine, engine_version FROM capture_recognition_jobs
             WHERE id = ?1 AND batch_id = ?2 AND account_id = ?3 AND profile_id = ?4",
            params![
                input.job_id,
                input.batch_id,
                input.account_id,
                input.profile_id
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::JobNotFound)?;
    if job_state != "review" {
        return Err(CaptureRecognitionError::InvalidState);
    }

    let mut selected = Vec::with_capacity(input.accepted_suggestion_ids.len());
    let mut stale_ids = Vec::new();
    for suggestion_id in &input.accepted_suggestion_ids {
        let row = connection
            .query_row(
                "SELECT s.item_id, s.regions_json, ji.source_snapshot_hash,
                        i.asset_id, a.media_type, a.encrypted_path, i.source_name,
                        s.state, s.review_band
                 FROM capture_recognition_suggestions s
                 JOIN capture_recognition_job_items ji
                   ON ji.job_id = s.job_id AND ji.item_id = s.item_id
                 JOIN capture_items i ON i.id = s.item_id AND i.batch_id = ?3
                 JOIN assets a ON a.id = i.asset_id AND a.account_id = ?4
                 WHERE s.id = ?1 AND s.job_id = ?2",
                params![
                    suggestion_id,
                    input.job_id,
                    input.batch_id,
                    input.account_id
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()?
            .ok_or(CaptureRecognitionError::Stale)?;
        if row.7 != "accepted" || row.8 == "low" {
            return Err(CaptureRecognitionError::InvalidSuggestion);
        }
        let regions: Vec<CaptureRecognitionRegionProposal> = serde_json::from_str(&row.1)?;
        let minimum_confidence = regions
            .iter()
            .map(|region| region.confidence_basis_points)
            .min()
            .unwrap_or_default();
        validate_regions(&regions, minimum_confidence)?;
        let current_snapshot = capture_item_snapshot_hash(
            connection,
            &input.account_id,
            &input.profile_id,
            &input.batch_id,
            &row.0,
        );
        if current_snapshot
            .as_ref()
            .map(|hash| hash.as_slice() != row.2.as_slice())
            .unwrap_or(true)
        {
            stale_ids.push(suggestion_id.clone());
            continue;
        }
        selected.push(SelectedSuggestion {
            id: suggestion_id.clone(),
            source: RecognitionSource {
                item_id: row.0,
                asset_id: row.3,
                media_type: row.4,
                encrypted_path: row.5,
                source_name: row.6,
                snapshot: row.2,
            },
            regions,
        });
    }
    if selected.is_empty() {
        mark_recognition_suggestions_stale(
            connection,
            &input.job_id,
            &stale_ids,
            input.now_utc_ms,
        )?;
        return Err(CaptureRecognitionError::Stale);
    }

    let mut prepared = Vec::new();
    for suggestion in &selected {
        let plaintext = decrypt_asset(
            &read_encrypted_blob(&input.blob_root, &suggestion.source.encrypted_path)?,
            &input.asset_key,
        )
        .map_err(|_| CaptureRecognitionError::Crypto)?;
        let source_image = image::load_from_memory_with_format(
            &plaintext,
            image_format_for_media_type(&suggestion.source.media_type)?,
        )
        .map_err(|_| CaptureRecognitionError::Inbox(CaptureInboxError::InvalidImage))?;
        for (position, region) in suggestion.regions.iter().enumerate() {
            let output_media_type = if suggestion.source.media_type == "image/jpeg" {
                "image/jpeg"
            } else {
                "image/png"
            };
            let encoded = encode_crop(
                &source_image,
                &CaptureCropRecipe {
                    rect: region.rect.clone(),
                    rotation_degrees: 0,
                    output_media_type: output_media_type.to_owned(),
                    max_edge: 4096,
                    jpeg_quality: 90,
                },
            )?;
            prepared.push(PreparedRegion {
                source_item_id: suggestion.source.item_id.clone(),
                source_asset_id: suggestion.source.asset_id.clone(),
                source_name: suggestion.source.source_name.clone(),
                role: region.role,
                confidence_basis_points: region.confidence_basis_points,
                position_in_source: position,
                encoded,
            });
        }
    }

    let (active_count, stored_bytes): (i64, i64) = connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM capture_items
             WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL),
           (SELECT COALESCE(SUM(a.byte_length), 0)
              FROM capture_items i JOIN assets a ON a.id = i.asset_id
             WHERE i.batch_id = ?1)",
        [input.batch_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let resulting_count = active_count - i64::try_from(selected.len()).unwrap_or(i64::MAX)
        + i64::try_from(prepared.len()).unwrap_or(i64::MAX);
    let added_bytes = prepared.iter().fold(0_i64, |total, region| {
        total.saturating_add(i64::try_from(region.encoded.bytes.len()).unwrap_or(i64::MAX))
    });
    if resulting_count > MAX_CAPTURE_BATCH_ITEMS
        || stored_bytes.saturating_add(added_bytes) > MAX_CAPTURE_BATCH_BYTES
    {
        return Err(CaptureRecognitionError::CapacityReached);
    }
    if input.failure_point == Some(CaptureRecognitionFailurePoint::BeforeStaging) {
        return Err(CaptureRecognitionError::InjectedFailure);
    }

    let staging_root = input.blob_root.join(".staging");
    std::fs::create_dir_all(&staging_root)?;
    let mut asset_by_hash = HashMap::<String, String>::new();
    let mut staged_assets = Vec::<StagedRecognitionAsset>::new();
    let mut derived_asset_ids = Vec::with_capacity(prepared.len());
    let stage_result = (|| -> Result<(), CaptureRecognitionError> {
        for region in &prepared {
            let hash = plaintext_sha256(&region.encoded.bytes);
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
            let staged_path = staging_root.join(format!("{asset_id}.recognition.tmp"));
            let final_path = input.blob_root.join(&relative_path);
            let encrypted = encrypt_asset(&region.encoded.bytes, &input.asset_key)
                .map_err(|_| CaptureRecognitionError::Crypto)?;
            if let Err(error) = std::fs::write(&staged_path, encrypted) {
                let _ = std::fs::remove_file(&staged_path);
                return Err(error.into());
            }
            asset_by_hash.insert(hash.clone(), asset_id.clone());
            derived_asset_ids.push(asset_id.clone());
            staged_assets.push(StagedRecognitionAsset {
                id: asset_id,
                hash,
                media_type: region.encoded.media_type.clone(),
                byte_length: i64::try_from(region.encoded.bytes.len()).unwrap_or(i64::MAX),
                relative: relative_path.to_string_lossy().replace('\\', "/"),
                staged_path,
                final_path,
            });
        }
        Ok(())
    })();
    if let Err(error) = stage_result {
        cleanup_staged_recognition_assets(&staged_assets, false);
        return Err(error);
    }
    if input.failure_point == Some(CaptureRecognitionFailurePoint::AfterStaging) {
        cleanup_staged_recognition_assets(&staged_assets, false);
        return Err(CaptureRecognitionError::InjectedFailure);
    }

    let operation_id = Uuid::now_v7().to_string();
    let item_ids = (0..prepared.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let derivation_ids = (0..prepared.len())
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();
    let selected_ids = selected
        .iter()
        .map(|suggestion| suggestion.id.as_str())
        .collect::<BTreeSet<_>>();
    let transaction = connection.transaction()?;
    let persist_result = (|| -> Result<RecognitionOperationLedger, CaptureRecognitionError> {
        let current_revision: i64 = transaction
            .query_row(
                "SELECT revision FROM capture_batches
                 WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND state = 'organizing'",
                params![input.batch_id, input.account_id, input.profile_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(CaptureRecognitionError::InvalidState)?;
        if u32::try_from(current_revision).unwrap_or(u32::MAX) != input.expected_revision {
            return Err(CaptureRecognitionError::RevisionConflict);
        }
        for suggestion in &selected {
            let current = capture_item_snapshot_hash(
                &transaction,
                &input.account_id,
                &input.profile_id,
                &input.batch_id,
                &suggestion.source.item_id,
            )?;
            if current.as_slice() != suggestion.source.snapshot.as_slice() {
                return Err(CaptureRecognitionError::Stale);
            }
        }
        for asset in &staged_assets {
            transaction.execute(
                "INSERT INTO assets(
                   id, account_id, plaintext_sha256, encrypted_path, byte_length,
                   media_type, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    asset.id,
                    input.account_id,
                    asset.hash,
                    asset.relative,
                    asset.byte_length,
                    asset.media_type,
                    input.now_utc_ms
                ],
            )?;
            if let Some(parent) = asset.final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&asset.staged_path, &asset.final_path)?;
        }
        if input.failure_point == Some(CaptureRecognitionFailurePoint::InTransaction) {
            return Err(CaptureRecognitionError::InjectedFailure);
        }

        if !stale_ids.is_empty() {
            for suggestion_id in &stale_ids {
                transaction.execute(
                    "UPDATE capture_recognition_suggestions SET state = 'stale'
                     WHERE id = ?1 AND job_id = ?2",
                    params![suggestion_id, input.job_id],
                )?;
                transaction.execute(
                    "UPDATE capture_recognition_job_items SET state = 'stale'
                     WHERE job_id = ?1 AND item_id = (
                       SELECT item_id FROM capture_recognition_suggestions WHERE id = ?2
                     )",
                    params![input.job_id, suggestion_id],
                )?;
            }
        }

        let max_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(source_sequence), -1) FROM capture_items WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?;
        let mut ledger_items = Vec::with_capacity(prepared.len());
        let mut first_derivation_by_source = HashMap::<String, String>::new();
        for (index, region) in prepared.iter().enumerate() {
            let item_id = &item_ids[index];
            let derivation_id = &derivation_ids[index];
            let role = match region.role {
                CaptureRecognitionRole::Question => "question",
                CaptureRecognitionRole::Answer => "answer",
            };
            let source_sequence = max_sequence + 1 + i64::try_from(index).unwrap_or(i64::MAX);
            let extension = if region.encoded.media_type == "image/jpeg" {
                "jpg"
            } else {
                "png"
            };
            let base_name = Path::new(&region.source_name)
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("image");
            let source_name = format!(
                "{base_name}-smart-{}.{}",
                region.position_in_source + 1,
                extension
            )
            .chars()
            .take(255)
            .collect::<String>();
            transaction.execute(
                "INSERT INTO capture_items(
                   id, batch_id, asset_id, client_upload_id, source_name, source_sequence,
                   width, height, created_at_utc_ms, staged_role
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    item_id,
                    input.batch_id,
                    derived_asset_ids[index],
                    format!("recognition:{operation_id}:{index}"),
                    source_name,
                    source_sequence,
                    region.encoded.width,
                    region.encoded.height,
                    input.now_utc_ms,
                    role
                ],
            )?;
            transaction.execute(
                "INSERT INTO asset_derivations(
                   id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                   source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                   engine, engine_version, confidence, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'crop', ?10, ?11, ?12, ?13, ?14)",
                params![
                    derivation_id,
                    operation_id,
                    input.account_id,
                    input.batch_id,
                    region.source_asset_id,
                    derived_asset_ids[index],
                    region.source_item_id,
                    item_id,
                    i64::try_from(region.position_in_source).unwrap_or(i64::MAX),
                    region.encoded.recipe_json,
                    engine,
                    engine_version,
                    f64::from(region.confidence_basis_points) / 10_000.0,
                    input.now_utc_ms
                ],
            )?;
            first_derivation_by_source
                .entry(region.source_item_id.clone())
                .or_insert_with(|| derivation_id.clone());

            ledger_items.push(RecognitionLedgerItem {
                item_id: item_id.clone(),
                asset_id: derived_asset_ids[index].clone(),
                derivation_id: derivation_id.clone(),
                source_sequence,
                staged_role: role.to_owned(),
                draft_id: None,
                role: None,
                position: None,
            });
        }

        let mut ledger_sources = Vec::with_capacity(selected.len());
        for suggestion in &selected {
            let derivation_id = first_derivation_by_source
                .get(&suggestion.source.item_id)
                .ok_or(CaptureRecognitionError::InvalidSuggestion)?;
            let changed = transaction.execute(
                "UPDATE capture_items SET superseded_by_derivation_id = ?1
                 WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NULL
                   AND NOT EXISTS(
                     SELECT 1 FROM capture_draft_items WHERE item_id = ?2
                   )",
                params![derivation_id, suggestion.source.item_id, input.batch_id],
            )?;
            if changed != 1 {
                return Err(CaptureRecognitionError::Stale);
            }
            transaction.execute(
                "INSERT INTO capture_source_retention(
                   batch_id, source_asset_id, retain_until_utc_ms, reason, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, 'crop_recovery', ?4)
                 ON CONFLICT(batch_id, source_asset_id) DO UPDATE SET
                   retain_until_utc_ms = excluded.retain_until_utc_ms",
                params![
                    input.batch_id,
                    suggestion.source.asset_id,
                    input.now_utc_ms.saturating_add(30 * 24 * 60 * 60 * 1_000),
                    input.now_utc_ms
                ],
            )?;
            ledger_sources.push(RecognitionLedgerSource {
                item_id: suggestion.source.item_id.clone(),
                asset_id: suggestion.source.asset_id.clone(),
                superseded_by_derivation_id: derivation_id.clone(),
            });
        }
        let ledger = RecognitionOperationLedger {
            source_items: ledger_sources,
            created_items: ledger_items,
            // New visual-splitting operations never create cards. Keep this
            // ledger field so operations written by earlier builds can still
            // be reverted without losing their draft restoration contract.
            created_drafts: Vec::new(),
        };
        transaction.execute(
            "INSERT INTO capture_recognition_operations(
               id, job_id, batch_id, before_revision, after_revision,
               created_entity_ids_json, created_at_utc_ms, reverted_at_utc_ms
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                operation_id,
                input.job_id,
                input.batch_id,
                input.expected_revision,
                input.expected_revision.saturating_add(1),
                serde_json::to_string(&ledger)?,
                input.now_utc_ms
            ],
        )?;
        transaction.execute(
            "UPDATE capture_recognition_jobs
             SET state = 'applied', updated_at_utc_ms = ?1 WHERE id = ?2",
            params![input.now_utc_ms, input.job_id],
        )?;
        transaction.execute(
            "UPDATE capture_batches
             SET revision = revision + 1, updated_at_utc_ms = ?1
             WHERE id = ?2",
            params![input.now_utc_ms, input.batch_id],
        )?;
        transaction.commit()?;
        Ok(ledger)
    })();
    let ledger = match persist_result {
        Ok(ledger) => ledger,
        Err(error) => {
            cleanup_staged_recognition_assets(&staged_assets, true);
            return Err(error);
        }
    };

    let detail = get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    Ok(CaptureRecognitionApplyReport {
        operation_id,
        applied_suggestion_count: u32::try_from(selected_ids.len()).unwrap_or(u32::MAX),
        created_draft_count: u32::try_from(ledger.created_drafts.len()).unwrap_or(u32::MAX),
        created_item_count: u32::try_from(ledger.created_items.len()).unwrap_or(u32::MAX),
        // Pairing belongs to the future full-recognition mode. The current
        // splitter places every crop in the material library.
        unmatched_answer_count: 0,
        stale_suggestion_count: u32::try_from(stale_ids.len()).unwrap_or(u32::MAX),
        detail,
    })
}

pub fn revert_capture_recognition(
    connection: &mut Connection,
    input: RevertCaptureRecognition,
) -> Result<CaptureRecognitionRevertReport, CaptureRecognitionError> {
    let operation = connection
        .query_row(
            "SELECT o.job_id, o.after_revision, o.created_entity_ids_json,
                    o.reverted_at_utc_ms
             FROM capture_recognition_operations o
             JOIN capture_recognition_jobs j ON j.id = o.job_id
             WHERE o.id = ?1 AND o.batch_id = ?2
               AND j.account_id = ?3 AND j.profile_id = ?4",
            params![
                input.operation_id,
                input.batch_id,
                input.account_id,
                input.profile_id
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(CaptureRecognitionError::RevertConflict)?;
    if operation.3.is_some() {
        return Err(CaptureRecognitionError::RevertConflict);
    }
    let ledger: RecognitionOperationLedger = serde_json::from_str(&operation.2)?;
    let current_revision: i64 = connection
        .query_row(
            "SELECT revision FROM capture_batches
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND state = 'organizing'",
            params![input.batch_id, input.account_id, input.profile_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::RevertConflict)?;
    if u32::try_from(current_revision).unwrap_or(u32::MAX) != input.expected_revision
        || current_revision != operation.1
    {
        return Err(CaptureRecognitionError::RevertConflict);
    }
    validate_recognition_revert_state(connection, &input.batch_id, &ledger)?;

    let transaction = connection.transaction()?;
    validate_recognition_revert_state(&transaction, &input.batch_id, &ledger)?;
    let mut orphan_paths = Vec::new();
    for item in &ledger.created_items {
        transaction.execute(
            "DELETE FROM asset_derivations
             WHERE id = ?1 AND operation_id = ?2 AND derived_capture_item_id = ?3",
            params![item.derivation_id, input.operation_id, item.item_id],
        )?;
    }
    for item in &ledger.created_items {
        transaction.execute(
            "DELETE FROM capture_items WHERE id = ?1 AND batch_id = ?2",
            params![item.item_id, input.batch_id],
        )?;
    }
    for draft in &ledger.created_drafts {
        transaction.execute(
            "DELETE FROM capture_drafts WHERE id = ?1 AND batch_id = ?2",
            params![draft.draft_id, input.batch_id],
        )?;
    }
    for source in &ledger.source_items {
        let restored = transaction.execute(
            "UPDATE capture_items SET superseded_by_derivation_id = NULL
             WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id = ?3",
            params![
                source.item_id,
                input.batch_id,
                source.superseded_by_derivation_id
            ],
        )?;
        if restored != 1 {
            return Err(CaptureRecognitionError::RevertConflict);
        }
        transaction.execute(
            "DELETE FROM capture_source_retention
             WHERE batch_id = ?1 AND source_asset_id = ?2
               AND NOT EXISTS(
                 SELECT 1 FROM capture_items
                  WHERE batch_id = ?1 AND asset_id = ?2
                    AND superseded_by_derivation_id IS NOT NULL
               )",
            params![input.batch_id, source.asset_id],
        )?;
    }
    let mut seen_assets = BTreeSet::new();
    for item in &ledger.created_items {
        if !seen_assets.insert(item.asset_id.clone()) {
            continue;
        }
        let encrypted_path = transaction
            .query_row(
                "SELECT encrypted_path FROM assets WHERE id = ?1 AND account_id = ?2",
                params![item.asset_id, input.account_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(encrypted_path) = encrypted_path else {
            continue;
        };
        let referenced: bool = transaction.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM capture_items WHERE asset_id = ?1
               UNION ALL SELECT 1 FROM problem_assets WHERE asset_id = ?1
             )",
            [item.asset_id.as_str()],
            |row| row.get(0),
        )?;
        if !referenced
            && transaction.execute("DELETE FROM assets WHERE id = ?1", [item.asset_id.as_str()])?
                == 1
        {
            orphan_paths.push(encrypted_path);
        }
    }
    transaction.execute(
        "UPDATE capture_recognition_operations SET reverted_at_utc_ms = ?1
         WHERE id = ?2 AND reverted_at_utc_ms IS NULL",
        params![input.now_utc_ms, input.operation_id],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'cancelled', updated_at_utc_ms = ?1 WHERE id = ?2 AND state = 'applied'",
        params![input.now_utc_ms, operation.0],
    )?;
    transaction.execute(
        "UPDATE capture_batches SET revision = revision + 1, updated_at_utc_ms = ?1
         WHERE id = ?2",
        params![input.now_utc_ms, input.batch_id],
    )?;
    transaction.commit()?;
    for path in orphan_paths {
        let _ = remove_encrypted_blob(&input.blob_root, &path);
    }
    let detail = get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    Ok(CaptureRecognitionRevertReport {
        operation_id: input.operation_id,
        reverted_item_count: u32::try_from(ledger.created_items.len()).unwrap_or(u32::MAX),
        detail,
    })
}

pub fn latest_capture_recognition_operation(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<Option<CaptureRecognitionOperationSummary>, CaptureRecognitionError> {
    let row = connection
        .query_row(
            "SELECT o.id, o.batch_id, o.after_revision, o.created_entity_ids_json,
                    o.reverted_at_utc_ms
             FROM capture_recognition_operations o
             JOIN capture_recognition_jobs j ON j.id = o.job_id
             WHERE o.batch_id = ?1 AND j.account_id = ?2 AND j.profile_id = ?3
             ORDER BY o.created_at_utc_ms DESC, o.id DESC LIMIT 1",
            params![batch_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((operation_id, batch_id, after_revision, ledger_json, reverted_at)) = row else {
        return Ok(None);
    };
    let ledger: RecognitionOperationLedger = serde_json::from_str(&ledger_json)?;
    Ok(Some(CaptureRecognitionOperationSummary {
        operation_id,
        batch_id,
        after_revision: u32::try_from(after_revision).unwrap_or(u32::MAX),
        created_item_count: u32::try_from(ledger.created_items.len()).unwrap_or(u32::MAX),
        reverted: reverted_at.is_some(),
    }))
}

fn validate_recognition_revert_state(
    connection: &Connection,
    batch_id: &str,
    ledger: &RecognitionOperationLedger,
) -> Result<(), CaptureRecognitionError> {
    for source in &ledger.source_items {
        let current = connection
            .query_row(
                "SELECT asset_id, superseded_by_derivation_id FROM capture_items
                 WHERE id = ?1 AND batch_id = ?2",
                params![source.item_id, batch_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        if current
            != Some((
                source.asset_id.clone(),
                Some(source.superseded_by_derivation_id.clone()),
            ))
        {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    for item in &ledger.created_items {
        let current = connection
            .query_row(
                "SELECT i.asset_id, i.source_sequence, i.staged_role,
                        i.superseded_by_derivation_id, di.draft_id, di.role, di.position
                 FROM capture_items i
                 LEFT JOIN capture_draft_items di ON di.item_id = i.id
                 WHERE i.id = ?1 AND i.batch_id = ?2",
                params![item.item_id, batch_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()?;
        if current
            != Some((
                item.asset_id.clone(),
                item.source_sequence,
                item.staged_role.clone(),
                None,
                item.draft_id.clone(),
                item.role.clone(),
                item.position,
            ))
        {
            return Err(CaptureRecognitionError::RevertConflict);
        }
        let referenced_or_derived: bool = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM problem_assets WHERE asset_id = ?1
               UNION ALL
               SELECT 1 FROM asset_derivations
                WHERE source_capture_item_id = ?2 AND id <> ?3
             )",
            params![item.asset_id, item.item_id, item.derivation_id],
            |row| row.get(0),
        )?;
        if referenced_or_derived {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    for draft in &ledger.created_drafts {
        let current = connection
            .query_row(
                "SELECT position, subject_override, tags_json, note FROM capture_drafts
                 WHERE id = ?1 AND batch_id = ?2",
                params![draft.draft_id, batch_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        if current != Some((draft.position, None, "[]".to_owned(), String::new())) {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    Ok(())
}

fn mark_recognition_suggestions_stale(
    connection: &mut Connection,
    job_id: &str,
    suggestion_ids: &[String],
    now_utc_ms: i64,
) -> Result<(), CaptureRecognitionError> {
    let transaction = connection.transaction()?;
    for suggestion_id in suggestion_ids {
        transaction.execute(
            "UPDATE capture_recognition_suggestions SET state = 'stale'
             WHERE id = ?1 AND job_id = ?2",
            params![suggestion_id, job_id],
        )?;
        transaction.execute(
            "UPDATE capture_recognition_job_items SET state = 'stale'
             WHERE job_id = ?1 AND item_id = (
               SELECT item_id FROM capture_recognition_suggestions WHERE id = ?2
             )",
            params![job_id, suggestion_id],
        )?;
    }
    transaction.execute(
        "UPDATE capture_recognition_jobs SET updated_at_utc_ms = ?1 WHERE id = ?2",
        params![now_utc_ms, job_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn cleanup_staged_recognition_assets(assets: &[StagedRecognitionAsset], include_final: bool) {
    for asset in assets {
        let _ = std::fs::remove_file(&asset.staged_path);
        if include_final {
            let _ = std::fs::remove_file(&asset.final_path);
        }
    }
}

pub fn capture_item_snapshot_hash(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<[u8; 32], CaptureRecognitionError> {
    let item = connection
        .query_row(
            "SELECT i.asset_id, i.staged_role, i.superseded_by_derivation_id,
                    di.draft_id, di.role, di.position
             FROM capture_items i
             JOIN capture_batches b ON b.id = i.batch_id
             LEFT JOIN capture_draft_items di ON di.item_id = i.id
             WHERE i.id = ?1 AND i.batch_id = ?2
               AND b.account_id = ?3 AND b.profile_id = ?4",
            params![item_id, batch_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or(CaptureRecognitionError::ItemNotFound)?;
    if item.2.is_some() || item.3.is_some() {
        return Err(CaptureRecognitionError::ItemNotFound);
    }
    let mut hash = Sha256::new();
    for value in [
        item.0,
        item.1,
        item.2.unwrap_or_default(),
        item.3.unwrap_or_default(),
        item.4.unwrap_or_default(),
        item.5.map(|value| value.to_string()).unwrap_or_default(),
    ] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    Ok(hash.finalize().into())
}

pub const fn review_band(confidence: u16) -> CaptureRecognitionReviewBand {
    match confidence {
        9000..=10000 => CaptureRecognitionReviewBand::High,
        6500..=8999 => CaptureRecognitionReviewBand::Review,
        _ => CaptureRecognitionReviewBand::Low,
    }
}

fn validate_regions(
    regions: &[CaptureRecognitionRegionProposal],
    confidence_basis_points: u16,
) -> Result<(), CaptureRecognitionError> {
    if regions.is_empty()
        || regions.len() > MAX_REGIONS_PER_ITEM
        || confidence_basis_points > 10_000
    {
        return Err(CaptureRecognitionError::InvalidSuggestion);
    }
    for region in regions {
        let rect = &region.rect;
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
            || rect.x < 0.0
            || rect.y < 0.0
            || rect.width <= 0.0
            || rect.height <= 0.0
            || rect.x + rect.width > 1.0
            || rect.y + rect.height > 1.0
            || region.confidence_basis_points > 10_000
            || region
                .group_slot
                .is_some_and(|slot| slot >= MAX_JOB_ITEMS as u32)
        {
            return Err(CaptureRecognitionError::InvalidSuggestion);
        }
    }
    Ok(())
}

fn list_suggestions(
    connection: &Connection,
    job_id: &str,
) -> Result<Vec<CaptureRecognitionSuggestion>, CaptureRecognitionError> {
    let mut statement = connection.prepare(
        "SELECT id, item_id, regions_json, confidence_basis_points,
                review_band, state, reason_codes_json
         FROM capture_recognition_suggestions
         WHERE job_id = ?1
         ORDER BY rowid",
    )?;
    let rows = statement.query_map([job_id], |row| {
        let regions_json = row.get::<_, String>(2)?;
        let reasons_json = row.get::<_, String>(6)?;
        let confidence = row.get::<_, i64>(3)?;
        Ok(CaptureRecognitionSuggestion {
            id: row.get(0)?,
            item_id: row.get(1)?,
            regions: serde_json::from_str(&regions_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            confidence_basis_points: u16::try_from(confidence).unwrap_or(u16::MAX),
            review_band: CaptureRecognitionReviewBand::from_database(&row.get::<_, String>(4)?)?,
            state: CaptureRecognitionSuggestionState::from_database(&row.get::<_, String>(5)?)?,
            reason_codes: serde_json::from_str(&reasons_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CaptureRecognitionError::from)
}
