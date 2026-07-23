use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    domain::profile::ProfileName,
    modules::{
        exports::ExportLayout,
        sync_store::{WireExportSnapshot, WireProblemAggregate, WireProblemAsset, WireProfile},
    },
};

#[derive(Debug, Error)]
pub enum SyncConflictError {
    #[error("sync conflict database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("sync conflict payload is invalid")]
    Json(#[from] serde_json::Error),
    #[error("sync conflict was not found")]
    NotFound,
    #[error("sync conflict value is invalid")]
    InvalidValue,
    #[error("the last profile cannot be deleted")]
    LastProfile,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictChoice {
    Local,
    Remote,
}

impl SyncConflictChoice {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    fn from_serde(value: Value) -> Result<Self, SyncConflictError> {
        Ok(match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(value),
            Value::Number(value) => Self::Number(value.to_string()),
            Value::String(value) => Self::String(value),
            Value::Array(values) => Self::Array(
                values
                    .into_iter()
                    .map(Self::from_serde)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Value::Object(values) => Self::Object(
                values
                    .into_iter()
                    .map(|(key, value)| Ok((key, Self::from_serde(value)?)))
                    .collect::<Result<BTreeMap<_, _>, SyncConflictError>>()?,
            ),
        })
    }
}

#[derive(Clone, Debug, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictSummary {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub entity_label: String,
    pub field_name: String,
    pub local_value: JsonValue,
    pub remote_value: JsonValue,
    pub created_at_utc_ms: f64,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSyncConflictFieldInput {
    pub conflict_id: String,
    pub choice: SyncConflictChoice,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSyncConflictEntityInput {
    pub entity_type: String,
    pub entity_id: String,
    pub choice: SyncConflictChoice,
}

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
    resolved_content: &Value,
    now_utc_ms: i64,
) -> Result<(), SyncConflictError> {
    let active_fields = conflicts
        .iter()
        .map(|conflict| conflict.field_name)
        .collect::<BTreeSet<_>>();
    let mut stale_statement = tx.prepare(
        "SELECT id, field_name, local_value_json, remote_value_json
         FROM sync_conflicts
         WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3
           AND resolved_at_utc_ms IS NULL
         ORDER BY created_at_utc_ms, id",
    )?;
    let stale = stale_statement
        .query_map(params![account_id, entity_type, entity_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stale_statement);
    for (id, field_name, local_json, remote_json) in stale {
        if active_fields.contains(field_name.as_str()) {
            continue;
        }
        let local = serde_json::from_str::<Value>(&local_json)?;
        let remote = serde_json::from_str::<Value>(&remote_json)?;
        let resolved = if field_name == "__deleted__" {
            Value::Bool(false)
        } else {
            resolved_content.get(&field_name).cloned().unwrap_or(remote)
        };
        let resolution = if resolved == local { "local" } else { "remote" };
        tx.execute(
            "UPDATE sync_conflicts
             SET resolution = ?1, resolved_value_json = ?2, resolved_at_utc_ms = ?3
             WHERE id = ?4 AND account_id = ?5 AND resolved_at_utc_ms IS NULL",
            params![resolution, resolved.to_string(), now_utc_ms, id, account_id],
        )?;
    }
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
         WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
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

pub(crate) fn cleanup_deleted_profile_sync_state(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
    preserve_profile_entity_conflicts: bool,
    now_utc_ms: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "DELETE FROM sync_operations
         WHERE account_id = ?1 AND (
           (entity_type = 'learner_profile' AND entity_id = ?2)
           OR (profile_id = ?2 AND entity_type <> 'asset')
         )",
        params![account_id, profile_id],
    )?;
    tx.execute(
        "UPDATE sync_operations SET profile_id = NULL
         WHERE account_id = ?1 AND profile_id = ?2 AND entity_type = 'asset'",
        params![account_id, profile_id],
    )?;
    if preserve_profile_entity_conflicts {
        tx.execute(
            "UPDATE sync_conflicts
             SET resolution = 'remote', resolved_value_json = 'null',
                 resolved_at_utc_ms = ?3
             WHERE account_id = ?1 AND profile_id = ?2
               AND resolved_at_utc_ms IS NULL
               AND NOT (
                 entity_type = 'learner_profile' AND entity_id = ?2
               )",
            params![account_id, profile_id, now_utc_ms],
        )?;
    } else {
        tx.execute(
            "UPDATE sync_conflicts
             SET resolution = 'remote', resolved_value_json = 'null',
                 resolved_at_utc_ms = ?3
             WHERE account_id = ?1 AND profile_id = ?2
               AND resolved_at_utc_ms IS NULL",
            params![account_id, profile_id, now_utc_ms],
        )?;
    }
    tx.execute(
        "DELETE FROM sync_entity_snapshots
         WHERE account_id = ?1 AND (
           profile_id = ?2
           OR (entity_type = 'learner_profile' AND entity_id = ?2)
         )",
        params![account_id, profile_id],
    )?;
    tx.execute(
        "DELETE FROM tombstones
         WHERE account_id = ?1 AND profile_id = ?2",
        params![account_id, profile_id],
    )?;
    Ok(())
}

pub(crate) fn has_open_conflict(
    tx: &Transaction<'_>,
    account_id: &str,
    entity_type: &str,
    entity_id: &str,
) -> Result<bool, rusqlite::Error> {
    tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sync_conflicts
           WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3
             AND resolved_at_utc_ms IS NULL
         )",
        params![account_id, entity_type, entity_id],
        |row| row.get(0),
    )
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

pub(crate) fn profile_content(value: &WireProfile) -> Value {
    serde_json::json!({ "name": value.name })
}

pub(crate) fn problem_content(value: &WireProblemAggregate) -> Value {
    serde_json::json!({
        "subject": value.subject,
        "tags": value.tags,
        "note": value.note,
        "status": value.status,
        "timeLimitSeconds": value.time_limit_seconds,
        "assets": value.assets,
    })
}

pub(crate) fn export_content(value: &WireExportSnapshot) -> Value {
    serde_json::json!({
        "title": value.title,
        "problemIds": value.problem_ids,
        "configuration": value.configuration,
    })
}

pub fn list_sync_conflicts(
    tx: &Transaction<'_>,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<SyncConflictSummary>, SyncConflictError> {
    let mut statement = tx.prepare(
        "SELECT c.id, c.entity_type, c.entity_id,
                CASE c.entity_type
                  WHEN 'learner_profile' THEN COALESCE(p.name, '已删除学习档案')
                  WHEN 'problem' THEN COALESCE(problem.subject, '已删除错题')
                  WHEN 'export_snapshot' THEN COALESCE(e.title, '已删除导出批次')
                  ELSE '同步内容'
                END,
                c.field_name, c.local_value_json, c.remote_value_json,
                c.created_at_utc_ms
         FROM sync_conflicts c
         LEFT JOIN learner_profiles p
           ON c.entity_type = 'learner_profile' AND p.id = c.entity_id
              AND p.account_id = c.account_id
         LEFT JOIN problems problem
           ON c.entity_type = 'problem' AND problem.id = c.entity_id
              AND problem.account_id = c.account_id
         LEFT JOIN export_snapshots e
           ON c.entity_type = 'export_snapshot' AND e.id = c.entity_id
              AND e.account_id = c.account_id
         WHERE c.account_id = ?1 AND c.profile_id = ?2
           AND c.resolved_at_utc_ms IS NULL
         ORDER BY c.created_at_utc_ms, c.entity_type, c.entity_id, c.field_name, c.id",
    )?;
    let rows = statement
        .query_map(params![account_id, profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                id,
                entity_type,
                entity_id,
                entity_label,
                field_name,
                local_value,
                remote_value,
                created_at_utc_ms,
            )| {
                Ok(SyncConflictSummary {
                    id,
                    entity_type,
                    entity_id,
                    entity_label,
                    field_name,
                    local_value: JsonValue::from_serde(serde_json::from_str(&local_value)?)?,
                    remote_value: JsonValue::from_serde(serde_json::from_str(&remote_value)?)?,
                    created_at_utc_ms: created_at_utc_ms as f64,
                })
            },
        )
        .collect()
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
