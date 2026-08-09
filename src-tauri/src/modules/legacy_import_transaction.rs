use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    LegacyAssetPlan, LegacyImportError, LegacyImportPhase, LegacyImportPlan, LegacyImportProgress,
    LegacyImportReceipt, LegacyRating, build_legacy_import_plan,
    legacy_scan::{MAX_ASSET_BYTES, read_bounded, take_chars},
    legacy_tree_fingerprint,
};
use crate::infrastructure::assets::encrypt_asset;

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
    candidate_id: &str,
    plan: LegacyImportPlan,
    now_utc_ms: i64,
    mut progress: impl FnMut(LegacyImportProgress),
) -> Result<LegacyImportReceipt, LegacyImportError> {
    if plan.report.truncated || plan.members.is_empty() {
        return Err(LegacyImportError::UnsafeSource);
    }
    progress(LegacyImportProgress {
        candidate_id: candidate_id.to_owned(),
        phase: LegacyImportPhase::Validating,
        completed: 0,
        total: 1,
    });
    let refreshed_plan = build_legacy_import_plan(&plan.source_root)?;
    if refreshed_plan.source_fingerprint != plan.source_fingerprint {
        return Err(LegacyImportError::SourceChanged);
    }
    let plan = refreshed_plan;
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
                candidate_id: candidate_id.to_owned(),
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
        candidate_id,
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
    candidate_id: &str,
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
                candidate_id: candidate_id.to_owned(),
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
        candidate_id: candidate_id.to_owned(),
        phase: LegacyImportPhase::Verifying,
        completed: 0,
        total: 1,
    });
    if legacy_tree_fingerprint(&plan.source_root)? != plan.source_fingerprint {
        return Err(LegacyImportError::SourceChanged);
    }
    transaction.commit()?;
    progress(LegacyImportProgress {
        candidate_id: candidate_id.to_owned(),
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
