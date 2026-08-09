use std::collections::BTreeSet;

use rusqlite::{OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    domain::profile::ProfileName,
    modules::{
        exports::ExportLayout,
        sync_conflict_merge::{export_content, problem_content},
        sync_store::{WireExportSnapshot, WireProblemAggregate, WireProblemAsset, WireProfile},
    },
};

use super::{
    ResolveSyncConflictEntityInput, ResolveSyncConflictFieldInput, SyncConflictChoice,
    SyncConflictError, SyncConflictSummary, cleanup_deleted_profile_sync_state,
    list_sync_conflicts, load_local_export, load_local_problem, load_local_profile,
    replace_entity_outbox,
};

#[derive(Clone, Debug)]
struct ConflictRow {
    id: String,
    profile_id: Option<String>,
    entity_type: String,
    entity_id: String,
    field_name: String,
    local_value: Value,
    remote_value: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidExportConfiguration {
    #[allow(dead_code)]
    layout: ExportLayout,
}

pub fn resolve_sync_conflict_field(
    connection: &mut rusqlite::Connection,
    account_id: &str,
    profile_id: &str,
    input: ResolveSyncConflictFieldInput,
    now_utc_ms: i64,
) -> Result<Vec<SyncConflictSummary>, SyncConflictError> {
    let transaction = connection.transaction()?;
    let row = load_conflict_by_id(&transaction, account_id, profile_id, &input.conflict_id)?;
    let rows = if row.field_name == "__deleted__" && input.choice == SyncConflictChoice::Remote {
        load_conflicts_for_entity(
            &transaction,
            account_id,
            profile_id,
            &row.entity_type,
            &row.entity_id,
        )?
    } else {
        vec![row]
    };
    resolve_rows(
        &transaction,
        account_id,
        profile_id,
        &rows,
        input.choice,
        now_utc_ms,
    )?;
    let remaining = list_sync_conflicts(&transaction, account_id, profile_id)?;
    transaction.commit()?;
    Ok(remaining)
}

pub fn resolve_sync_conflict_entity(
    connection: &mut rusqlite::Connection,
    account_id: &str,
    profile_id: &str,
    input: ResolveSyncConflictEntityInput,
    now_utc_ms: i64,
) -> Result<Vec<SyncConflictSummary>, SyncConflictError> {
    let transaction = connection.transaction()?;
    let rows = load_conflicts_for_entity(
        &transaction,
        account_id,
        profile_id,
        &input.entity_type,
        &input.entity_id,
    )?;
    if rows.is_empty() {
        return Err(SyncConflictError::NotFound);
    }
    resolve_rows(
        &transaction,
        account_id,
        profile_id,
        &rows,
        input.choice,
        now_utc_ms,
    )?;
    let remaining = list_sync_conflicts(&transaction, account_id, profile_id)?;
    transaction.commit()?;
    Ok(remaining)
}

fn load_conflict_by_id(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
    conflict_id: &str,
) -> Result<ConflictRow, SyncConflictError> {
    tx.query_row(
        "SELECT id, profile_id, entity_type, entity_id, field_name,
                local_value_json, remote_value_json
         FROM sync_conflicts
         WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3
           AND resolved_at_utc_ms IS NULL",
        params![conflict_id, account_id, profile_id],
        conflict_row,
    )
    .optional()?
    .ok_or(SyncConflictError::NotFound)
}

fn load_conflicts_for_entity(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
    entity_type: &str,
    entity_id: &str,
) -> Result<Vec<ConflictRow>, SyncConflictError> {
    let mut statement = tx.prepare(
        "SELECT id, profile_id, entity_type, entity_id, field_name,
                local_value_json, remote_value_json
         FROM sync_conflicts
         WHERE account_id = ?1 AND profile_id = ?2 AND entity_type = ?3
           AND entity_id = ?4 AND resolved_at_utc_ms IS NULL
         ORDER BY created_at_utc_ms, field_name, id",
    )?;
    Ok(statement
        .query_map(
            params![account_id, profile_id, entity_type, entity_id],
            conflict_row,
        )?
        .collect::<Result<Vec<_>, _>>()?)
}

fn conflict_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConflictRow> {
    let local_value_json = row.get::<_, String>(5)?;
    let remote_value_json = row.get::<_, String>(6)?;
    let local_value = serde_json::from_str(&local_value_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            local_value_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let remote_value = serde_json::from_str(&remote_value_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            remote_value_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(ConflictRow {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        entity_type: row.get(2)?,
        entity_id: row.get(3)?,
        field_name: row.get(4)?,
        local_value,
        remote_value,
    })
}

fn resolve_rows(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
    rows: &[ConflictRow],
    choice: SyncConflictChoice,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    let first = rows.first().ok_or(SyncConflictError::NotFound)?;
    if rows.iter().any(|row| {
        row.entity_type != first.entity_type
            || row.entity_id != first.entity_id
            || row.profile_id.as_deref() != Some(profile_id)
    }) {
        return Err(SyncConflictError::NotFound);
    }
    let mut accepted_remote_delete = false;
    let mut kept_local_delete = false;
    for row in rows {
        let chosen = match choice {
            SyncConflictChoice::Local => &row.local_value,
            SyncConflictChoice::Remote => &row.remote_value,
        };
        if row.field_name == "__deleted__" {
            if choice == SyncConflictChoice::Remote {
                apply_remote_delete(tx, account_id, row, now_utc_ms)?;
                accepted_remote_delete = true;
            } else {
                kept_local_delete = true;
            }
        } else {
            apply_field_value(tx, account_id, row, chosen, now_utc_ms)?;
        }
        let changed = tx.execute(
            "UPDATE sync_conflicts
             SET resolution = ?1, resolved_value_json = ?2, resolved_at_utc_ms = ?3
             WHERE id = ?4 AND account_id = ?5 AND resolved_at_utc_ms IS NULL",
            params![
                choice.as_str(),
                chosen.to_string(),
                now_utc_ms,
                row.id,
                account_id
            ],
        )?;
        if changed != 1 {
            return Err(SyncConflictError::NotFound);
        }
    }
    let remaining: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sync_conflicts
         WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3
           AND resolved_at_utc_ms IS NULL",
        params![account_id, first.entity_type, first.entity_id],
        |row| row.get(0),
    )?;
    if remaining == 0 {
        if accepted_remote_delete {
            replace_entity_outbox(
                tx,
                account_id,
                first.profile_id.as_deref(),
                &first.entity_type,
                &first.entity_id,
                false,
                now_utc_ms,
            )?;
            tx.execute(
                "DELETE FROM sync_entity_snapshots
                 WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
                params![account_id, first.entity_type, first.entity_id],
            )?;
        } else {
            finalize_resolved_entity(tx, account_id, first, now_utc_ms)?;
            if kept_local_delete {
                tx.execute(
                    "DELETE FROM tombstones
                     WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
                    params![account_id, first.entity_type, first.entity_id],
                )?;
            }
        }
    }
    Ok(())
}

fn apply_field_value(
    tx: &Transaction<'_>,
    account_id: &str,
    row: &ConflictRow,
    value: &Value,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    match row.entity_type.as_str() {
        "learner_profile" if row.field_name == "name" => {
            let name = value.as_str().ok_or(SyncConflictError::InvalidValue)?;
            let name = ProfileName::parse(name).map_err(|_| SyncConflictError::InvalidValue)?;
            tx.execute(
                "UPDATE learner_profiles SET name = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![name.as_str(), now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "problem" => apply_problem_field(tx, account_id, row, value, now_utc_ms)?,
        "export_snapshot" => apply_export_field(tx, account_id, row, value)?,
        _ => return Err(SyncConflictError::InvalidValue),
    }
    Ok(())
}

fn apply_problem_field(
    tx: &Transaction<'_>,
    account_id: &str,
    row: &ConflictRow,
    value: &Value,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    match row.field_name.as_str() {
        "subject" | "note" => {
            let text = value
                .as_str()
                .ok_or(SyncConflictError::InvalidValue)?
                .trim();
            let max = if row.field_name == "subject" {
                40
            } else {
                2_000
            };
            if text.chars().count() > max {
                return Err(SyncConflictError::InvalidValue);
            }
            let column = if row.field_name == "subject" {
                "subject"
            } else {
                "note"
            };
            tx.execute(
                &format!(
                    "UPDATE problems SET {column} = ?1, updated_at_utc_ms = ?2
                     WHERE id = ?3 AND account_id = ?4"
                ),
                params![text, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "tags" => {
            let tags = serde_json::from_value::<Vec<String>>(value.clone())
                .map_err(|_| SyncConflictError::InvalidValue)?;
            if tags.len() > 20 {
                return Err(SyncConflictError::InvalidValue);
            }
            let mut seen = BTreeSet::new();
            let tags = tags
                .into_iter()
                .map(|tag| tag.trim().to_owned())
                .filter(|tag| !tag.is_empty())
                .map(|tag| {
                    if tag.chars().count() > 30 {
                        Err(SyncConflictError::InvalidValue)
                    } else {
                        Ok(tag)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|tag| seen.insert(tag.clone()))
                .collect::<Vec<_>>();
            tx.execute(
                "UPDATE problems SET tags_json = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![
                    serde_json::to_string(&tags)?,
                    now_utc_ms,
                    row.entity_id,
                    account_id
                ],
            )?;
        }
        "status" => {
            let status = value.as_str().ok_or(SyncConflictError::InvalidValue)?;
            let status = match status {
                "active" | "archived" | "trashed" => status,
                "deleted" => "trashed",
                _ => return Err(SyncConflictError::InvalidValue),
            };
            tx.execute(
                "UPDATE problems SET status = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![status, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "timeLimitSeconds" => {
            let seconds = if value.is_null() {
                None
            } else {
                Some(value.as_i64().ok_or(SyncConflictError::InvalidValue)?)
            };
            if seconds.is_some_and(|value| !(1..=86_400).contains(&value)) {
                return Err(SyncConflictError::InvalidValue);
            }
            tx.execute(
                "UPDATE problems SET time_limit_seconds = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![seconds, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "assets" => {
            let links = serde_json::from_value::<Vec<WireProblemAsset>>(value.clone())
                .map_err(|_| SyncConflictError::InvalidValue)?;
            tx.execute(
                "DELETE FROM problem_assets WHERE problem_id = ?1",
                [&row.entity_id],
            )?;
            for link in links {
                if !matches!(link.role.as_str(), "question" | "answer") || link.position < 0 {
                    return Err(SyncConflictError::InvalidValue);
                }
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM assets WHERE id = ?1 AND account_id = ?2
                     )",
                    params![link.asset_id, account_id],
                    |row| row.get(0),
                )?;
                if !exists {
                    return Err(SyncConflictError::InvalidValue);
                }
                tx.execute(
                    "INSERT INTO problem_assets(problem_id, asset_id, role, position)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![row.entity_id, link.asset_id, link.role, link.position],
                )?;
            }
        }
        _ => return Err(SyncConflictError::InvalidValue),
    }
    Ok(())
}

fn apply_export_field(
    tx: &Transaction<'_>,
    account_id: &str,
    row: &ConflictRow,
    value: &Value,
) -> Result<(), SyncConflictError> {
    match row.field_name.as_str() {
        "title" => {
            let title = value
                .as_str()
                .ok_or(SyncConflictError::InvalidValue)?
                .trim();
            if title.is_empty() || title.chars().count() > 80 {
                return Err(SyncConflictError::InvalidValue);
            }
            tx.execute(
                "UPDATE export_snapshots SET title = ?1
                 WHERE id = ?2 AND account_id = ?3",
                params![title, row.entity_id, account_id],
            )?;
        }
        "problemIds" => {
            let ids = serde_json::from_value::<Vec<String>>(value.clone())
                .map_err(|_| SyncConflictError::InvalidValue)?;
            let mut seen = BTreeSet::new();
            let ids = ids
                .into_iter()
                .filter(|id| seen.insert(id.clone()))
                .collect::<Vec<_>>();
            if ids.is_empty() || ids.len() > 500 {
                return Err(SyncConflictError::InvalidValue);
            }
            let profile_id = row
                .profile_id
                .as_deref()
                .ok_or(SyncConflictError::InvalidValue)?;
            for id in &ids {
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM problems
                       WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3
                         AND status != 'trashed'
                     )",
                    params![id, account_id, profile_id],
                    |record| record.get(0),
                )?;
                if !exists {
                    return Err(SyncConflictError::InvalidValue);
                }
            }
            tx.execute(
                "UPDATE export_snapshots SET problem_ids_json = ?1
                 WHERE id = ?2 AND account_id = ?3",
                params![serde_json::to_string(&ids)?, row.entity_id, account_id],
            )?;
        }
        "configuration" => {
            serde_json::from_value::<ValidExportConfiguration>(value.clone())
                .map_err(|_| SyncConflictError::InvalidValue)?;
            tx.execute(
                "UPDATE export_snapshots SET configuration_json = ?1
                 WHERE id = ?2 AND account_id = ?3",
                params![value.to_string(), row.entity_id, account_id],
            )?;
        }
        _ => return Err(SyncConflictError::InvalidValue),
    }
    Ok(())
}

fn apply_remote_delete(
    tx: &Transaction<'_>,
    account_id: &str,
    row: &ConflictRow,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    let deleted_revision = tx
        .query_row(
            "SELECT revision FROM tombstones
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, row.entity_type, row.entity_id],
            |record| record.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(SyncConflictError::NotFound)?;
    match row.entity_type.as_str() {
        "problem" => {
            tx.execute(
                "UPDATE problems
                 SET status = 'trashed', revision = max(revision, ?1),
                     updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![deleted_revision, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "export_snapshot" => {
            tx.execute(
                "DELETE FROM export_snapshots WHERE id = ?1 AND account_id = ?2",
                params![row.entity_id, account_id],
            )?;
        }
        "learner_profile" => {
            let replacement = tx
                .query_row(
                    "SELECT id FROM learner_profiles
                     WHERE account_id = ?1 AND id <> ?2
                     ORDER BY created_at_utc_ms, id LIMIT 1",
                    params![account_id, row.entity_id],
                    |record| record.get::<_, String>(0),
                )
                .optional()?
                .ok_or(SyncConflictError::LastProfile)?;
            tx.execute(
                "UPDATE account_preferences
                 SET active_profile_id = CASE
                   WHEN active_profile_id = ?1 THEN ?2 ELSE active_profile_id END,
                   updated_at_utc_ms = ?3
                 WHERE account_id = ?4",
                params![row.entity_id, replacement, now_utc_ms, account_id],
            )?;
            cleanup_deleted_profile_sync_state(tx, account_id, &row.entity_id, true, now_utc_ms)?;
            tx.execute(
                "DELETE FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
                params![row.entity_id, account_id],
            )?;
        }
        _ => return Err(SyncConflictError::InvalidValue),
    }
    Ok(())
}

fn finalize_resolved_entity(
    tx: &Transaction<'_>,
    account_id: &str,
    row: &ConflictRow,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    let snapshot = tx
        .query_row(
            "SELECT revision, payload_json FROM sync_entity_snapshots
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, row.entity_type, row.entity_id],
            |record| Ok((record.get::<_, i64>(0)?, record.get::<_, String>(1)?)),
        )
        .optional()?;
    let tombstone_revision = tx
        .query_row(
            "SELECT revision FROM tombstones
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, row.entity_type, row.entity_id],
            |record| record.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let remote_revision = snapshot
        .as_ref()
        .map_or(tombstone_revision, |value| value.0.max(tombstone_revision));
    let (profile_id, local_revision, local_content, remote_content) = match row.entity_type.as_str()
    {
        "learner_profile" => {
            let local = load_local_profile(tx, account_id, &row.entity_id)?
                .ok_or(SyncConflictError::NotFound)?;
            let remote = snapshot
                .as_ref()
                .map(|value| serde_json::from_str::<WireProfile>(&value.1))
                .transpose()?;
            (
                Some(local.id.clone()),
                local.revision,
                serde_json::json!({ "name": local.name }),
                remote.map(|value| serde_json::json!({ "name": value.name })),
            )
        }
        "problem" => {
            let local = load_local_problem(tx, account_id, &row.entity_id)?
                .ok_or(SyncConflictError::NotFound)?;
            let remote = snapshot
                .as_ref()
                .map(|value| serde_json::from_str::<WireProblemAggregate>(&value.1))
                .transpose()?;
            (
                Some(local.profile_id.clone()),
                local.revision,
                problem_content(&local),
                remote.map(|value| problem_content(&value)),
            )
        }
        "export_snapshot" => {
            let local = load_local_export(tx, account_id, &row.entity_id)?
                .ok_or(SyncConflictError::NotFound)?;
            let remote = snapshot
                .as_ref()
                .map(|value| serde_json::from_str::<WireExportSnapshot>(&value.1))
                .transpose()?;
            (
                Some(local.profile_id.clone()),
                local.revision,
                export_content(&local),
                remote.map(|value| export_content(&value)),
            )
        }
        _ => return Err(SyncConflictError::InvalidValue),
    };
    let needs_push = remote_content.as_ref() != Some(&local_content);
    let revision = if needs_push {
        local_revision.max(remote_revision).saturating_add(1)
    } else {
        remote_revision
    };
    match row.entity_type.as_str() {
        "learner_profile" => {
            tx.execute(
                "UPDATE learner_profiles
                 SET revision = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![revision, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "problem" => {
            tx.execute(
                "UPDATE problems SET revision = ?1, updated_at_utc_ms = ?2
                 WHERE id = ?3 AND account_id = ?4",
                params![revision, now_utc_ms, row.entity_id, account_id],
            )?;
        }
        "export_snapshot" => {
            tx.execute(
                "UPDATE export_snapshots SET revision = ?1
                 WHERE id = ?2 AND account_id = ?3",
                params![revision, row.entity_id, account_id],
            )?;
        }
        _ => unreachable!(),
    }
    replace_entity_outbox(
        tx,
        account_id,
        profile_id.as_deref(),
        &row.entity_type,
        &row.entity_id,
        needs_push,
        now_utc_ms,
    )?;
    Ok(())
}
