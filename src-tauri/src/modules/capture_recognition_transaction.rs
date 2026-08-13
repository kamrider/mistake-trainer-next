use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    application::ports::assets::{AssetDecryptor, AssetEncryptor},
    domain::assets::plaintext_sha256,
    modules::capture_inbox::{
        CaptureCropRecipe, CaptureInboxError, EncodedCrop, MAX_CAPTURE_BATCH_BYTES,
        MAX_CAPTURE_BATCH_ITEMS, encode_crop, get_capture_batch_detail,
        image_format_for_media_type, read_encrypted_blob,
    },
};

use super::{
    ApplyCaptureRecognition, CaptureRecognitionApplyReport, CaptureRecognitionError,
    CaptureRecognitionFailurePoint, CaptureRecognitionRegionProposal, CaptureRecognitionRole,
    MAX_JOB_ITEMS, capture_item_snapshot_hash,
    capture_recognition_operation_ledger::{
        RecognitionLedgerItem, RecognitionLedgerSource, RecognitionOperationLedger,
    },
    validate_regions,
};

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
    group_slot: Option<u32>,
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
        let plaintext = input
            .asset_key
            .decrypt(&read_encrypted_blob(
                &input.blob_root,
                &suggestion.source.encrypted_path,
            )?)
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
                    perspective_quad: None,
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
                group_slot: region.group_slot,
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
            let encrypted = input
                .asset_key
                .encrypt(&region.encoded.bytes)
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
    let mut paired_region_indexes = BTreeMap::<u32, Vec<usize>>::new();
    for (index, region) in prepared.iter().enumerate() {
        if let Some(slot) = region.group_slot {
            paired_region_indexes.entry(slot).or_default().push(index);
        }
    }
    paired_region_indexes.retain(|_, indexes| {
        indexes
            .iter()
            .any(|index| prepared[*index].role == CaptureRecognitionRole::Question)
            && indexes
                .iter()
                .any(|index| prepared[*index].role == CaptureRecognitionRole::Answer)
    });
    let pair_ids = paired_region_indexes
        .keys()
        .map(|slot| (*slot, Uuid::now_v7().to_string()))
        .collect::<BTreeMap<_, _>>();
    let unmatched_answer_count = prepared
        .iter()
        .filter(|region| {
            region.role == CaptureRecognitionRole::Answer
                && region
                    .group_slot
                    .map(|slot| !paired_region_indexes.contains_key(&slot))
                    .unwrap_or(true)
        })
        .count();
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            cleanup_staged_recognition_assets(&staged_assets, false);
            return Err(error.into());
        }
    };
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
        for (slot, indexes) in &paired_region_indexes {
            let pair_id = pair_ids
                .get(slot)
                .ok_or(CaptureRecognitionError::InvalidSuggestion)?;
            let confidence_basis_points = indexes
                .iter()
                .map(|index| prepared[*index].confidence_basis_points)
                .min()
                .unwrap_or_default();
            transaction.execute(
                "INSERT INTO capture_recognition_pairs(
                   id, operation_id, pair_slot, confidence_basis_points, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5)",
                params![
                    pair_id,
                    operation_id,
                    i64::from(*slot),
                    confidence_basis_points,
                    input.now_utc_ms
                ],
            )?;
            for index in indexes {
                let role = match prepared[*index].role {
                    CaptureRecognitionRole::Question => "question",
                    CaptureRecognitionRole::Answer => "answer",
                };
                transaction.execute(
                    "INSERT INTO capture_recognition_pair_items(pair_id, item_id, role)
                     VALUES(?1, ?2, ?3)",
                    params![pair_id, item_ids[*index], role],
                )?;
            }
        }
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
        pair_suggestion_count: u32::try_from(pair_ids.len()).unwrap_or(u32::MAX),
        unmatched_answer_count: u32::try_from(unmatched_answer_count).unwrap_or(u32::MAX),
        stale_suggestion_count: u32::try_from(stale_ids.len()).unwrap_or(u32::MAX),
        detail,
    })
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
