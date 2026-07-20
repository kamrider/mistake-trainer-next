use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::infrastructure::assets::{AssetCryptoError, encrypt_asset};

const MAX_MEMBERS: usize = 512;
const MAX_DIRECTORY_ENTRIES: usize = 2_048;
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const MAX_ISSUES: usize = 10_000;
const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_LABEL_CHARS: usize = 160;
const MAX_TAGS: usize = 32;
const MAX_TAG_CHARS: usize = 40;
const MAX_NOTE_CHARS: usize = 20_000;

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

struct StagedLegacyAsset {
    id: String,
    plaintext_sha256: String,
    media_type: String,
    byte_length: i64,
    encrypted_path: String,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
}

pub fn import_legacy_plan(
    connection: &mut Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    plan: LegacyImportPlan,
    now_utc_ms: i64,
    mut progress: impl FnMut(LegacyImportProgress),
) -> Result<LegacyImportReceipt, LegacyImportError> {
    if plan.report.truncated || plan.members.is_empty() {
        return Err(LegacyImportError::UnsafeSource);
    }
    progress(LegacyImportProgress {
        phase: LegacyImportPhase::Validating,
        completed: 0,
        total: 1,
    });
    if legacy_tree_fingerprint(&plan.source_root)? != plan.source_fingerprint {
        return Err(LegacyImportError::SourceChanged);
    }
    let already_imported: bool = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM legacy_imports
           WHERE account_id = ?1 AND source_fingerprint = ?2 AND status = 'completed'
         )",
        params![account_id, plan.source_fingerprint],
        |row| row.get(0),
    )?;
    if already_imported {
        return Err(LegacyImportError::AlreadyImported);
    }

    let import_id = Uuid::now_v7().to_string();
    let staging_root = blob_root.join(".legacy-import").join(&import_id);
    let mut unique_assets = BTreeMap::<String, &LegacyAssetPlan>::new();
    let mut problem_count = 0_usize;
    let mut review_count = 0_usize;
    let mut frozen_problem_count = 0_usize;
    for member in &plan.members {
        problem_count = problem_count.saturating_add(member.problems.len());
        for problem in &member.problems {
            review_count = review_count.saturating_add(problem.reviews.len());
            frozen_problem_count = frozen_problem_count.saturating_add(usize::from(problem.frozen));
            for asset in problem.question_assets.iter().chain(&problem.answer_assets) {
                unique_assets
                    .entry(asset.plaintext_sha256.clone())
                    .or_insert(asset);
            }
        }
    }
    if problem_count == 0 {
        return Err(LegacyImportError::UnsafeSource);
    }

    let asset_total = i32::try_from(unique_assets.len()).unwrap_or(i32::MAX);
    let mut asset_ids = HashMap::<String, (String, bool)>::new();
    let mut staged_assets = Vec::<StagedLegacyAsset>::new();
    let stage_result = (|| -> Result<(), LegacyImportError> {
        for (index, (hash, source)) in unique_assets.into_iter().enumerate() {
            progress(LegacyImportProgress {
                phase: LegacyImportPhase::Encrypting,
                completed: i32::try_from(index).unwrap_or(i32::MAX),
                total: asset_total,
            });
            if let Some(id) = connection
                .query_row(
                    "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
                    params![account_id, hash],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                asset_ids.insert(hash, (id, false));
                continue;
            }
            let bytes = read_bounded(&source.source_path, MAX_ASSET_BYTES)
                .map_err(|_| LegacyImportError::InvalidImage)?;
            if plaintext_digest(&bytes) != hash {
                return Err(LegacyImportError::SourceChanged);
            }
            validate_import_image(&bytes, &source.media_type)?;
            let id = Uuid::now_v7().to_string();
            let relative = PathBuf::from("blobs")
                .join(&id[..2])
                .join(format!("{id}.mtb"));
            let staged_path = staging_root.join(format!("{id}.tmp"));
            let final_path = blob_root.join(&relative);
            fs::create_dir_all(&staging_root)?;
            fs::write(&staged_path, encrypt_asset(&bytes, key)?)?;
            staged_assets.push(StagedLegacyAsset {
                id: id.clone(),
                plaintext_sha256: hash.clone(),
                media_type: source.media_type.clone(),
                byte_length: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
                encrypted_path: relative.to_string_lossy().replace('\\', "/"),
                staged_path,
                final_path,
                moved_to_final: false,
            });
            asset_ids.insert(hash, (id, true));
        }
        Ok(())
    })();
    if let Err(error) = stage_result {
        cleanup_legacy_staging(&staging_root, &staged_assets);
        return Err(error);
    }

    let receipt = LegacyImportReceipt {
        import_id: import_id.clone(),
        member_count: i32::try_from(plan.members.len()).unwrap_or(i32::MAX),
        problem_count: i32::try_from(problem_count).unwrap_or(i32::MAX),
        asset_count: asset_total,
        review_count: i32::try_from(review_count).unwrap_or(i32::MAX),
        frozen_problem_count: i32::try_from(frozen_problem_count).unwrap_or(i32::MAX),
        created_at_utc_ms: now_utc_ms as f64,
    };

    let persist_result = persist_legacy_import(
        connection,
        account_id,
        &plan,
        now_utc_ms,
        &receipt,
        &asset_ids,
        &mut staged_assets,
        &mut progress,
    );
    if persist_result.is_err() {
        cleanup_legacy_staging(&staging_root, &staged_assets);
    } else {
        let _ = fs::remove_dir_all(&staging_root);
        let _ = fs::remove_dir(blob_root.join(".legacy-import"));
    }
    persist_result.map(|_| receipt)
}

#[allow(clippy::too_many_arguments)]
fn persist_legacy_import(
    connection: &mut Connection,
    account_id: &str,
    plan: &LegacyImportPlan,
    now_utc_ms: i64,
    receipt: &LegacyImportReceipt,
    asset_ids: &HashMap<String, (String, bool)>,
    staged_assets: &mut [StagedLegacyAsset],
    progress: &mut impl FnMut(LegacyImportProgress),
) -> Result<(), LegacyImportError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO legacy_imports(
           id, account_id, source_fingerprint, member_count, problem_count, asset_count,
           review_count, status, created_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 'completed', ?8)",
        params![
            receipt.import_id,
            account_id,
            plan.source_fingerprint,
            receipt.member_count,
            receipt.problem_count,
            receipt.asset_count,
            receipt.review_count,
            now_utc_ms
        ],
    )?;

    let mut profile_ids = Vec::with_capacity(plan.members.len());
    for member in &plan.members {
        let profile_id = Uuid::now_v7().to_string();
        let profile_name = unique_profile_name(&transaction, account_id, &member.name)?;
        transaction.execute(
            "INSERT INTO learner_profiles(
               id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES(?1, ?2, ?3, ?4, ?4, 1)",
            params![profile_id, account_id, profile_name, now_utc_ms],
        )?;
        record_import_entity(
            &transaction,
            &receipt.import_id,
            "profile",
            &profile_id,
            true,
        )?;
        let payload = serde_json::to_string(&serde_json::json!({
            "id": profile_id,
            "accountId": account_id,
            "name": profile_name,
            "createdAtUtcMs": now_utc_ms,
            "updatedAtUtcMs": now_utc_ms,
            "revision": 1
        }))?;
        insert_import_sync_operation(
            &transaction,
            &receipt.import_id,
            account_id,
            Some(&profile_id),
            "learner_profile",
            &profile_id,
            &payload,
            now_utc_ms,
        )?;
        profile_ids.push(profile_id);
    }

    let default_profile = profile_ids.first().ok_or(LegacyImportError::UnsafeSource)?;
    for staged in staged_assets.iter() {
        transaction.execute(
            "INSERT INTO assets(
               id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
               created_at_utc_ms
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                staged.id,
                account_id,
                staged.plaintext_sha256,
                staged.encrypted_path,
                staged.byte_length,
                staged.media_type,
                now_utc_ms
            ],
        )?;
        record_import_entity(&transaction, &receipt.import_id, "asset", &staged.id, true)?;
        let payload = serde_json::to_string(&serde_json::json!({
            "id": staged.id,
            "accountId": account_id,
            "plaintextSha256": staged.plaintext_sha256,
            "encryptedPath": staged.encrypted_path,
            "byteLength": staged.byte_length,
            "mediaType": staged.media_type,
            "createdAtUtcMs": now_utc_ms
        }))?;
        insert_import_sync_operation(
            &transaction,
            &receipt.import_id,
            account_id,
            Some(default_profile),
            "asset",
            &staged.id,
            &payload,
            now_utc_ms,
        )?;
    }
    for (asset_id, created) in asset_ids.values() {
        record_import_entity(
            &transaction,
            &receipt.import_id,
            "asset",
            asset_id,
            *created,
        )?;
    }

    let mut completed = 0_i32;
    for (member, profile_id) in plan.members.iter().zip(&profile_ids) {
        let mut frozen_problem_ids = Vec::new();
        for problem in &member.problems {
            let problem_id = Uuid::now_v7().to_string();
            let tags_json = serde_json::to_string(&problem.tags)?;
            transaction.execute(
                "INSERT INTO problems(
                   id, account_id, profile_id, subject, tags_json, note, status,
                   time_limit_seconds, created_at_utc_ms, updated_at_utc_ms, revision
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8, ?8, 1)",
                params![
                    problem_id,
                    account_id,
                    profile_id,
                    problem.subject,
                    tags_json,
                    problem.note,
                    problem.time_limit_seconds,
                    now_utc_ms
                ],
            )?;
            record_import_entity(
                &transaction,
                &receipt.import_id,
                "problem",
                &problem_id,
                true,
            )?;
            let mut linked_asset_ids = Vec::new();
            for (role, assets) in [
                ("question", &problem.question_assets),
                ("answer", &problem.answer_assets),
            ] {
                for (position, asset) in assets.iter().enumerate() {
                    let (asset_id, _) = asset_ids
                        .get(&asset.plaintext_sha256)
                        .ok_or(LegacyImportError::UnsafeSource)?;
                    transaction.execute(
                        "INSERT INTO problem_assets(problem_id, asset_id, role, position)
                         VALUES(?1, ?2, ?3, ?4)",
                        params![problem_id, asset_id, role, position as i64],
                    )?;
                    linked_asset_ids.push(asset_id.clone());
                }
            }
            let problem_payload = serde_json::to_string(&serde_json::json!({
                "id": problem_id,
                "accountId": account_id,
                "profileId": profile_id,
                "subject": problem.subject,
                "tags": problem.tags,
                "note": problem.note,
                "timeLimitSeconds": problem.time_limit_seconds,
                "assetIds": linked_asset_ids,
                "createdAtUtcMs": now_utc_ms,
                "updatedAtUtcMs": now_utc_ms,
                "revision": 1
            }))?;
            insert_import_sync_operation(
                &transaction,
                &receipt.import_id,
                account_id,
                Some(profile_id),
                "problem",
                &problem_id,
                &problem_payload,
                now_utc_ms,
            )?;

            let mut last_reviewed = None;
            for review in &problem.reviews {
                let event_id = Uuid::now_v7().to_string();
                let rating = match review.rating {
                    LegacyRating::Good => "good",
                    LegacyRating::Again => "again",
                };
                transaction.execute(
                    "INSERT INTO review_events(
                       id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
                       occurred_at_utc_ms, algorithm_version, parameter_version
                     ) VALUES(?1, ?2, ?3, ?4, 'legacy-import', ?5, ?6, ?7,
                              'legacy-proficiency-v1', 'legacy-import-v1')",
                    params![
                        event_id,
                        account_id,
                        profile_id,
                        problem_id,
                        rating,
                        review.duration_ms,
                        review.occurred_at_utc_ms
                    ],
                )?;
                record_import_entity(
                    &transaction,
                    &receipt.import_id,
                    "review_event",
                    &event_id,
                    true,
                )?;
                let payload = serde_json::to_string(&serde_json::json!({
                    "id": event_id,
                    "accountId": account_id,
                    "profileId": profile_id,
                    "problemId": problem_id,
                    "deviceId": "legacy-import",
                    "rating": rating,
                    "durationMs": review.duration_ms,
                    "occurredAtUtcMs": review.occurred_at_utc_ms,
                    "algorithmVersion": "legacy-proficiency-v1",
                    "parameterVersion": "legacy-import-v1"
                }))?;
                insert_import_sync_operation(
                    &transaction,
                    &receipt.import_id,
                    account_id,
                    Some(profile_id),
                    "review_event",
                    &event_id,
                    &payload,
                    now_utc_ms,
                )?;
                last_reviewed = Some(
                    last_reviewed
                        .unwrap_or(review.occurred_at_utc_ms)
                        .max(review.occurred_at_utc_ms),
                );
            }
            transaction.execute(
                "INSERT INTO schedule_states(
                   problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms,
                   algorithm_version, parameter_version, rebuilt_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, 'legacy-proficiency-v1',
                          'legacy-import-v1', ?6)",
                params![
                    problem_id,
                    problem.due_at_utc_ms.unwrap_or(now_utc_ms),
                    problem.stability_days,
                    problem.difficulty,
                    last_reviewed,
                    now_utc_ms
                ],
            )?;
            if problem.frozen {
                frozen_problem_ids.push(problem_id);
            }
            completed = completed.saturating_add(1);
            progress(LegacyImportProgress {
                phase: LegacyImportPhase::Writing,
                completed,
                total: receipt.problem_count,
            });
        }

        if !frozen_problem_ids.is_empty() {
            let snapshot_id = Uuid::now_v7().to_string();
            let ids_json = serde_json::to_string(&frozen_problem_ids)?;
            let configuration = serde_json::to_string(&serde_json::json!({
                "layout": "interleaved",
                "source": "legacy-import"
            }))?;
            transaction.execute(
                "INSERT INTO export_snapshots(
                   id, account_id, profile_id, title, problem_ids_json, configuration_json,
                   created_at_utc_ms, revision
                 ) VALUES(?1, ?2, ?3, '旧版冻结批次', ?4, ?5, ?6, 1)",
                params![
                    snapshot_id,
                    account_id,
                    profile_id,
                    ids_json,
                    configuration,
                    now_utc_ms
                ],
            )?;
            record_import_entity(
                &transaction,
                &receipt.import_id,
                "export_snapshot",
                &snapshot_id,
                true,
            )?;
            let payload = serde_json::to_string(&serde_json::json!({
                "id": snapshot_id,
                "accountId": account_id,
                "profileId": profile_id,
                "title": "旧版冻结批次",
                "problemIds": frozen_problem_ids,
                "configuration": { "layout": "interleaved", "source": "legacy-import" },
                "createdAtUtcMs": now_utc_ms,
                "revision": 1
            }))?;
            insert_import_sync_operation(
                &transaction,
                &receipt.import_id,
                account_id,
                Some(profile_id),
                "export_snapshot",
                &snapshot_id,
                &payload,
                now_utc_ms,
            )?;
        }
    }

    for asset in staged_assets.iter_mut() {
        if let Some(parent) = asset.final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&asset.staged_path, &asset.final_path)?;
        asset.moved_to_final = true;
    }
    progress(LegacyImportProgress {
        phase: LegacyImportPhase::Verifying,
        completed: 0,
        total: 1,
    });
    if legacy_tree_fingerprint(&plan.source_root)? != plan.source_fingerprint {
        return Err(LegacyImportError::SourceChanged);
    }
    transaction.commit()?;
    progress(LegacyImportProgress {
        phase: LegacyImportPhase::Completed,
        completed: 1,
        total: 1,
    });
    Ok(())
}

fn insert_import_sync_operation(
    transaction: &Transaction<'_>,
    import_id: &str,
    account_id: &str,
    profile_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    payload: &str,
    now_utc_ms: i64,
) -> Result<(), LegacyImportError> {
    let operation_id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO sync_operations(
           id, account_id, profile_id, entity_type, entity_id, operation, payload_json,
           status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, 'upsert', ?6, 'pending', 0, ?7, ?7)",
        params![
            operation_id,
            account_id,
            profile_id,
            entity_type,
            entity_id,
            payload,
            now_utc_ms
        ],
    )?;
    record_import_entity(
        transaction,
        import_id,
        "sync_operation",
        &operation_id,
        true,
    )?;
    Ok(())
}

fn record_import_entity(
    transaction: &Transaction<'_>,
    import_id: &str,
    entity_type: &str,
    entity_id: &str,
    created: bool,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "INSERT INTO legacy_import_entities(import_id, entity_type, entity_id, created_by_import)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(import_id, entity_type, entity_id) DO UPDATE SET
           created_by_import = MAX(created_by_import, excluded.created_by_import)",
        params![import_id, entity_type, entity_id, i64::from(created)],
    )?;
    Ok(())
}

fn unique_profile_name(
    transaction: &Transaction<'_>,
    account_id: &str,
    requested: &str,
) -> Result<String, rusqlite::Error> {
    let base = take_chars(requested.trim(), 40);
    let base = if base.is_empty() {
        "旧版档案"
    } else {
        &base
    };
    for suffix_number in 1..=10_000 {
        let candidate = if suffix_number == 1 {
            base.to_owned()
        } else {
            let suffix = format!(" ({suffix_number})");
            format!(
                "{}{}",
                take_chars(base, 40 - suffix.chars().count()),
                suffix
            )
        };
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM learner_profiles WHERE account_id = ?1 AND name = ?2
             )",
            params![account_id, candidate],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(rusqlite::Error::InvalidQuery)
}

fn validate_import_image(bytes: &[u8], expected_media_type: &str) -> Result<(), LegacyImportError> {
    let guessed = image::guess_format(bytes).map_err(|_| LegacyImportError::InvalidImage)?;
    let expected_matches = matches!(
        (expected_media_type, guessed),
        ("image/jpeg", image::ImageFormat::Jpeg)
            | ("image/png", image::ImageFormat::Png)
            | ("image/webp", image::ImageFormat::WebP)
    );
    if !expected_matches {
        return Err(LegacyImportError::InvalidImage);
    }
    let decoded = image::load_from_memory(bytes).map_err(|_| LegacyImportError::InvalidImage)?;
    let (width, height) = decoded.dimensions();
    if width == 0
        || height == 0
        || width > 12_000
        || height > 12_000
        || u64::from(width).saturating_mul(u64::from(height)) > 80_000_000
    {
        return Err(LegacyImportError::InvalidImage);
    }
    Ok(())
}

fn plaintext_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn cleanup_legacy_staging(staging_root: &Path, assets: &[StagedLegacyAsset]) {
    for asset in assets {
        let path = if asset.moved_to_final {
            &asset.final_path
        } else {
            &asset.staged_path
        };
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir_all(staging_root);
    if let Some(parent) = staging_root.parent() {
        let _ = fs::remove_dir(parent);
    }
}

pub fn rollback_legacy_import(
    connection: &mut Connection,
    blob_root: &Path,
    account_id: &str,
    import_id: &str,
    now_utc_ms: i64,
) -> Result<LegacyRollbackReceipt, LegacyImportError> {
    let status = connection
        .query_row(
            "SELECT status FROM legacy_imports WHERE id = ?1 AND account_id = ?2",
            params![import_id, account_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(LegacyImportError::ImportNotFound)?;
    if status != "completed" {
        return Err(LegacyImportError::ImportNotFound);
    }

    let transaction = connection.transaction()?;
    let sync_ids = import_entity_ids(&transaction, import_id, "sync_operation", true)?;
    for id in sync_ids {
        transaction.execute("DELETE FROM sync_operations WHERE id = ?1", [&id])?;
    }
    let review_ids = import_entity_ids(&transaction, import_id, "review_event", true)?;
    for id in review_ids {
        transaction.execute("DELETE FROM review_events WHERE id = ?1", [&id])?;
    }

    let snapshot_ids = import_entity_ids(&transaction, import_id, "export_snapshot", true)?;
    let mut removed_snapshots = 0_i32;
    for id in snapshot_ids {
        removed_snapshots = removed_snapshots.saturating_add(
            i32::try_from(transaction.execute(
                "DELETE FROM export_snapshots WHERE id = ?1 AND account_id = ?2 AND revision = 1",
                params![id, account_id],
            )?)
            .unwrap_or(i32::MAX),
        );
    }

    let problem_ids = import_entity_ids(&transaction, import_id, "problem", true)?;
    let mut removed_problem_count = 0_i32;
    for id in &problem_ids {
        removed_problem_count = removed_problem_count.saturating_add(
            i32::try_from(transaction.execute(
                "DELETE FROM problems
                 WHERE id = ?1 AND account_id = ?2 AND revision = 1
                   AND NOT EXISTS(SELECT 1 FROM review_events WHERE problem_id = ?1)
                   AND NOT EXISTS(
                     SELECT 1 FROM export_snapshots s, json_each(s.problem_ids_json) j
                     WHERE j.value = ?1
                   )",
                params![id, account_id],
            )?)
            .unwrap_or(i32::MAX),
        );
    }

    let profile_ids = import_entity_ids(&transaction, import_id, "profile", true)?;
    let mut removed_profile_count = 0_i32;
    for id in &profile_ids {
        removed_profile_count = removed_profile_count.saturating_add(
            i32::try_from(transaction.execute(
                "DELETE FROM learner_profiles
                 WHERE id = ?1 AND account_id = ?2 AND revision = 1
                   AND NOT EXISTS(SELECT 1 FROM problems WHERE profile_id = ?1)
                   AND NOT EXISTS(SELECT 1 FROM capture_batches WHERE profile_id = ?1)
                   AND NOT EXISTS(SELECT 1 FROM account_preferences WHERE active_profile_id = ?1)",
                params![id, account_id],
            )?)
            .unwrap_or(i32::MAX),
        );
    }

    let asset_ids = import_entity_ids(&transaction, import_id, "asset", true)?;
    let quarantine = blob_root.join(".legacy-rollback").join(import_id);
    let mut quarantined = Vec::<(PathBuf, PathBuf)>::new();
    let mut removable_assets = Vec::<(String, PathBuf)>::new();
    for id in &asset_ids {
        let encrypted_path = transaction
            .query_row(
                "SELECT encrypted_path FROM assets
                 WHERE id = ?1 AND account_id = ?2
                   AND NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)
                   AND NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)",
                params![id, account_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(relative) = encrypted_path {
            let relative = Path::new(&relative);
            if !is_safe_relative_path(relative) {
                return Err(LegacyImportError::UnsafeSource);
            }
            removable_assets.push((id.clone(), blob_root.join(relative)));
        }
    }
    let file_stage_result = (|| -> Result<(), LegacyImportError> {
        for (id, original) in &removable_assets {
            if !original.exists() {
                continue;
            }
            fs::create_dir_all(&quarantine)?;
            let staged = quarantine.join(format!("{id}.mtb"));
            fs::rename(original, &staged)?;
            quarantined.push((original.clone(), staged));
        }
        Ok(())
    })();
    if let Err(error) = file_stage_result {
        restore_quarantined_assets(&quarantined);
        return Err(error);
    }

    let finalize = (|| -> Result<(i32, i32), LegacyImportError> {
        let mut removed_asset_count = 0_i32;
        for (id, _) in &removable_assets {
            removed_asset_count = removed_asset_count.saturating_add(
                i32::try_from(transaction.execute(
                    "DELETE FROM assets WHERE id = ?1 AND account_id = ?2
                     AND NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)
                     AND NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)",
                    params![id, account_id],
                )?)
                .unwrap_or(i32::MAX),
            );
        }
        transaction.execute(
            "UPDATE legacy_imports
             SET status = 'rolled_back', rolled_back_at_utc_ms = ?3
             WHERE id = ?1 AND account_id = ?2 AND status = 'completed'",
            params![import_id, account_id, now_utc_ms],
        )?;

        let preserved_entity_count = i32::try_from(problem_ids.len())
            .unwrap_or(i32::MAX)
            .saturating_sub(removed_problem_count)
            .saturating_add(
                i32::try_from(profile_ids.len())
                    .unwrap_or(i32::MAX)
                    .saturating_sub(removed_profile_count),
            )
            .saturating_add(
                i32::try_from(asset_ids.len())
                    .unwrap_or(i32::MAX)
                    .saturating_sub(removed_asset_count),
            )
            .saturating_add(
                i32::try_from(
                    import_entity_ids(&transaction, import_id, "export_snapshot", true)?.len(),
                )
                .unwrap_or(i32::MAX)
                .saturating_sub(removed_snapshots),
            );
        transaction.commit()?;
        Ok((removed_asset_count, preserved_entity_count))
    })();
    let (removed_asset_count, preserved_entity_count) = match finalize {
        Ok(result) => result,
        Err(error) => {
            restore_quarantined_assets(&quarantined);
            return Err(error);
        }
    };
    let _ = fs::remove_dir_all(&quarantine);
    let _ = fs::remove_dir(blob_root.join(".legacy-rollback"));

    Ok(LegacyRollbackReceipt {
        import_id: import_id.to_owned(),
        removed_problem_count,
        removed_profile_count,
        removed_asset_count,
        preserved_entity_count,
        rolled_back_at_utc_ms: now_utc_ms as f64,
    })
}

fn import_entity_ids(
    transaction: &Transaction<'_>,
    import_id: &str,
    entity_type: &str,
    created: bool,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT entity_id FROM legacy_import_entities
         WHERE import_id = ?1 AND entity_type = ?2 AND created_by_import = ?3
         ORDER BY entity_id",
    )?;
    Ok(statement
        .query_map(params![import_id, entity_type, i64::from(created)], |row| {
            row.get(0)
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn restore_quarantined_assets(paths: &[(PathBuf, PathBuf)]) {
    for (original, staged) in paths.iter().rev() {
        if let Some(parent) = original.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::rename(staged, original);
    }
}

#[derive(Debug, Error)]
pub enum LegacyScanError {
    #[error("legacy storage root is not a directory")]
    InvalidRoot,
    #[error("failed to inspect legacy storage: {0}")]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyIssue {
    pub code: String,
    pub member: String,
    pub record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyScanReport {
    pub members: i32,
    pub metadata_records: i32,
    pub existing_assets: i32,
    pub training_records: i32,
    pub frozen_records: i32,
    pub duplicate_assets: i32,
    pub truncated: bool,
    pub issues: Vec<LegacyIssue>,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyStore {
    #[allow(dead_code)]
    version: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, LegacyFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyFile {
    id: Option<String>,
    relative_path: Option<String>,
    hash: Option<String>,
    pair_id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    proficiency: Option<f64>,
    training_interval: Option<f64>,
    next_training_date: Option<String>,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    notes: String,
    answer_time_limit: Option<i64>,
    #[serde(default)]
    training_records: Vec<serde_json::Value>,
    #[serde(default)]
    is_frozen: bool,
}

pub fn build_legacy_import_plan(root: &Path) -> Result<LegacyImportPlan, LegacyScanError> {
    if !root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }

    let mut report = scan_legacy_storage(&canonical_root)?;
    let sources = discover_member_sources(&canonical_root, &mut report)?;
    let mut members = Vec::with_capacity(sources.len());

    for source in sources {
        if let Some(member) = build_member_plan(&source, &canonical_root, &mut report) {
            members.push(member);
        }
    }

    Ok(LegacyImportPlan {
        source_fingerprint: legacy_tree_fingerprint(&canonical_root)?,
        source_root: canonical_root,
        report,
        members,
    })
}

pub fn legacy_tree_fingerprint(root: &Path) -> Result<String, LegacyScanError> {
    if !root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }

    let mut files = Vec::new();
    collect_fingerprint_files(&canonical_root, &canonical_root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.len() > MAX_RECORDS.saturating_add(MAX_DIRECTORY_ENTRIES) {
        return Err(LegacyScanError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy tree contains too many files",
        )));
    }

    let mut digest = Sha256::new();
    let mut total_bytes = 0_u64;
    for (relative, path) in files {
        let metadata = path.metadata()?;
        total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "legacy tree is too large")
        })?;
        if total_bytes > MAX_TOTAL_ASSET_BYTES.saturating_add(MAX_METADATA_BYTES) {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy tree is too large",
            )));
        }
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_le_bytes());
        let mut file = fs::File::open(path)?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        digest.update([0xff]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_fingerprint_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), LegacyScanError> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(root) {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy path resolves outside the selected directory",
            )));
        }
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(LegacyScanError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy tree contains a symbolic link",
            )));
        }
        if canonical.is_dir() {
            collect_fingerprint_files(root, &canonical, files)?;
        } else if canonical.is_file() {
            let relative = canonical
                .strip_prefix(root)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid legacy path"))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, canonical));
        }
    }
    Ok(())
}

fn build_member_plan(
    source: &MemberSource,
    selected_root: &Path,
    report: &mut LegacyScanReport,
) -> Option<LegacyMemberPlan> {
    let metadata_path = source.metadata_path.canonicalize().ok()?;
    if !metadata_path.starts_with(selected_root) || !metadata_path.starts_with(&source.member_root)
    {
        return None;
    }
    let metadata = read_bounded(&metadata_path, MAX_METADATA_BYTES).ok()?;
    let store: LegacyStore = serde_json::from_slice(&metadata).ok()?;
    let files_root = source.files_root.canonicalize().ok()?;
    if !files_root.starts_with(selected_root) || !files_root.starts_with(&source.member_root) {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<(String, LegacyFile)>> = BTreeMap::new();
    for (map_id, record) in store.files.into_iter().take(MAX_RECORDS) {
        let record_id = safe_label(record.id.as_deref().unwrap_or(&map_id));
        let is_answer = record.kind.as_deref() == Some("answer");
        match (record.pair_id.as_deref(), is_answer) {
            (Some(pair_id), _) if !pair_id.trim().is_empty() => groups
                .entry(safe_label(pair_id))
                .or_default()
                .push((record_id, record)),
            (None, false) | (Some(""), false) => {
                groups.insert(record_id.clone(), vec![(record_id, record)]);
            }
            _ => push_source_issue(
                report,
                "orphan_answer",
                source,
                Some(record_id),
                "answer image is not paired with a question",
            ),
        }
    }

    let mut problems = Vec::new();
    for (group_key, mut records) in groups {
        records.sort_by(|left, right| left.0.cmp(&right.0));
        let question_indices = records
            .iter()
            .enumerate()
            .filter_map(|(index, (_, record))| {
                (record.kind.as_deref() != Some("answer")).then_some(index)
            })
            .collect::<Vec<_>>();
        if question_indices.is_empty() {
            for (record_id, _) in records {
                push_source_issue(
                    report,
                    "orphan_answer",
                    source,
                    Some(record_id),
                    "answer image has no question in its pair",
                );
            }
            continue;
        }

        let metadata_record = &records[question_indices[0]].1;
        let mut question_assets = Vec::new();
        let mut answer_assets = Vec::new();
        for (record_id, record) in &records {
            let Some(asset) = build_asset_plan(source, &files_root, record_id, record, report)
            else {
                continue;
            };
            if record.kind.as_deref() == Some("answer") {
                answer_assets.push(asset);
            } else {
                question_assets.push(asset);
            }
        }
        if question_assets.is_empty() {
            push_source_issue(
                report,
                "missing_question_asset",
                source,
                Some(group_key),
                "problem has no readable question image",
            );
            continue;
        }

        let mut reviews = Vec::new();
        for index in question_indices {
            let (record_id, record) = &records[index];
            for value in &record.training_records {
                if let Some(review) = parse_review(value, source, record_id, report) {
                    reviews.push(review);
                }
            }
        }
        reviews.sort_by_key(|review| review.occurred_at_utc_ms);

        let due_at_utc_ms = metadata_record
            .next_training_date
            .as_deref()
            .and_then(|value| {
                parse_utc_ms(value).or_else(|| {
                    push_source_issue(
                        report,
                        "invalid_due_date",
                        source,
                        Some(group_key.clone()),
                        "next training date is not valid RFC 3339",
                    );
                    None
                })
            });
        let stability_days = finite_or(metadata_record.training_interval, 1.0).clamp(0.1, 36_500.0);
        let proficiency = finite_or(metadata_record.proficiency, 0.0).clamp(0.0, 100.0);
        let difficulty = (10.0 - proficiency * 0.09).clamp(1.0, 10.0);

        problems.push(LegacyProblemPlan {
            source_problem_key: group_key,
            subject: normalized_subject(&metadata_record.subject),
            tags: normalized_tags(&metadata_record.tags),
            note: take_chars(metadata_record.notes.trim(), MAX_NOTE_CHARS),
            time_limit_seconds: metadata_record
                .answer_time_limit
                .filter(|seconds| (1..=86_400).contains(seconds))
                .and_then(|seconds| i32::try_from(seconds).ok()),
            question_assets,
            answer_assets,
            reviews,
            due_at_utc_ms,
            stability_days,
            difficulty,
            frozen: records.iter().any(|(_, record)| record.is_frozen),
        });
    }

    Some(LegacyMemberPlan {
        source_member_key: source.name.clone(),
        name: source.name.clone(),
        problems,
    })
}

fn build_asset_plan(
    source: &MemberSource,
    files_root: &Path,
    record_id: &str,
    record: &LegacyFile,
    report: &mut LegacyScanReport,
) -> Option<LegacyAssetPlan> {
    let relative = record.relative_path.as_deref().map(Path::new)?;
    if !is_safe_relative_path(relative) {
        return None;
    }
    let source_path = files_root.join(relative).canonicalize().ok()?;
    if !source_path.starts_with(files_root) || !source_path.is_file() {
        return None;
    }
    let media_type = match source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => {
            push_source_issue(
                report,
                "unsupported_asset",
                source,
                Some(record_id.to_owned()),
                "image format is not supported by the new app",
            );
            return None;
        }
    };
    match sha256_file(&source_path, MAX_ASSET_BYTES) {
        Ok((plaintext_sha256, byte_length)) => Some(LegacyAssetPlan {
            source_record_id: record_id.to_owned(),
            source_path,
            media_type: media_type.to_owned(),
            plaintext_sha256,
            byte_length,
        }),
        Err(_) => None,
    }
}

fn parse_review(
    value: &serde_json::Value,
    source: &MemberSource,
    record_id: &str,
    report: &mut LegacyScanReport,
) -> Option<LegacyReviewPlan> {
    let date = value.get("date").and_then(serde_json::Value::as_str)?;
    let Some(occurred_at_utc_ms) = parse_utc_ms(date) else {
        push_source_issue(
            report,
            "invalid_training_date",
            source,
            Some(record_id.to_owned()),
            "training date is not valid RFC 3339",
        );
        return None;
    };
    let rating = match value.get("result").and_then(serde_json::Value::as_str) {
        Some("success") => LegacyRating::Good,
        Some("fail") => LegacyRating::Again,
        _ => {
            push_source_issue(
                report,
                "invalid_training_result",
                source,
                Some(record_id.to_owned()),
                "training result is neither success nor fail",
            );
            return None;
        }
    };
    let duration_ms = value
        .get("answerTime")
        .and_then(serde_json::Value::as_f64)
        .filter(|duration| duration.is_finite() && *duration >= 0.0)
        .map(|duration| duration.min(i64::MAX as f64) as i64)
        .unwrap_or(0);
    Some(LegacyReviewPlan {
        occurred_at_utc_ms,
        rating,
        duration_ms,
    })
}

fn parse_utc_ms(value: &str) -> Option<i64> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).ok()?;
    i64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).ok()
}

fn finite_or(value: Option<f64>, fallback: f64) -> f64 {
    value.filter(|value| value.is_finite()).unwrap_or(fallback)
}

fn normalized_subject(value: &str) -> String {
    let value = take_chars(value.trim(), MAX_LABEL_CHARS);
    if value.is_empty() {
        "未分类".to_owned()
    } else {
        value
    }
}

fn normalized_tags(values: &[String]) -> Vec<String> {
    let mut tags = Vec::new();
    for value in values {
        let tag = take_chars(value.trim(), MAX_TAG_CHARS);
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
        if tags.len() == MAX_TAGS {
            break;
        }
    }
    tags
}

fn take_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub fn scan_legacy_storage(root: &Path) -> Result<LegacyScanReport, LegacyScanError> {
    if !root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }

    let mut report = LegacyScanReport::default();
    let sources = discover_member_sources(&canonical_root, &mut report)?;
    report.members = i32::try_from(sources.len()).unwrap_or(i32::MAX);
    let mut observed_hashes: HashMap<String, (String, String)> = HashMap::new();
    let mut scanned_asset_bytes = 0_u64;

    for source in sources {
        scan_member(
            &source,
            &canonical_root,
            &mut report,
            &mut observed_hashes,
            &mut scanned_asset_bytes,
        );
    }

    Ok(report)
}

struct MemberSource {
    name: String,
    member_root: PathBuf,
    metadata_path: PathBuf,
    files_root: PathBuf,
}

fn discover_member_sources(
    root: &Path,
    report: &mut LegacyScanReport,
) -> Result<Vec<MemberSource>, io::Error> {
    let mut sources = Vec::new();
    let root_metadata = root.join(".metadata.json");
    if root_metadata.is_file() {
        match root_metadata.canonicalize() {
            Ok(metadata_path) if metadata_path.starts_with(root) => sources.push(MemberSource {
                name: "default".to_owned(),
                member_root: root.to_path_buf(),
                metadata_path,
                files_root: root.to_path_buf(),
            }),
            _ => push_issue(
                report,
                LegacyIssue {
                    code: "unsafe_metadata_path".to_owned(),
                    member: "default".to_owned(),
                    record_id: None,
                    detail: "metadata resolves outside the selected directory".to_owned(),
                },
            ),
        }
    }

    let members_root = root.join("members");
    if !members_root.is_dir() {
        return Ok(sources);
    }
    let canonical_members_root = members_root.canonicalize()?;
    if !canonical_members_root.starts_with(root) {
        push_issue(
            report,
            LegacyIssue {
                code: "unsafe_member_path".to_owned(),
                member: "members".to_owned(),
                record_id: None,
                detail: "members directory resolves outside the selected directory".to_owned(),
            },
        );
        return Ok(sources);
    }

    let mut entries = fs::read_dir(&canonical_members_root)?
        .take(MAX_DIRECTORY_ENTRIES.saturating_add(1))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.len() > MAX_DIRECTORY_ENTRIES {
        entries.truncate(MAX_DIRECTORY_ENTRIES);
        mark_truncated(report, "目录条目数量超过安全扫描上限，报告已截断");
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        if !entry.path().is_dir() {
            continue;
        }
        if sources.len() >= MAX_MEMBERS {
            mark_truncated(report, "学习档案数量超过安全扫描上限，报告已截断");
            break;
        }
        let name = safe_label(&entry.file_name().to_string_lossy());
        let member_root = match entry.path().canonicalize() {
            Ok(path) if path.starts_with(root) && path.starts_with(&canonical_members_root) => path,
            _ => {
                push_issue(
                    report,
                    LegacyIssue {
                        code: "unsafe_member_path".to_owned(),
                        member: name,
                        record_id: None,
                        detail: "learning profile resolves outside the selected directory"
                            .to_owned(),
                    },
                );
                continue;
            }
        };
        sources.push(MemberSource {
            name,
            metadata_path: member_root.join(".metadata.json"),
            files_root: member_root.join("files"),
            member_root,
        });
    }

    Ok(sources)
}

fn scan_member(
    source: &MemberSource,
    selected_root: &Path,
    report: &mut LegacyScanReport,
    observed_hashes: &mut HashMap<String, (String, String)>,
    scanned_asset_bytes: &mut u64,
) {
    if usize::try_from(report.metadata_records).unwrap_or(MAX_RECORDS) >= MAX_RECORDS {
        mark_truncated(report, "元数据记录数量超过安全扫描上限，报告已截断");
        return;
    }

    let metadata_path = match source.metadata_path.canonicalize() {
        Ok(path) if path.starts_with(selected_root) && path.starts_with(&source.member_root) => {
            path
        }
        Ok(_) => {
            push_source_issue(
                report,
                "unsafe_metadata_path",
                source,
                None,
                "metadata resolves outside the selected directory",
            );
            return;
        }
        Err(_) => {
            push_source_issue(
                report,
                "missing_metadata",
                source,
                None,
                "member metadata cannot be read",
            );
            return;
        }
    };
    let metadata = match read_bounded(&metadata_path, MAX_METADATA_BYTES) {
        Ok(contents) => contents,
        Err(BoundedReadError::TooLarge) => {
            push_source_issue(
                report,
                "metadata_too_large",
                source,
                None,
                "metadata exceeds the safe scan size",
            );
            return;
        }
        Err(BoundedReadError::Io) => {
            push_source_issue(
                report,
                "missing_metadata",
                source,
                None,
                "member metadata cannot be read",
            );
            return;
        }
    };
    let store: LegacyStore = match serde_json::from_slice(&metadata) {
        Ok(store) => store,
        Err(error) => {
            push_source_issue(
                report,
                "invalid_metadata",
                source,
                None,
                &format!(
                    "invalid JSON near line {} column {}",
                    error.line(),
                    error.column()
                ),
            );
            return;
        }
    };

    let processed = usize::try_from(report.metadata_records).unwrap_or(MAX_RECORDS);
    let remaining_records = MAX_RECORDS.saturating_sub(processed);
    let records_to_scan = store.files.len().min(remaining_records);
    report.metadata_records = report
        .metadata_records
        .saturating_add(i32::try_from(records_to_scan).unwrap_or(i32::MAX));
    if store.files.len() > remaining_records {
        mark_truncated(report, "元数据记录数量超过安全扫描上限，报告已截断");
    }
    let mut pair_roles: HashMap<String, (bool, bool)> = HashMap::new();
    for record in store.files.values().take(records_to_scan) {
        let Some(pair_id) = record
            .pair_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let roles = pair_roles.entry(pair_id.to_owned()).or_default();
        if record.kind.as_deref() == Some("answer") {
            roles.1 = true;
        } else {
            roles.0 = true;
        }
    }

    let canonical_files_root = if source.files_root.exists() {
        match source.files_root.canonicalize() {
            Ok(path)
                if path.starts_with(selected_root) && path.starts_with(&source.member_root) =>
            {
                Some(path)
            }
            _ => {
                push_source_issue(
                    report,
                    "unsafe_files_root",
                    source,
                    None,
                    "image directory resolves outside the selected directory",
                );
                None
            }
        }
    } else {
        None
    };
    let unsafe_files_root = source.files_root.exists() && canonical_files_root.is_none();

    for (map_id, record) in store.files.into_iter().take(records_to_scan) {
        let record_id = safe_label(record.id.as_deref().unwrap_or(&map_id));
        report.training_records = report
            .training_records
            .saturating_add(i32::try_from(record.training_records.len()).unwrap_or(i32::MAX));
        report.frozen_records = report
            .frozen_records
            .saturating_add(i32::from(record.is_frozen));

        if let Some(pair_id) = record.pair_id.as_deref()
            && !matches!(pair_roles.get(pair_id), Some((true, true)))
        {
            push_source_issue(
                report,
                "missing_pair",
                source,
                Some(record_id.clone()),
                "paired record does not exist",
            );
        }

        let Some(relative_path) = record.relative_path.as_deref() else {
            push_source_issue(
                report,
                "missing_relative_path",
                source,
                Some(record_id),
                "record has no relativePath",
            );
            continue;
        };
        let relative_path = Path::new(relative_path);
        if !is_safe_relative_path(relative_path) {
            push_source_issue(
                report,
                "unsafe_relative_path",
                source,
                Some(record_id),
                "relativePath escapes the member image directory",
            );
            continue;
        }
        if unsafe_files_root {
            continue;
        }

        let Some(files_root) = canonical_files_root.as_ref() else {
            push_source_issue(
                report,
                "missing_asset",
                source,
                Some(record_id),
                "referenced image is missing",
            );
            continue;
        };
        let asset_path = files_root.join(relative_path);
        if !asset_path.is_file() {
            push_source_issue(
                report,
                "missing_asset",
                source,
                Some(record_id),
                "referenced image is missing",
            );
            continue;
        }
        let canonical_asset_path = match asset_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                push_source_issue(
                    report,
                    "unreadable_asset",
                    source,
                    Some(record_id),
                    "referenced image cannot be read",
                );
                continue;
            }
        };
        if !canonical_asset_path.starts_with(selected_root)
            || !canonical_asset_path.starts_with(files_root)
        {
            push_source_issue(
                report,
                "unsafe_asset_path",
                source,
                Some(record_id),
                "asset resolves outside the selected directory",
            );
            continue;
        }

        let remaining_total = MAX_TOTAL_ASSET_BYTES.saturating_sub(*scanned_asset_bytes);
        if remaining_total == 0 {
            mark_truncated(report, "图片累计大小超过安全扫描上限，报告已截断");
            continue;
        }
        let read_limit = MAX_ASSET_BYTES.min(remaining_total);
        match sha256_file(&canonical_asset_path, read_limit) {
            Ok((actual_hash, byte_length)) => {
                *scanned_asset_bytes = scanned_asset_bytes.saturating_add(byte_length);
                report.existing_assets = report.existing_assets.saturating_add(1);
                if let Some(expected_hash) = record.hash.as_deref()
                    && !expected_hash.eq_ignore_ascii_case(&actual_hash)
                {
                    push_source_issue(
                        report,
                        "hash_mismatch",
                        source,
                        Some(record_id.clone()),
                        "stored hash does not match calculated content hash",
                    );
                }
                if observed_hashes
                    .insert(actual_hash, (source.name.clone(), record_id.clone()))
                    .is_some()
                {
                    report.duplicate_assets = report.duplicate_assets.saturating_add(1);
                    push_source_issue(
                        report,
                        "duplicate_asset",
                        source,
                        Some(record_id),
                        "same content as an earlier record",
                    );
                }
            }
            Err(BoundedReadError::TooLarge) if remaining_total < MAX_ASSET_BYTES => {
                mark_truncated(report, "图片累计大小超过安全扫描上限，报告已截断");
            }
            Err(BoundedReadError::TooLarge) => push_source_issue(
                report,
                "asset_too_large",
                source,
                Some(record_id),
                "referenced image exceeds the safe scan size",
            ),
            Err(BoundedReadError::Io) => push_source_issue(
                report,
                "unreadable_asset",
                source,
                Some(record_id),
                "referenced image cannot be read",
            ),
        }
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[derive(Debug)]
enum BoundedReadError {
    Io,
    TooLarge,
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BoundedReadError> {
    let file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut contents = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)
        .map_err(|_| BoundedReadError::Io)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    Ok(contents)
}

fn sha256_file(path: &Path, max_bytes: u64) -> Result<(String, u64), BoundedReadError> {
    let mut file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| BoundedReadError::Io)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(BoundedReadError::TooLarge)?;
        if total > max_bytes {
            return Err(BoundedReadError::TooLarge);
        }
        digest.update(&buffer[..read]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

fn safe_label(value: &str) -> String {
    if value.contains(['/', '\\', ':']) {
        return "redacted".to_owned();
    }
    let mut output = value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_LABEL_CHARS)
        .collect::<String>();
    if output.trim().is_empty() {
        output = "unknown".to_owned();
    }
    output
}

fn push_source_issue(
    report: &mut LegacyScanReport,
    code: &str,
    source: &MemberSource,
    record_id: Option<String>,
    detail: &str,
) {
    push_issue(
        report,
        LegacyIssue {
            code: code.to_owned(),
            member: source.name.clone(),
            record_id,
            detail: detail.to_owned(),
        },
    );
}

fn push_issue(report: &mut LegacyScanReport, issue: LegacyIssue) {
    if report.issues.len() < MAX_ISSUES {
        report.issues.push(issue);
    } else {
        report.truncated = true;
    }
}

fn mark_truncated(report: &mut LegacyScanReport, detail: &str) {
    if !report.truncated {
        report.truncated = true;
        push_issue(
            report,
            LegacyIssue {
                code: "scan_limit_exceeded".to_owned(),
                member: "system".to_owned(),
                record_id: None,
                detail: detail.to_owned(),
            },
        );
    }
}
