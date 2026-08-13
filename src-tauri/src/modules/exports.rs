use std::{collections::HashSet, path::Path};

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::application::ports::assets::AssetDecryptor;

#[path = "exports_generation.rs"]
mod generation;

pub use generation::generate_export;

pub(crate) struct PreparedExport(generation::PreparedExport);

pub(crate) fn prepare_export(
    connection: &Connection,
    blob_root: &Path,
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
) -> Result<PreparedExport, ExportError> {
    generation::prepare_export(connection, blob_root, account_id, profile_id, snapshot_id)
        .map(PreparedExport)
}

pub(crate) fn write_prepared_export(
    prepared: PreparedExport,
    destination: &Path,
    asset_decryptor: &dyn AssetDecryptor,
) -> Result<GeneratedExportSummary, ExportError> {
    generation::write_prepared_export(prepared.0, destination, asset_decryptor)
}

const TRASH_RETENTION_MS: i64 = 30 * 86_400_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportLayout {
    QuestionAnswerAlternating,
    QuestionsThenAnswers,
    OriginalImageFolder,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportCandidateSource {
    Due,
    LatestReviewSession,
    AllActive,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportCandidate {
    pub id: String,
    pub subject: String,
    pub note: String,
    pub question_asset_count: i32,
    pub answer_asset_count: i32,
    pub due_at_utc_ms: Option<f64>,
    pub review_count: i32,
}

#[derive(Clone, Debug)]
pub struct CreateExportSnapshot {
    pub account_id: String,
    pub profile_id: String,
    pub title: String,
    pub problem_ids: Vec<String>,
    pub layout: ExportLayout,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportSnapshotSummary {
    pub id: String,
    pub title: String,
    pub problem_count: i32,
    pub layout: ExportLayout,
    pub created_at_utc_ms: f64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeletedExportSnapshotSummary {
    pub snapshot: ExportSnapshotSummary,
    pub deleted_at_utc_ms: f64,
    pub purge_after_utc_ms: f64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedExportSummary {
    pub snapshot_id: String,
    pub output_name: String,
    pub problem_count: i32,
    pub layout: ExportLayout,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("export title must contain between 1 and 80 characters")]
    InvalidTitle,
    #[error("an export requires between 1 and 500 unique problems")]
    InvalidSelection,
    #[error("one or more selected problems are unavailable")]
    ProblemNotFound,
    #[error("export snapshot was not found")]
    SnapshotNotFound,
    #[error("the selected export destination is invalid")]
    InvalidDestination,
    #[error("an encrypted asset path is invalid")]
    InvalidAssetPath,
    #[error("an encrypted asset exceeds the safety limit")]
    AssetTooLarge,
    #[error("the selected export exceeds the process memory safety budget")]
    ExportTooLarge,
    #[error("an export image is invalid or unsupported")]
    InvalidImage,
    #[error("an export file operation failed")]
    File(#[from] std::io::Error),
    #[error("a DOCX document could not be generated")]
    Docx(String),
    #[error("export database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("export snapshot serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub fn create_export_snapshot(
    connection: &mut Connection,
    input: CreateExportSnapshot,
) -> Result<ExportSnapshotSummary, ExportError> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 80 {
        return Err(ExportError::InvalidTitle);
    }
    let mut observed = HashSet::new();
    let problem_ids = input
        .problem_ids
        .into_iter()
        .filter(|problem_id| observed.insert(problem_id.clone()))
        .collect::<Vec<_>>();
    if problem_ids.is_empty() || problem_ids.len() > 500 {
        return Err(ExportError::InvalidSelection);
    }
    let transaction = connection.transaction()?;
    for problem_id in &problem_ids {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM problems WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND status != 'trashed')",
            params![problem_id, input.account_id, input.profile_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(ExportError::ProblemNotFound);
        }
    }
    let id = Uuid::now_v7().to_string();
    let ids = problem_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let problem_ids_json = serde_json::to_string(&ids)?;
    let configuration_json = serde_json::to_string(&serde_json::json!({ "layout": input.layout }))?;
    transaction.execute(
        "INSERT INTO export_snapshots(id, account_id, profile_id, title, problem_ids_json, configuration_json, created_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params![id, input.account_id, input.profile_id, title, problem_ids_json, configuration_json, input.now_utc_ms],
    )?;
    let payload = serde_json::to_string(&serde_json::json!({
        "id": id,
        "accountId": input.account_id,
        "profileId": input.profile_id,
        "title": title,
        "problemIds": ids,
        "configuration": { "layout": input.layout },
        "createdAtUtcMs": input.now_utc_ms,
        "revision": 1,
    }))?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, ?3, 'export_snapshot', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), input.account_id, input.profile_id, id, payload, input.now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(ExportSnapshotSummary {
        id,
        title: title.to_owned(),
        problem_count: i32::try_from(problem_ids.len()).unwrap_or(i32::MAX),
        layout: input.layout,
        created_at_utc_ms: input.now_utc_ms as f64,
    })
}

pub fn list_export_snapshots(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<ExportSnapshotSummary>, ExportError> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.title, json_array_length(s.problem_ids_json), s.configuration_json, s.created_at_utc_ms
         FROM export_snapshots s
         WHERE s.account_id = ?1 AND s.profile_id = ?2
           AND NOT EXISTS(SELECT 1 FROM tombstones t WHERE t.entity_type = 'export_snapshot' AND t.entity_id = s.id)
         ORDER BY created_at_utc_ms DESC, id DESC LIMIT 100",
    )?;
    let rows = statement.query_map(params![account_id, profile_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, title, problem_count, configuration, created_at_utc_ms) = row?;
        #[derive(Deserialize)]
        struct Configuration {
            layout: ExportLayout,
        }
        let configuration: Configuration = serde_json::from_str(&configuration)?;
        Ok(ExportSnapshotSummary {
            id,
            title,
            problem_count: i32::try_from(problem_count).unwrap_or(i32::MAX),
            layout: configuration.layout,
            created_at_utc_ms: created_at_utc_ms as f64,
        })
    })
    .collect()
}

pub fn list_export_candidates(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    source: ExportCandidateSource,
    now_utc_ms: i64,
) -> Result<Vec<ExportCandidate>, ExportError> {
    let select = "SELECT p.id, p.subject, p.note,
                         (SELECT COUNT(*) FROM problem_assets pa WHERE pa.problem_id = p.id AND pa.role = 'question'),
                         (SELECT COUNT(*) FROM problem_assets pa WHERE pa.problem_id = p.id AND pa.role = 'answer'),
                         s.due_at_utc_ms,
                         (SELECT COUNT(*) FROM review_events e WHERE e.problem_id = p.id)
                  FROM problems p
                  LEFT JOIN schedule_states s ON s.problem_id = p.id";
    match source {
        ExportCandidateSource::Due => {
            let sql = format!(
                "{select}
                 WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
                   AND (s.due_at_utc_ms IS NULL OR s.due_at_utc_ms <= ?3)
                 ORDER BY COALESCE(s.due_at_utc_ms, 0), p.updated_at_utc_ms, p.id
                 LIMIT 500"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params![account_id, profile_id, now_utc_ms],
                candidate_from_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(ExportError::Database)
        }
        ExportCandidateSource::AllActive => {
            let sql = format!(
                "{select}
                 WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
                 ORDER BY p.updated_at_utc_ms DESC, p.id DESC
                 LIMIT 500"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params![account_id, profile_id], candidate_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(ExportError::Database)
        }
        ExportCandidateSource::LatestReviewSession => {
            let mut statement = connection.prepare(
                "WITH latest AS (
                     SELECT problem_ids_json
                     FROM review_sessions
                     WHERE account_id = ?1 AND profile_id = ?2
                     ORDER BY updated_at_utc_ms DESC, id DESC
                     LIMIT 1
                 )
                 SELECT p.id, p.subject, p.note,
                        (SELECT COUNT(*) FROM problem_assets pa WHERE pa.problem_id = p.id AND pa.role = 'question'),
                        (SELECT COUNT(*) FROM problem_assets pa WHERE pa.problem_id = p.id AND pa.role = 'answer'),
                        s.due_at_utc_ms,
                        (SELECT COUNT(*) FROM review_events e WHERE e.problem_id = p.id)
                 FROM latest
                 JOIN json_each(latest.problem_ids_json) item
                 JOIN problems p ON p.id = item.value
                 LEFT JOIN schedule_states s ON s.problem_id = p.id
                 WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status != 'trashed'
                 ORDER BY CAST(item.key AS INTEGER)
                 LIMIT 500",
            )?;
            let rows = statement.query_map(params![account_id, profile_id], candidate_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(ExportError::Database)
        }
    }
}

fn candidate_from_row(row: &Row<'_>) -> rusqlite::Result<ExportCandidate> {
    let question_asset_count = row.get::<_, i64>(3)?;
    let answer_asset_count = row.get::<_, i64>(4)?;
    let due_at_utc_ms = row.get::<_, Option<i64>>(5)?;
    let review_count = row.get::<_, i64>(6)?;
    Ok(ExportCandidate {
        id: row.get(0)?,
        subject: row.get(1)?,
        note: row.get(2)?,
        question_asset_count: i32::try_from(question_asset_count).unwrap_or(i32::MAX),
        answer_asset_count: i32::try_from(answer_asset_count).unwrap_or(i32::MAX),
        due_at_utc_ms: due_at_utc_ms.map(|value| value as f64),
        review_count: i32::try_from(review_count).unwrap_or(i32::MAX),
    })
}

pub fn list_deleted_export_snapshots(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<DeletedExportSnapshotSummary>, ExportError> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.title, json_array_length(s.problem_ids_json), s.configuration_json,
                s.created_at_utc_ms, t.deleted_at_utc_ms, t.purge_after_utc_ms
         FROM export_snapshots s
         JOIN tombstones t ON t.entity_type = 'export_snapshot' AND t.entity_id = s.id
         WHERE s.account_id = ?1 AND s.profile_id = ?2
           AND t.account_id = ?1 AND t.profile_id = ?2
         ORDER BY t.deleted_at_utc_ms DESC, s.id DESC LIMIT 100",
    )?;
    let rows = statement.query_map(params![account_id, profile_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (
            id,
            title,
            problem_count,
            configuration,
            created_at_utc_ms,
            deleted_at_utc_ms,
            purge_after_utc_ms,
        ) = row?;
        #[derive(Deserialize)]
        struct Configuration {
            layout: ExportLayout,
        }
        let configuration: Configuration = serde_json::from_str(&configuration)?;
        Ok(DeletedExportSnapshotSummary {
            snapshot: ExportSnapshotSummary {
                id,
                title,
                problem_count: i32::try_from(problem_count).unwrap_or(i32::MAX),
                layout: configuration.layout,
                created_at_utc_ms: created_at_utc_ms as f64,
            },
            deleted_at_utc_ms: deleted_at_utc_ms as f64,
            purge_after_utc_ms: purge_after_utc_ms as f64,
        })
    })
    .collect()
}

pub fn delete_export_snapshot(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
    now_utc_ms: i64,
) -> Result<(), ExportError> {
    let transaction = connection.transaction()?;
    let revision = transaction.query_row(
        "SELECT revision FROM export_snapshots WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
        params![snapshot_id, account_id, profile_id],
        |row| row.get::<_, i64>(0),
    ).optional()?.ok_or(ExportError::SnapshotNotFound)?;
    let purge_after_utc_ms = now_utc_ms + TRASH_RETENTION_MS;
    let deleted_revision = revision + 1;
    transaction.execute(
        "UPDATE export_snapshots SET revision = ?1 WHERE id = ?2 AND account_id = ?3 AND profile_id = ?4",
        params![deleted_revision, snapshot_id, account_id, profile_id],
    )?;
    transaction.execute(
        "INSERT INTO tombstones(id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms, purge_after_utc_ms, revision)
         VALUES(?1, ?2, ?3, 'export_snapshot', ?4, ?5, ?6, ?7)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET deleted_at_utc_ms = excluded.deleted_at_utc_ms, purge_after_utc_ms = excluded.purge_after_utc_ms, revision = excluded.revision",
        params![Uuid::now_v7().to_string(), account_id, profile_id, snapshot_id, now_utc_ms, purge_after_utc_ms, deleted_revision],
    )?;
    let payload = serde_json::to_string(&serde_json::json!({
        "id": snapshot_id, "baseRevision": revision, "revision": deleted_revision,
        "deletedAtUtcMs": now_utc_ms, "purgeAfterUtcMs": purge_after_utc_ms,
    }))?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, ?3, 'export_snapshot', ?4, 'delete', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), account_id, profile_id, snapshot_id, payload, now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn restore_export_snapshot(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    snapshot_id: &str,
    now_utc_ms: i64,
) -> Result<(), ExportError> {
    let transaction = connection.transaction()?;
    let deleted_revision = transaction
        .query_row(
            "SELECT t.revision FROM export_snapshots s
         JOIN tombstones t ON t.entity_type = 'export_snapshot' AND t.entity_id = s.id
         WHERE s.id = ?1 AND s.account_id = ?2 AND s.profile_id = ?3
           AND t.account_id = ?2 AND t.profile_id = ?3",
            params![snapshot_id, account_id, profile_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(ExportError::SnapshotNotFound)?;
    let restored_revision = deleted_revision + 1;
    transaction.execute(
        "UPDATE export_snapshots SET revision = ?1 WHERE id = ?2 AND account_id = ?3 AND profile_id = ?4",
        params![restored_revision, snapshot_id, account_id, profile_id],
    )?;
    transaction.execute(
        "DELETE FROM tombstones WHERE entity_type = 'export_snapshot' AND entity_id = ?1",
        [snapshot_id],
    )?;
    let payload = serde_json::to_string(&serde_json::json!({
        "id": snapshot_id, "baseRevision": deleted_revision, "revision": restored_revision,
        "restoredAtUtcMs": now_utc_ms,
    }))?;
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES(?1, ?2, ?3, 'export_snapshot', ?4, 'restore', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), account_id, profile_id, snapshot_id, payload, now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(())
}
