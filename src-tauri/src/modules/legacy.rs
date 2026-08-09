use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::infrastructure::assets::AssetCryptoError;

#[path = "legacy_import_transaction.rs"]
mod legacy_import_transaction;
#[path = "legacy_rollback_transaction.rs"]
mod legacy_rollback_transaction;
#[path = "legacy_scan.rs"]
mod legacy_scan;

pub use legacy_import_transaction::import_legacy_plan;
pub use legacy_rollback_transaction::rollback_legacy_import;
pub use legacy_scan::{
    LegacyIssue, LegacyScanError, LegacyScanReport, build_legacy_import_plan,
    legacy_tree_fingerprint, scan_legacy_storage,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyRating {
    Good,
    Again,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyReviewPlan {
    pub occurred_at_utc_ms: i64,
    pub rating: LegacyRating,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyAssetPlan {
    pub source_record_id: String,
    pub source_path: PathBuf,
    pub media_type: String,
    pub plaintext_sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyProblemPlan {
    pub source_problem_key: String,
    pub subject: String,
    pub tags: Vec<String>,
    pub note: String,
    pub time_limit_seconds: Option<i32>,
    pub question_assets: Vec<LegacyAssetPlan>,
    pub answer_assets: Vec<LegacyAssetPlan>,
    pub reviews: Vec<LegacyReviewPlan>,
    pub due_at_utc_ms: Option<i64>,
    pub stability_days: f64,
    pub difficulty: f64,
    pub frozen: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyMemberPlan {
    pub source_member_key: String,
    pub name: String,
    pub problems: Vec<LegacyProblemPlan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyImportPlan {
    pub source_root: PathBuf,
    pub source_fingerprint: String,
    pub report: LegacyScanReport,
    pub members: Vec<LegacyMemberPlan>,
}

impl LegacyImportPlan {
    pub fn public_report(&self) -> LegacyScanReport {
        self.report.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LegacyImportPhase {
    Validating,
    Encrypting,
    Writing,
    Verifying,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportProgress {
    pub candidate_id: String,
    pub phase: LegacyImportPhase,
    pub completed: i32,
    pub total: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportReceipt {
    pub import_id: String,
    pub member_count: i32,
    pub problem_count: i32,
    pub asset_count: i32,
    pub review_count: i32,
    pub frozen_problem_count: i32,
    pub created_at_utc_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRollbackReceipt {
    pub import_id: String,
    pub removed_problem_count: i32,
    pub removed_profile_count: i32,
    pub removed_asset_count: i32,
    pub preserved_entity_count: i32,
    pub rolled_back_at_utc_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportCandidate {
    pub candidate_id: String,
    pub report: LegacyScanReport,
    pub problem_count: i32,
    pub expires_at_utc_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportSummary {
    pub import_id: String,
    pub member_count: i32,
    pub problem_count: i32,
    pub asset_count: i32,
    pub review_count: i32,
    pub status: String,
    pub created_at_utc_ms: f64,
    pub rolled_back_at_utc_ms: Option<f64>,
}

#[derive(Clone)]
pub struct LegacyImportManager {
    candidate: Arc<Mutex<Option<PreparedLegacyCandidate>>>,
}

#[derive(Clone)]
struct PreparedLegacyCandidate {
    public: LegacyImportCandidate,
    plan: LegacyImportPlan,
}

impl Default for LegacyImportManager {
    fn default() -> Self {
        Self {
            candidate: Arc::new(Mutex::new(None)),
        }
    }
}

impl LegacyImportManager {
    pub fn prepare(
        &self,
        root: &Path,
        now_utc_ms: i64,
    ) -> Result<LegacyImportCandidate, LegacyImportError> {
        let plan = build_legacy_import_plan(root)?;
        let problem_count = plan
            .members
            .iter()
            .map(|member| member.problems.len())
            .sum::<usize>();
        let public = LegacyImportCandidate {
            candidate_id: Uuid::now_v7().to_string(),
            report: plan.public_report(),
            problem_count: i32::try_from(problem_count).unwrap_or(i32::MAX),
            expires_at_utc_ms: now_utc_ms.saturating_add(30 * 60 * 1_000) as f64,
        };
        *self
            .candidate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(PreparedLegacyCandidate {
            public: public.clone(),
            plan,
        });
        Ok(public)
    }

    pub fn plan_for(
        &self,
        candidate_id: &str,
        now_utc_ms: i64,
    ) -> Result<LegacyImportPlan, LegacyImportError> {
        let mut slot = self
            .candidate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(candidate) = slot.as_ref() else {
            return Err(LegacyImportError::ImportNotFound);
        };
        if candidate.public.candidate_id != candidate_id {
            return Err(LegacyImportError::ImportNotFound);
        }
        if candidate.public.expires_at_utc_ms <= now_utc_ms as f64 {
            *slot = None;
            return Err(LegacyImportError::ImportNotFound);
        }
        Ok(candidate.plan.clone())
    }

    pub fn consume(&self, candidate_id: &str) {
        let mut slot = self
            .candidate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot
            .as_ref()
            .is_some_and(|candidate| candidate.public.candidate_id == candidate_id)
        {
            *slot = None;
        }
    }
}

pub fn list_legacy_imports(
    connection: &Connection,
    account_id: &str,
) -> Result<Vec<LegacyImportSummary>, LegacyImportError> {
    let mut statement = connection.prepare(
        "SELECT id, member_count, problem_count, asset_count, review_count, status,
                created_at_utc_ms, rolled_back_at_utc_ms
         FROM legacy_imports WHERE account_id = ?1
         ORDER BY created_at_utc_ms DESC, id DESC LIMIT 100",
    )?;
    Ok(statement
        .query_map([account_id], |row| {
            Ok(LegacyImportSummary {
                import_id: row.get(0)?,
                member_count: row.get(1)?,
                problem_count: row.get(2)?,
                asset_count: row.get(3)?,
                review_count: row.get(4)?,
                status: row.get(5)?,
                created_at_utc_ms: row.get::<_, i64>(6)? as f64,
                rolled_back_at_utc_ms: row.get::<_, Option<i64>>(7)?.map(|value| value as f64),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

#[derive(Debug, Error)]
pub enum LegacyImportError {
    #[error("legacy import source is not safe to import")]
    UnsafeSource,
    #[error("legacy import source changed during import")]
    SourceChanged,
    #[error("legacy import contains an invalid image")]
    InvalidImage,
    #[error("legacy import was already completed")]
    AlreadyImported,
    #[error("legacy import was not found")]
    ImportNotFound,
    #[error("legacy import file operation failed")]
    Io(#[from] io::Error),
    #[error("legacy import database operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("legacy import serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("legacy import encryption failed")]
    Crypto(#[from] AssetCryptoError),
    #[error("legacy import scan failed")]
    Scan(#[from] LegacyScanError),
}
