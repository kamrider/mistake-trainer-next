use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use uuid::Uuid;

use super::{LegacyImportError, LegacyRollbackReceipt, legacy_scan::is_safe_relative_path};

const TOMBSTONE_RETENTION_MILLIS: i64 = 30 * 86_400_000;

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
    let mut removed_entities = Vec::<RemovedLegacyEntity>::new();
    let sync_ids = import_entity_ids(&transaction, import_id, "sync_operation", true)?;
    for id in sync_ids {
        transaction.execute("DELETE FROM sync_operations WHERE id = ?1", [&id])?;
    }
    let review_ids = import_entity_ids(&transaction, import_id, "review_event", true)?;

    let snapshot_ids = import_entity_ids(&transaction, import_id, "export_snapshot", true)?;
    let mut removed_snapshots = 0_i32;
    for id in snapshot_ids {
        let metadata = transaction
            .query_row(
                "SELECT profile_id, revision FROM export_snapshots
                 WHERE id = ?1 AND account_id = ?2",
                params![id, account_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let removed = i32::try_from(transaction.execute(
            "DELETE FROM export_snapshots WHERE id = ?1 AND account_id = ?2 AND revision = 1",
            params![id, account_id],
        )?)
        .unwrap_or(i32::MAX);
        removed_snapshots = removed_snapshots.saturating_add(removed);
        if removed == 1 {
            let (profile_id, revision) = metadata.ok_or(LegacyImportError::UnsafeSource)?;
            removed_entities.push(RemovedLegacyEntity {
                entity_type: "export_snapshot",
                entity_id: id,
                profile_id: Some(profile_id),
                revision,
            });
        }
    }

    let problem_ids = import_entity_ids(&transaction, import_id, "problem", true)?;
    let mut removed_problem_count = 0_i32;
    let mut removed_review_count = 0_i32;
    for id in &problem_ids {
        let metadata = transaction
            .query_row(
                "SELECT profile_id, revision FROM problems
                 WHERE id = ?1 AND account_id = ?2",
                params![id, account_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let Some((profile_id, revision)) = metadata else {
            continue;
        };
        let has_non_import_review: bool = transaction.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM review_events e
               WHERE e.problem_id = ?1 AND e.account_id = ?2
                 AND NOT EXISTS(
                   SELECT 1 FROM legacy_import_entities lie
                   WHERE lie.import_id = ?3 AND lie.entity_type = 'review_event'
                     AND lie.entity_id = e.id AND lie.created_by_import = 1
                 )
             )",
            params![id, account_id, import_id],
            |row| row.get(0),
        )?;
        let has_snapshot_reference: bool = transaction.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM export_snapshots s, json_each(s.problem_ids_json) j
               WHERE s.account_id = ?2 AND j.value = ?1
             )",
            params![id, account_id],
            |row| row.get(0),
        )?;
        if revision != 1 || has_non_import_review || has_snapshot_reference {
            continue;
        }

        for review_id in &review_ids {
            let review_profile = transaction
                .query_row(
                    "SELECT profile_id FROM review_events
                     WHERE id = ?1 AND problem_id = ?2 AND account_id = ?3",
                    params![review_id, id, account_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(review_profile) = review_profile {
                let removed = transaction.execute(
                    "DELETE FROM review_events
                     WHERE id = ?1 AND problem_id = ?2 AND account_id = ?3",
                    params![review_id, id, account_id],
                )?;
                if removed == 1 {
                    removed_review_count = removed_review_count.saturating_add(1);
                    removed_entities.push(RemovedLegacyEntity {
                        entity_type: "review_event",
                        entity_id: review_id.clone(),
                        profile_id: Some(review_profile),
                        revision: 1,
                    });
                }
            }
        }

        let removed = i32::try_from(transaction.execute(
            "DELETE FROM problems
                 WHERE id = ?1 AND account_id = ?2 AND revision = 1
                   AND NOT EXISTS(SELECT 1 FROM review_events WHERE problem_id = ?1)
                   AND NOT EXISTS(
                     SELECT 1 FROM export_snapshots s, json_each(s.problem_ids_json) j
                     WHERE j.value = ?1
                   )",
            params![id, account_id],
        )?)
        .unwrap_or(i32::MAX);
        removed_problem_count = removed_problem_count.saturating_add(removed);
        if removed == 1 {
            removed_entities.push(RemovedLegacyEntity {
                entity_type: "problem",
                entity_id: id.clone(),
                profile_id: Some(profile_id),
                revision,
            });
        }
    }

    let profile_ids = import_entity_ids(&transaction, import_id, "profile", true)?;
    let mut removed_profile_count = 0_i32;
    for id in &profile_ids {
        let revision = transaction
            .query_row(
                "SELECT revision FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
                params![id, account_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let removed = i32::try_from(transaction.execute(
            "DELETE FROM learner_profiles
                 WHERE id = ?1 AND account_id = ?2 AND revision = 1
                   AND NOT EXISTS(SELECT 1 FROM problems WHERE profile_id = ?1)
                   AND NOT EXISTS(SELECT 1 FROM capture_batches WHERE profile_id = ?1)
                   AND NOT EXISTS(SELECT 1 FROM account_preferences WHERE active_profile_id = ?1)",
            params![id, account_id],
        )?)
        .unwrap_or(i32::MAX);
        removed_profile_count = removed_profile_count.saturating_add(removed);
        if removed == 1 {
            removed_entities.push(RemovedLegacyEntity {
                entity_type: "learner_profile",
                entity_id: id.clone(),
                profile_id: Some(id.clone()),
                revision: revision.ok_or(LegacyImportError::UnsafeSource)?,
            });
        }
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
            let removed = i32::try_from(transaction.execute(
                "DELETE FROM assets WHERE id = ?1 AND account_id = ?2
                     AND NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)
                     AND NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)",
                params![id, account_id],
            )?)
            .unwrap_or(i32::MAX);
            removed_asset_count = removed_asset_count.saturating_add(removed);
            if removed == 1 {
                removed_entities.push(RemovedLegacyEntity {
                    entity_type: "asset",
                    entity_id: id.clone(),
                    profile_id: None,
                    revision: 1,
                });
            }
        }
        for entity in &removed_entities {
            enqueue_legacy_rollback_deletion(&transaction, account_id, entity, now_utc_ms)?;
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
            )
            .saturating_add(
                i32::try_from(review_ids.len())
                    .unwrap_or(i32::MAX)
                    .saturating_sub(removed_review_count),
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

struct RemovedLegacyEntity {
    entity_type: &'static str,
    entity_id: String,
    profile_id: Option<String>,
    revision: i64,
}

fn enqueue_legacy_rollback_deletion(
    transaction: &Transaction<'_>,
    account_id: &str,
    entity: &RemovedLegacyEntity,
    now_utc_ms: i64,
) -> Result<(), LegacyImportError> {
    let revision = entity.revision.saturating_add(1).max(1);
    let purge_after_utc_ms = now_utc_ms.saturating_add(TOMBSTONE_RETENTION_MILLIS);
    transaction.execute(
        "INSERT INTO tombstones(
           id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms,
           purge_after_utc_ms, revision
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
           deleted_at_utc_ms = excluded.deleted_at_utc_ms,
           purge_after_utc_ms = excluded.purge_after_utc_ms,
           revision = MAX(tombstones.revision, excluded.revision)",
        params![
            Uuid::now_v7().to_string(),
            account_id,
            entity.profile_id,
            entity.entity_type,
            entity.entity_id,
            now_utc_ms,
            purge_after_utc_ms,
            revision,
        ],
    )?;
    let payload = serde_json::to_string(&serde_json::json!({
        "id": &entity.entity_id,
        "deletedAtUtcMs": now_utc_ms,
        "purgeAfterUtcMs": purge_after_utc_ms,
        "revision": revision,
        "source": "legacy-import-rollback",
    }))?;
    transaction.execute(
        "INSERT INTO sync_operations(
           id, account_id, profile_id, entity_type, entity_id, operation, payload_json,
           status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, 'delete', ?6, 'pending', 0, ?7, ?7)",
        params![
            Uuid::now_v7().to_string(),
            account_id,
            entity.profile_id,
            entity.entity_type,
            entity.entity_id,
            payload,
            now_utc_ms,
        ],
    )?;
    Ok(())
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
    statement
        .query_map(params![import_id, entity_type, i64::from(created)], |row| {
            row.get(0)
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn restore_quarantined_assets(paths: &[(PathBuf, PathBuf)]) {
    for (original, staged) in paths.iter().rev() {
        if let Some(parent) = original.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::rename(staged, original);
    }
}
