use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

use super::{
    SyncPullError, asset_staging::StagedAsset, sync_pull_decoder::DecodedChange, validate_uuid,
};
use crate::{
    application::ports::assets::AssetBlobRemover,
    modules::{
        review::rebuild_schedule_for_problem,
        sync_conflict_merge::{
            FieldConflict, MergeAction, export_content, problem_content, profile_content,
        },
        sync_conflicts::{
            cleanup_deleted_profile_sync_state, deletion_conflict, merge_remote_export,
            merge_remote_problem, merge_remote_profile, record_conflicts, replace_entity_outbox,
            store_remote_snapshot,
        },
        sync_store::{
            WireAsset, WireExportSnapshot, WireProblemAggregate, WireProfile, WireReviewEvent,
            WireTombstone, record_pull_success_tx,
        },
    },
};

pub(super) fn apply_page(
    connection: &mut Connection,
    account_id: &str,
    changes: &[DecodedChange],
    staged_assets: &mut [StagedAsset],
    blob_root: &Path,
    asset_blob_remover: &dyn AssetBlobRemover,
    page_cursor: i64,
    now_utc_ms: i64,
) -> Result<usize, SyncPullError> {
    let transaction = connection.transaction()?;
    let mut asset_ids = HashMap::<String, String>::new();
    let mut staged_by_id = staged_assets
        .iter_mut()
        .map(|item| (item.asset().id.clone(), item))
        .collect::<HashMap<_, _>>();
    for change in changes {
        if let DecodedChange::Asset(asset) = change {
            let local_id = upsert_asset(
                &transaction,
                account_id,
                asset,
                staged_by_id
                    .get_mut(asset.id.as_str())
                    .map(|staged| &mut **staged),
            )?;
            asset_ids.insert(asset.id.clone(), local_id);
        }
    }
    let mut affected_problems = HashSet::new();
    let mut orphan_blob_paths = Vec::new();
    for change in changes {
        match change {
            DecodedChange::Profile(profile) => {
                apply_profile_merge(&transaction, account_id, profile, now_utc_ms)?
            }
            DecodedChange::Asset(_) => {}
            DecodedChange::Problem(problem) => {
                let local_id =
                    apply_problem_merge(&transaction, account_id, problem, &asset_ids, now_utc_ms)?;
                affected_problems.insert(local_id);
            }
            DecodedChange::Review(event) => {
                insert_review_event(&transaction, account_id, event)?;
                affected_problems.insert(event.problem_id.clone());
            }
            DecodedChange::Export(snapshot) => {
                apply_export_merge(&transaction, account_id, snapshot, now_utc_ms)?
            }
            DecodedChange::Tombstone(tombstone) => {
                if let Some(relative_path) =
                    apply_tombstone_merge(&transaction, account_id, tombstone, now_utc_ms)?
                {
                    orphan_blob_paths.push(relative_path);
                }
                if tombstone.entity_type == "problem" {
                    affected_problems.insert(tombstone.entity_id.clone());
                }
            }
        }
    }
    for problem_id in &affected_problems {
        rebuild_schedule_for_problem(&transaction, account_id, problem_id, now_utc_ms)?;
    }
    record_pull_success_tx(&transaction, account_id, page_cursor, now_utc_ms)?;
    transaction.commit()?;
    for relative_path in orphan_blob_paths {
        remove_orphan_blob(asset_blob_remover, blob_root, &relative_path);
    }
    Ok(changes.len())
}

fn apply_profile_merge(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProfile,
    now_utc_ms: i64,
) -> Result<(), SyncPullError> {
    let action = merge_remote_profile(tx, account_id, remote, now_utc_ms)?;
    match action {
        MergeAction::ApplyRemote(value) => {
            upsert_profile(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                &[],
                &profile_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                false,
                now_utc_ms,
            )?;
        }
        MergeAction::ApplyMergedAndEnqueue(value) => {
            upsert_profile(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                &[],
                &profile_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                true,
                now_utc_ms,
            )?;
        }
        MergeAction::ApplyPartialWithConflicts { value, conflicts } => {
            upsert_profile(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                &conflicts,
                &profile_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.id),
                "learner_profile",
                &value.id,
                false,
                now_utc_ms,
            )?;
        }
    }
    store_remote_snapshot(
        tx,
        account_id,
        Some(&remote.id),
        "learner_profile",
        &remote.id,
        remote.revision,
        remote,
        now_utc_ms,
    )?;
    Ok(())
}

fn apply_problem_merge(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProblemAggregate,
    asset_ids: &HashMap<String, String>,
    now_utc_ms: i64,
) -> Result<String, SyncPullError> {
    let action = merge_remote_problem(tx, account_id, remote, now_utc_ms)?;
    let local_id = match action {
        MergeAction::ApplyRemote(value) => {
            let id = upsert_problem(tx, account_id, &value, asset_ids)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                &[],
                &problem_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                false,
                now_utc_ms,
            )?;
            id
        }
        MergeAction::ApplyMergedAndEnqueue(value) => {
            let id = upsert_problem(tx, account_id, &value, asset_ids)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                &[],
                &problem_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                true,
                now_utc_ms,
            )?;
            id
        }
        MergeAction::ApplyPartialWithConflicts { value, conflicts } => {
            let id = upsert_problem(tx, account_id, &value, asset_ids)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                &conflicts,
                &problem_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "problem",
                &value.id,
                false,
                now_utc_ms,
            )?;
            id
        }
    };
    store_remote_snapshot(
        tx,
        account_id,
        Some(&remote.profile_id),
        "problem",
        &remote.id,
        remote.revision,
        remote,
        now_utc_ms,
    )?;
    Ok(local_id)
}

fn apply_export_merge(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireExportSnapshot,
    now_utc_ms: i64,
) -> Result<(), SyncPullError> {
    let action = merge_remote_export(tx, account_id, remote)?;
    match action {
        MergeAction::ApplyRemote(value) => {
            upsert_export(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                &[],
                &export_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                false,
                now_utc_ms,
            )?;
        }
        MergeAction::ApplyMergedAndEnqueue(value) => {
            upsert_export(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                &[],
                &export_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                true,
                now_utc_ms,
            )?;
        }
        MergeAction::ApplyPartialWithConflicts { value, conflicts } => {
            upsert_export(tx, account_id, &value)?;
            record_conflicts(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                &conflicts,
                &export_content(&value),
                now_utc_ms,
            )?;
            replace_entity_outbox(
                tx,
                account_id,
                Some(&value.profile_id),
                "export_snapshot",
                &value.id,
                false,
                now_utc_ms,
            )?;
        }
    }
    store_remote_snapshot(
        tx,
        account_id,
        Some(&remote.profile_id),
        "export_snapshot",
        &remote.id,
        remote.revision,
        remote,
        now_utc_ms,
    )?;
    Ok(())
}

fn upsert_profile(
    tx: &Transaction<'_>,
    account_id: &str,
    profile: &WireProfile,
) -> Result<(), SyncPullError> {
    validate_uuid(&profile.id)?;
    tx.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at_utc_ms = excluded.updated_at_utc_ms, revision = excluded.revision
         WHERE learner_profiles.account_id = excluded.account_id AND excluded.revision > learner_profiles.revision",
        params![profile.id, account_id, profile.name, profile.created_at_utc_ms, profile.updated_at_utc_ms, profile.revision],
    )?;
    Ok(())
}

fn upsert_asset(
    tx: &Transaction<'_>,
    account_id: &str,
    asset: &WireAsset,
    staged: Option<&mut StagedAsset>,
) -> Result<String, SyncPullError> {
    let existing = tx.query_row(
        "SELECT id, plaintext_sha256, byte_length, media_type FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
        params![account_id, asset.plaintext_sha256],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?)),
    ).optional()?;
    if let Some((id, hash, length, media_type)) = existing {
        if id != asset.id
            || hash != asset.plaintext_sha256
            || length != asset.byte_length
            || media_type != asset.media_type
        {
            return Err(SyncPullError::AssetMismatch);
        }
        return Ok(id);
    }
    let staged = staged.ok_or(SyncPullError::InvalidAsset)?;
    staged.promote()?;
    tx.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![asset.id, account_id, asset.plaintext_sha256, staged.relative_path(), asset.byte_length, asset.media_type, asset.created_at_utc_ms],
    )?;
    Ok(asset.id.clone())
}

fn upsert_problem(
    tx: &Transaction<'_>,
    account_id: &str,
    problem: &WireProblemAggregate,
    asset_ids: &HashMap<String, String>,
) -> Result<String, SyncPullError> {
    validate_uuid(&problem.id)?;
    validate_uuid(&problem.profile_id)?;
    let profile_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![problem.profile_id, account_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(SyncPullError::InvalidChange);
    }
    let changed = tx.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, tags_json, note, status, time_limit_seconds, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET subject=excluded.subject, tags_json=excluded.tags_json, note=excluded.note, status=excluded.status, time_limit_seconds=excluded.time_limit_seconds, updated_at_utc_ms=excluded.updated_at_utc_ms, revision=excluded.revision
         WHERE problems.account_id = excluded.account_id AND excluded.revision > problems.revision",
        params![problem.id, account_id, problem.profile_id, problem.subject, serde_json::to_string(&problem.tags).map_err(|_| SyncPullError::InvalidChange)?, problem.note, problem.status, problem.time_limit_seconds, problem.created_at_utc_ms, problem.updated_at_utc_ms, problem.revision],
    )?;
    if changed == 0 {
        return Ok(problem.id.clone());
    }
    tx.execute(
        "DELETE FROM problem_assets WHERE problem_id = ?1",
        [&problem.id],
    )?;
    for link in &problem.assets {
        validate_uuid(&link.asset_id)?;
        let asset_id = asset_ids
            .get(&link.asset_id)
            .cloned()
            .unwrap_or_else(|| link.asset_id.clone());
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1 AND account_id = ?2)",
            params![asset_id, account_id],
            |row| row.get(0),
        )?;
        if !exists || !matches!(link.role.as_str(), "question" | "answer") || link.position < 0 {
            return Err(SyncPullError::InvalidChange);
        }
        tx.execute("INSERT INTO problem_assets(problem_id, asset_id, role, position) VALUES(?1, ?2, ?3, ?4)", params![problem.id, asset_id, link.role, link.position])?;
    }
    Ok(problem.id.clone())
}

fn insert_review_event(
    tx: &Transaction<'_>,
    account_id: &str,
    event: &WireReviewEvent,
) -> Result<(), SyncPullError> {
    for id in [
        &event.id,
        &event.profile_id,
        &event.problem_id,
        &event.device_id,
    ] {
        validate_uuid(id)?;
    }
    if !matches!(event.rating.as_str(), "again" | "hard" | "good" | "easy") || event.duration_ms < 0
    {
        return Err(SyncPullError::InvalidChange);
    }
    tx.execute(
        "INSERT OR IGNORE INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![event.id, account_id, event.profile_id, event.problem_id, event.device_id, event.rating, event.duration_ms, event.occurred_at_utc_ms, event.algorithm_version, event.parameter_version],
    )?;
    Ok(())
}

fn upsert_export(
    tx: &Transaction<'_>,
    account_id: &str,
    snapshot: &WireExportSnapshot,
) -> Result<(), SyncPullError> {
    validate_uuid(&snapshot.id)?;
    validate_uuid(&snapshot.profile_id)?;
    tx.execute(
        "INSERT INTO export_snapshots(id, account_id, profile_id, title, problem_ids_json, configuration_json, created_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title, problem_ids_json=excluded.problem_ids_json, configuration_json=excluded.configuration_json, revision=excluded.revision
         WHERE export_snapshots.account_id = excluded.account_id AND excluded.revision > export_snapshots.revision",
        params![snapshot.id, account_id, snapshot.profile_id, snapshot.title, serde_json::to_string(&snapshot.problem_ids).map_err(|_| SyncPullError::InvalidChange)?, snapshot.configuration.to_string(), snapshot.created_at_utc_ms, snapshot.revision],
    )?;
    Ok(())
}

fn apply_tombstone_merge(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
    now_utc_ms: i64,
) -> Result<Option<String>, SyncPullError> {
    if let Some(pending) = deletion_conflict(
        tx,
        account_id,
        &tombstone.entity_type,
        &tombstone.entity_id,
        tombstone.deleted_revision,
    )? {
        upsert_tombstone_row(tx, account_id, tombstone)?;
        let local_value = pending.local_value;
        record_conflicts(
            tx,
            account_id,
            pending.profile_id.as_deref(),
            &tombstone.entity_type,
            &tombstone.entity_id,
            &[FieldConflict {
                field_name: "__deleted__",
                local_value: local_value.clone(),
                remote_value: Value::Bool(true),
                base_revision: pending.base_revision,
            }],
            &local_value,
            now_utc_ms,
        )?;
        replace_entity_outbox(
            tx,
            account_id,
            pending.profile_id.as_deref(),
            &tombstone.entity_type,
            &tombstone.entity_id,
            false,
            now_utc_ms,
        )?;
        return Ok(None);
    }
    let removed = apply_tombstone(tx, account_id, tombstone)?;
    tx.execute(
        "DELETE FROM sync_entity_snapshots
         WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
        params![account_id, tombstone.entity_type, tombstone.entity_id],
    )?;
    Ok(removed)
}

fn apply_tombstone(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
) -> Result<Option<String>, SyncPullError> {
    upsert_tombstone_row(tx, account_id, tombstone)?;
    match tombstone.entity_type.as_str() {
        "problem" => {
            tx.execute(
                "UPDATE problems SET status = 'trashed', updated_at_utc_ms = ?1, revision = max(revision, ?2) WHERE id = ?3 AND account_id = ?4",
                params![tombstone.deleted_at_utc_ms, tombstone.deleted_revision, tombstone.entity_id, account_id],
            )?;
        }
        "learner_profile" => apply_profile_tombstone(tx, account_id, tombstone)?,
        "asset" => return apply_asset_tombstone(tx, account_id, tombstone),
        "export_snapshot" => {
            tx.execute(
                "DELETE FROM sync_operations
                 WHERE account_id = ?1 AND entity_type = 'export_snapshot' AND entity_id = ?2",
                params![account_id, tombstone.entity_id],
            )?;
            tx.execute(
                "DELETE FROM export_snapshots WHERE id = ?1 AND account_id = ?2",
                params![tombstone.entity_id, account_id],
            )?;
        }
        "review_event" => {}
        _ => return Err(SyncPullError::InvalidChange),
    }
    Ok(None)
}

fn upsert_tombstone_row(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
) -> Result<(), SyncPullError> {
    validate_uuid(&tombstone.tombstone_id)?;
    validate_uuid(&tombstone.entity_id)?;
    if let Some(profile_id) = &tombstone.profile_id {
        validate_uuid(profile_id)?;
    }
    if tombstone.purge_after_utc_ms <= tombstone.deleted_at_utc_ms {
        return Err(SyncPullError::InvalidChange);
    }
    tx.execute(
        "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET deleted_at_utc_ms=excluded.deleted_at_utc_ms, purge_after_utc_ms=excluded.purge_after_utc_ms, revision=excluded.revision
         WHERE tombstones.account_id = excluded.account_id AND excluded.revision > tombstones.revision",
        params![tombstone.tombstone_id, account_id, tombstone.profile_id, tombstone.entity_type, tombstone.entity_id, tombstone.deleted_at_utc_ms, tombstone.purge_after_utc_ms, tombstone.deleted_revision],
    )?;
    Ok(())
}

fn apply_profile_tombstone(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
) -> Result<(), SyncPullError> {
    if tombstone.profile_id.is_some() {
        return Err(SyncPullError::InvalidChange);
    }
    let target_exists = tx
        .query_row(
            "SELECT revision FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
            params![tombstone.entity_id, account_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(_) = target_exists else {
        return Ok(());
    };
    let replacement = tx
        .query_row(
            "SELECT id FROM learner_profiles
             WHERE account_id = ?1 AND id <> ?2
             ORDER BY created_at_utc_ms, id LIMIT 1",
            params![account_id, tombstone.entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(SyncPullError::InvalidChange)?;
    tx.execute(
        "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
         VALUES(?3, ?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET
           active_profile_id = CASE
             WHEN account_preferences.active_profile_id = ?4 THEN excluded.active_profile_id
             ELSE account_preferences.active_profile_id
           END,
           updated_at_utc_ms = CASE
             WHEN account_preferences.active_profile_id = ?4 THEN excluded.updated_at_utc_ms
             ELSE account_preferences.updated_at_utc_ms
           END",
        params![
            replacement,
            tombstone.deleted_at_utc_ms,
            account_id,
            tombstone.entity_id
        ],
    )?;
    cleanup_deleted_profile_sync_state(
        tx,
        account_id,
        &tombstone.entity_id,
        false,
        tombstone.deleted_at_utc_ms,
    )?;
    tx.execute(
        "DELETE FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
        params![tombstone.entity_id, account_id],
    )?;
    Ok(())
}

fn apply_asset_tombstone(
    tx: &Transaction<'_>,
    account_id: &str,
    tombstone: &WireTombstone,
) -> Result<Option<String>, SyncPullError> {
    if tombstone.profile_id.is_some() {
        return Err(SyncPullError::InvalidChange);
    }
    let relative_path = tx
        .query_row(
            "SELECT encrypted_path FROM assets
             WHERE id = ?1 AND account_id = ?2
               AND NOT EXISTS(SELECT 1 FROM problem_assets WHERE asset_id = ?1)
               AND NOT EXISTS(SELECT 1 FROM capture_items WHERE asset_id = ?1)",
            params![tombstone.entity_id, account_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(relative_path) = relative_path else {
        return Ok(None);
    };
    tx.execute(
        "DELETE FROM sync_operations
         WHERE account_id = ?1 AND entity_type = 'asset' AND entity_id = ?2",
        params![account_id, tombstone.entity_id],
    )?;
    tx.execute(
        "DELETE FROM assets WHERE id = ?1 AND account_id = ?2",
        params![tombstone.entity_id, account_id],
    )?;
    Ok(Some(relative_path))
}

fn remove_orphan_blob(
    asset_blob_remover: &dyn AssetBlobRemover,
    blob_root: &Path,
    relative_path: &str,
) {
    let _ = asset_blob_remover.remove(blob_root, relative_path);
}
