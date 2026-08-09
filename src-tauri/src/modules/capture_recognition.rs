use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use specta::Type;
use thiserror::Error;

use crate::modules::capture_inbox::{
    CaptureBatchDetail, CaptureInboxError, MAX_CAPTURE_BATCH_ITEMS, NormalizedCropRect,
};

#[path = "capture_recognition_job.rs"]
mod capture_recognition_job;
#[path = "capture_recognition_operation_ledger.rs"]
mod capture_recognition_operation_ledger;
#[path = "capture_recognition_revert.rs"]
mod capture_recognition_revert;
#[path = "capture_recognition_transaction.rs"]
mod capture_recognition_transaction;

pub use capture_recognition_job::{
    cancel_recognition_job, claim_next_recognition_item, create_or_resume_recognition_job,
    fail_recognition_job, finish_recognition_item_without_suggestion, get_active_recognition_job,
    get_recognition_job_by_id, reset_abandoned_recognition_work, review_recognition_suggestion,
    store_recognition_suggestion,
};
pub use capture_recognition_revert::{
    latest_capture_recognition_operation, revert_capture_recognition,
};
pub use capture_recognition_transaction::apply_capture_recognition;

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
    pub pair_suggestion_count: u32,
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
