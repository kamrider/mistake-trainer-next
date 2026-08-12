use std::io;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[path = "backup_creation.rs"]
mod backup_creation;
#[path = "backup_package_repository.rs"]
mod backup_package_repository;
#[path = "backup_restore.rs"]
mod backup_restore;
#[path = "backup_restore_repository.rs"]
mod backup_restore_repository;
#[path = "backup_schema_validation.rs"]
mod backup_schema_validation;
#[path = "backup_validation.rs"]
mod backup_validation;

pub use backup_creation::create_backup;
pub use backup_restore::{
    RestoreSwap, begin_pending_restore, prepare_backup_restore, record_failed_restore,
    schedule_backup_restore, schedule_backup_restore_with_mode, take_restore_receipt,
    validate_restore_candidate,
};
pub use backup_restore_repository::RestoreMode;
pub use backup_validation::validate_backup;

const FORMAT_VERSION: i32 = 1;
const CURRENT_SCHEMA_VERSION: i64 = 18;
const DATABASE_FILE: &str = "library.db";
const MANIFEST_FILE: &str = "manifest.json";
const ASSETS_DIRECTORY: &str = "assets";
const MAX_DATABASE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 65 * 1024 * 1024;
const MAX_TOTAL_ASSET_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ASSETS: usize = 50_000;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
pub const RESTORE_CANDIDATE_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub format_version: i32,
    pub created_at_utc_ms: f64,
    pub asset_count: i32,
    pub encrypted_bytes: f64,
    pub label: String,
    pub ready_for_restore: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreCandidate {
    pub id: String,
    pub summary: BackupSummary,
    pub expires_at_utc_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreReceipt {
    pub status: String,
    pub label: String,
    pub finished_at_utc_ms: f64,
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("backup destination is invalid")]
    InvalidDestination,
    #[error("backup package is invalid")]
    InvalidPackage,
    #[error("backup belongs to another local account")]
    AccountMismatch,
    #[error("local library contains foreign account data")]
    ForeignAccountData,
    #[error("backup schema is unsupported")]
    UnsupportedSchema,
    #[error("backup exceeds the safety budget")]
    TooLarge,
    #[error("backup integrity check failed")]
    Integrity,
    #[error("prepared restore candidate has expired")]
    ExpiredCandidate,
    #[error("another restore is already pending")]
    RestorePending,
    #[error("local library is busy")]
    Lock,
    #[error("backup file operation failed")]
    Io(#[from] io::Error),
    #[error("backup database operation failed")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format_version: i32,
    created_at_utc_ms: i64,
    schema_version: i64,
    account_hash: String,
    database: ManifestFile,
    assets: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    relative_path: String,
    encrypted_bytes: u64,
    ciphertext_sha256: String,
}
