use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::modules::sync_store::{
    WireExportSnapshot, WireProblemAggregate, WireProblemAsset, WireProfile,
};

#[derive(Debug, Error)]
pub enum SyncConflictError {
    #[error("sync conflict database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("sync conflict payload is invalid")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldConflict {
    pub field_name: &'static str,
    pub local_value: Value,
    pub remote_value: Value,
    pub base_revision: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MergeAction<T> {
    ApplyRemote(T),
    ApplyMergedAndEnqueue(T),
    ApplyPartialWithConflicts {
        value: T,
        conflicts: Vec<FieldConflict>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeletionConflict {
    pub profile_id: Option<String>,
    pub local_value: Value,
    pub base_revision: i64,
}

enum FieldDecision {
    Value(Value),
    Conflict { local: Value, remote: Value },
}

fn merge_field(base: Option<&Value>, local: &Value, remote: &Value) -> FieldDecision {
    let Some(base) = base else {
        return if local == remote {
            FieldDecision::Value(remote.clone())
        } else {
            FieldDecision::Conflict {
                local: local.clone(),
                remote: remote.clone(),
            }
        };
    };
    if local == remote || local == base {
        FieldDecision::Value(remote.clone())
    } else if remote == base {
        FieldDecision::Value(local.clone())
    } else {
        FieldDecision::Conflict {
            local: local.clone(),
            remote: remote.clone(),
        }
    }
}

pub(crate) fn merge_remote_problem(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProblemAggregate,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProblemAggregate>, SyncConflictError> {
    let Some(local) = load_local_problem(tx, account_id, &remote.id)? else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base = load_snapshot::<WireProblemAggregate>(tx, account_id, "problem", &remote.id)?;
    let base_revision = base
        .as_ref()
        .map_or(local.revision.min(remote.revision), |value| value.revision);
    let fields = [
        (
            "subject",
            base.as_ref()
                .map(|v| serde_json::to_value(&v.subject))
                .transpose()?,
            serde_json::to_value(&local.subject)?,
            serde_json::to_value(&remote.subject)?,
        ),
        (
            "tags",
            base.as_ref()
                .map(|v| serde_json::to_value(&v.tags))
                .transpose()?,
            serde_json::to_value(&local.tags)?,
            serde_json::to_value(&remote.tags)?,
        ),
        (
            "note",
            base.as_ref()
                .map(|v| serde_json::to_value(&v.note))
                .transpose()?,
            serde_json::to_value(&local.note)?,
            serde_json::to_value(&remote.note)?,
        ),
        (
            "status",
            base.as_ref()
                .map(|v| serde_json::to_value(&v.status))
                .transpose()?,
            serde_json::to_value(&local.status)?,
            serde_json::to_value(&remote.status)?,
        ),
        (
            "timeLimitSeconds",
            base.as_ref()
                .map(|v| serde_json::to_value(v.time_limit_seconds))
                .transpose()?,
            serde_json::to_value(local.time_limit_seconds)?,
            serde_json::to_value(remote.time_limit_seconds)?,
        ),
        (
            "assets",
            base.as_ref()
                .map(|v| serde_json::to_value(&v.assets))
                .transpose()?,
            serde_json::to_value(&local.assets)?,
            serde_json::to_value(&remote.assets)?,
        ),
    ];
    let mut merged = serde_json::to_value(remote)?;
    let merged_object = merged
        .as_object_mut()
        .expect("wire problems always serialize as objects");
    let mut conflicts = Vec::new();
    let mut differs_from_remote = false;
    for (field_name, base_value, local_value, remote_value) in fields {
        match merge_field(base_value.as_ref(), &local_value, &remote_value) {
            FieldDecision::Value(value) => {
                differs_from_remote |= value != remote_value;
                merged_object.insert(field_name.to_owned(), value);
            }
            FieldDecision::Conflict { local, remote } => {
                differs_from_remote = true;
                merged_object.insert(field_name.to_owned(), local.clone());
                conflicts.push(FieldConflict {
                    field_name,
                    local_value: local,
                    remote_value: remote,
                    base_revision,
                });
            }
        }
    }
    let mut merged: WireProblemAggregate = serde_json::from_value(merged)?;
    if !conflicts.is_empty() {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        merged.updated_at_utc_ms = now_utc_ms;
        return Ok(MergeAction::ApplyPartialWithConflicts {
            value: merged,
            conflicts,
        });
    }
    if differs_from_remote {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        merged.updated_at_utc_ms = now_utc_ms;
        Ok(MergeAction::ApplyMergedAndEnqueue(merged))
    } else {
        Ok(MergeAction::ApplyRemote(remote.clone()))
    }
}

pub(crate) fn merge_remote_profile(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProfile,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProfile>, SyncConflictError> {
    let Some(local) = load_local_profile(tx, account_id, &remote.id)? else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base = load_snapshot::<WireProfile>(tx, account_id, "learner_profile", &remote.id)?;
    let base_name = base
        .as_ref()
        .map(|value| serde_json::to_value(&value.name))
        .transpose()?;
    match merge_field(
        base_name.as_ref(),
        &serde_json::to_value(&local.name)?,
        &serde_json::to_value(&remote.name)?,
    ) {
        FieldDecision::Value(value) if value == serde_json::to_value(&remote.name)? => {
            Ok(MergeAction::ApplyRemote(remote.clone()))
        }
        FieldDecision::Value(value) => {
            let mut merged = remote.clone();
            merged.name = serde_json::from_value(value)?;
            merged.revision = local.revision.max(remote.revision).saturating_add(1);
            merged.updated_at_utc_ms = now_utc_ms;
            Ok(MergeAction::ApplyMergedAndEnqueue(merged))
        }
        FieldDecision::Conflict {
            local: local_value,
            remote: remote_value,
        } => {
            let mut merged = local.clone();
            merged.revision = local.revision.max(remote.revision).saturating_add(1);
            merged.updated_at_utc_ms = now_utc_ms;
            Ok(MergeAction::ApplyPartialWithConflicts {
                value: merged,
                conflicts: vec![FieldConflict {
                    field_name: "name",
                    local_value,
                    remote_value,
                    base_revision: base
                        .as_ref()
                        .map_or(local.revision.min(remote.revision), |value| value.revision),
                }],
            })
        }
    }
}

pub(crate) fn merge_remote_export(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireExportSnapshot,
) -> Result<MergeAction<WireExportSnapshot>, SyncConflictError> {
    let Some(local) = load_local_export(tx, account_id, &remote.id)? else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base = load_snapshot::<WireExportSnapshot>(tx, account_id, "export_snapshot", &remote.id)?;
    let base_revision = base
        .as_ref()
        .map_or(local.revision.min(remote.revision), |value| value.revision);
    let mut merged = remote.clone();
    let mut conflicts = Vec::new();
    let mut differs_from_remote = false;
    let fields = [
        (
            "title",
            base.as_ref()
                .map(|value| serde_json::to_value(&value.title))
                .transpose()?,
            serde_json::to_value(&local.title)?,
            serde_json::to_value(&remote.title)?,
        ),
        (
            "problemIds",
            base.as_ref()
                .map(|value| serde_json::to_value(&value.problem_ids))
                .transpose()?,
            serde_json::to_value(&local.problem_ids)?,
            serde_json::to_value(&remote.problem_ids)?,
        ),
        (
            "configuration",
            base.as_ref().map(|value| value.configuration.clone()),
            local.configuration.clone(),
            remote.configuration.clone(),
        ),
    ];
    for (field_name, base_value, local_value, remote_value) in fields {
        match merge_field(base_value.as_ref(), &local_value, &remote_value) {
            FieldDecision::Value(value) => {
                differs_from_remote |= value != remote_value;
                match field_name {
                    "title" => merged.title = serde_json::from_value(value)?,
                    "problemIds" => merged.problem_ids = serde_json::from_value(value)?,
                    "configuration" => merged.configuration = value,
                    _ => unreachable!(),
                }
            }
            FieldDecision::Conflict { local, remote } => {
                differs_from_remote = true;
                match field_name {
                    "title" => merged.title = serde_json::from_value(local.clone())?,
                    "problemIds" => merged.problem_ids = serde_json::from_value(local.clone())?,
                    "configuration" => merged.configuration = local.clone(),
                    _ => unreachable!(),
                }
                conflicts.push(FieldConflict {
                    field_name,
                    local_value: local,
                    remote_value: remote,
                    base_revision,
                });
            }
        }
    }
    if !conflicts.is_empty() {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        return Ok(MergeAction::ApplyPartialWithConflicts {
            value: merged,
            conflicts,
        });
    }
    if differs_from_remote {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        Ok(MergeAction::ApplyMergedAndEnqueue(merged))
    } else {
        Ok(MergeAction::ApplyRemote(remote.clone()))
    }
}

pub(crate) fn store_remote_snapshot<T: Serialize>(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    revision: i64,
    payload: &T,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    tx.execute(
        "INSERT INTO sync_entity_snapshots(
           account_id, profile_id, entity_type, entity_id, revision,
           payload_json, updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(account_id, entity_type, entity_id) DO UPDATE SET
           profile_id = excluded.profile_id,
           revision = excluded.revision,
           payload_json = excluded.payload_json,
           updated_at_utc_ms = excluded.updated_at_utc_ms",
        params![
            account_id,
            profile_id,
            entity_type,
            entity_id,
            revision,
            serde_json::to_string(payload)?,
            now_utc_ms
        ],
    )?;
    Ok(())
}

pub(crate) fn record_conflicts(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    conflicts: &[FieldConflict],
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    for conflict in conflicts {
        let updated = tx.execute(
            "UPDATE sync_conflicts
             SET profile_id = ?1, local_value_json = ?2, remote_value_json = ?3,
                 base_revision = ?4, created_at_utc_ms = ?5
             WHERE account_id = ?6 AND entity_type = ?7 AND entity_id = ?8
               AND field_name = ?9 AND resolved_at_utc_ms IS NULL",
            params![
                profile_id,
                conflict.local_value.to_string(),
                conflict.remote_value.to_string(),
                conflict.base_revision,
                now_utc_ms,
                account_id,
                entity_type,
                entity_id,
                conflict.field_name,
            ],
        )?;
        if updated == 0 {
            tx.execute(
                "INSERT INTO sync_conflicts(
                   id, account_id, profile_id, entity_type, entity_id, field_name,
                   local_value_json, remote_value_json, base_revision, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    Uuid::now_v7().to_string(),
                    account_id,
                    profile_id,
                    entity_type,
                    entity_id,
                    conflict.field_name,
                    conflict.local_value.to_string(),
                    conflict.remote_value.to_string(),
                    conflict.base_revision,
                    now_utc_ms,
                ],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn replace_entity_outbox(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    enqueue: bool,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    tx.execute(
        "DELETE FROM sync_operations
         WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3
           AND operation = 'upsert'",
        params![account_id, entity_type, entity_id],
    )?;
    if enqueue {
        tx.execute(
            "INSERT INTO sync_operations(
               id, account_id, profile_id, entity_type, entity_id, operation,
               payload_json, status, attempt_count, created_at_utc_ms,
               next_attempt_at_utc_ms
             ) VALUES(?1, ?2, ?3, ?4, ?5, 'upsert', '{}', 'pending', 0, ?6, ?6)",
            params![
                Uuid::now_v7().to_string(),
                account_id,
                profile_id,
                entity_type,
                entity_id,
                now_utc_ms
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn deletion_conflict(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_type: &str,
    entity_id: &str,
    deleted_revision: i64,
) -> Result<Option<DeletionConflict>, SyncConflictError> {
    let (profile_id, local_value, local_revision) = match entity_type {
        "learner_profile" => {
            let Some(local) = load_local_profile(tx, account_id, entity_id)? else {
                return Ok(None);
            };
            (
                Some(local.id.clone()),
                serde_json::json!({ "name": local.name }),
                local.revision,
            )
        }
        "problem" => {
            let Some(local) = load_local_problem(tx, account_id, entity_id)? else {
                return Ok(None);
            };
            (
                Some(local.profile_id.clone()),
                problem_content(&local),
                local.revision,
            )
        }
        "export_snapshot" => {
            let Some(local) = load_local_export(tx, account_id, entity_id)? else {
                return Ok(None);
            };
            (
                Some(local.profile_id.clone()),
                export_content(&local),
                local.revision,
            )
        }
        _ => return Ok(None),
    };
    let base_payload = tx
        .query_row(
            "SELECT revision, payload_json FROM sync_entity_snapshots
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, entity_type, entity_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((base_revision, payload)) = base_payload else {
        return Ok(Some(DeletionConflict {
            profile_id,
            local_value,
            base_revision: local_revision.min(deleted_revision),
        }));
    };
    let payload: Value = serde_json::from_str(&payload)?;
    let base_content = match entity_type {
        "learner_profile" => serde_json::json!({
            "name": payload.get("name").cloned().unwrap_or(Value::Null)
        }),
        "problem" => problem_content(&serde_json::from_value(payload)?),
        "export_snapshot" => export_content(&serde_json::from_value(payload)?),
        _ => unreachable!(),
    };
    if local_value == base_content {
        Ok(None)
    } else {
        Ok(Some(DeletionConflict {
            profile_id,
            local_value,
            base_revision,
        }))
    }
}

fn problem_content(value: &WireProblemAggregate) -> Value {
    serde_json::json!({
        "subject": value.subject,
        "tags": value.tags,
        "note": value.note,
        "status": value.status,
        "timeLimitSeconds": value.time_limit_seconds,
        "assets": value.assets,
    })
}

fn export_content(value: &WireExportSnapshot) -> Value {
    serde_json::json!({
        "title": value.title,
        "problemIds": value.problem_ids,
        "configuration": value.configuration,
    })
}

fn load_snapshot<T: DeserializeOwned>(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_type: &str,
    entity_id: &str,
) -> Result<Option<T>, SyncConflictError> {
    let payload = tx
        .query_row(
            "SELECT payload_json FROM sync_entity_snapshots
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, entity_type, entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    payload
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(Into::into)
}

fn load_local_profile(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_id: &str,
) -> Result<Option<WireProfile>, SyncConflictError> {
    Ok(tx
        .query_row(
            "SELECT id, name, revision, created_at_utc_ms, updated_at_utc_ms
             FROM learner_profiles WHERE id = ?1 AND account_id = ?2",
            params![entity_id, account_id],
            |row| {
                Ok(WireProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    revision: row.get(2)?,
                    created_at_utc_ms: row.get(3)?,
                    updated_at_utc_ms: row.get(4)?,
                })
            },
        )
        .optional()?)
}

fn load_local_problem(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_id: &str,
) -> Result<Option<WireProblemAggregate>, SyncConflictError> {
    let problem = tx
        .query_row(
            "SELECT id, profile_id, subject, tags_json, note, status,
                    time_limit_seconds, revision, created_at_utc_ms, updated_at_utc_ms
             FROM problems WHERE id = ?1 AND account_id = ?2",
            params![entity_id, account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()?;
    let Some(problem) = problem else {
        return Ok(None);
    };
    let mut statement = tx.prepare(
        "SELECT asset_id, role, position FROM problem_assets
         WHERE problem_id = ?1
         ORDER BY CASE role WHEN 'question' THEN 0 ELSE 1 END, position, asset_id",
    )?;
    let assets = statement
        .query_map([entity_id], |row| {
            Ok(WireProblemAsset {
                asset_id: row.get(0)?,
                role: row.get(1)?,
                position: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(WireProblemAggregate {
        id: problem.0,
        profile_id: problem.1,
        subject: problem.2,
        tags: serde_json::from_str(&problem.3)?,
        note: problem.4,
        status: if problem.5 == "trashed" {
            "deleted".to_owned()
        } else {
            problem.5
        },
        time_limit_seconds: problem.6,
        assets,
        revision: problem.7,
        created_at_utc_ms: problem.8,
        updated_at_utc_ms: problem.9,
    }))
}

fn load_local_export(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_id: &str,
) -> Result<Option<WireExportSnapshot>, SyncConflictError> {
    let row = tx
        .query_row(
            "SELECT id, profile_id, title, problem_ids_json, configuration_json,
                    revision, created_at_utc_ms
             FROM export_snapshots WHERE id = ?1 AND account_id = ?2",
            params![entity_id, account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| {
        Ok(WireExportSnapshot {
            id: row.0,
            profile_id: row.1,
            title: row.2,
            problem_ids: serde_json::from_str(&row.3)?,
            configuration: serde_json::from_str(&row.4)?,
            revision: row.5,
            created_at_utc_ms: row.6,
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::merge_field;
    use serde_json::json;

    #[test]
    fn field_merge_truth_table_is_deterministic() {
        let base = json!("base");
        assert!(matches!(
            merge_field(Some(&base), &json!("local"), &base),
            super::FieldDecision::Value(value) if value == json!("local")
        ));
        assert!(matches!(
            merge_field(Some(&base), &base, &json!("remote")),
            super::FieldDecision::Value(value) if value == json!("remote")
        ));
        assert!(matches!(
            merge_field(Some(&base), &json!("same"), &json!("same")),
            super::FieldDecision::Value(value) if value == json!("same")
        ));
        assert!(matches!(
            merge_field(Some(&base), &json!("local"), &json!("remote")),
            super::FieldDecision::Conflict { .. }
        ));
    }
}
