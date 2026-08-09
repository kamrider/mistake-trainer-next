use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::modules::{
    sync_conflict_merge::{
        FieldConflict, MergeAction, export_content, merge_export_versions, merge_problem_versions,
        merge_profile_versions, problem_content,
    },
    sync_store::{WireExportSnapshot, WireProblemAggregate, WireProblemAsset, WireProfile},
};

#[path = "sync_conflict_resolution.rs"]
mod sync_conflict_resolution;

pub use sync_conflict_resolution::{resolve_sync_conflict_entity, resolve_sync_conflict_field};

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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeletionConflict {
    pub profile_id: Option<String>,
    pub local_value: Value,
    pub base_revision: i64,
}

pub(crate) fn merge_remote_problem(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProblemAggregate,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProblemAggregate>, SyncConflictError> {
    let Some(local) = load_local_problem(tx, account_id, &remote.id)? else {
        return Ok(merge_problem_versions(None, None, remote, now_utc_ms)?);
    };
    let base = load_snapshot::<WireProblemAggregate>(tx, account_id, "problem", &remote.id)?;
    Ok(merge_problem_versions(
        Some(&local),
        base.as_ref(),
        remote,
        now_utc_ms,
    )?)
}

pub(crate) fn merge_remote_profile(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireProfile,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProfile>, SyncConflictError> {
    let Some(local) = load_local_profile(tx, account_id, &remote.id)? else {
        return Ok(merge_profile_versions(None, None, remote, now_utc_ms)?);
    };
    let base = load_snapshot::<WireProfile>(tx, account_id, "learner_profile", &remote.id)?;
    Ok(merge_profile_versions(
        Some(&local),
        base.as_ref(),
        remote,
        now_utc_ms,
    )?)
}

pub(crate) fn merge_remote_export(
    tx: &Transaction<'_>,
    account_id: &str,
    remote: &WireExportSnapshot,
) -> Result<MergeAction<WireExportSnapshot>, SyncConflictError> {
    let Some(local) = load_local_export(tx, account_id, &remote.id)? else {
        return Ok(merge_export_versions(None, None, remote)?);
    };
    let base = load_snapshot::<WireExportSnapshot>(tx, account_id, "export_snapshot", &remote.id)?;
    Ok(merge_export_versions(Some(&local), base.as_ref(), remote)?)
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
