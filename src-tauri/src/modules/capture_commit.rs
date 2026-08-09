use std::collections::BTreeSet;

use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

use super::{CaptureBatchState, CaptureInboxError, ensure_organizing_revision, query_batch};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommitReport {
    pub committed_problem_ids: Vec<String>,
    pub committed_count: u32,
    pub remaining_draft_count: u32,
}

pub fn commit_ready_capture_drafts(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    now_utc_ms: i64,
) -> Result<CaptureCommitReport, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    ensure_organizing_revision(&batch, expected_revision)?;
    let transaction = connection.transaction()?;
    let ready_drafts = query_ready_drafts(&transaction, batch_id, &batch.subject)?;
    let mut committed_problem_ids = Vec::with_capacity(ready_drafts.len());
    for draft in ready_drafts {
        let links = query_draft_asset_links(&transaction, &draft.id)?;
        let problem_id = Uuid::now_v7().to_string();
        transaction.execute(
            "INSERT INTO problems(id, account_id, profile_id, subject, tags_json, note, status,
                                  created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?7, 1)",
            params![
                problem_id,
                account_id,
                profile_id,
                draft.subject,
                draft.tags_json,
                draft.note,
                now_utc_ms
            ],
        )?;
        let mut seen_links = BTreeSet::new();
        let mut asset_ids = Vec::new();
        for link in &links {
            if seen_links.insert((link.role.clone(), link.asset_id.clone())) {
                let position = asset_ids_for_role(&links, &link.role, &link.asset_id);
                transaction.execute(
                    "INSERT INTO problem_assets(problem_id, asset_id, role, position)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![problem_id, link.asset_id, link.role, position],
                )?;
                asset_ids.push(link.asset_id.clone());
            }
        }
        for asset_id in asset_ids.iter().collect::<BTreeSet<_>>() {
            let metadata = query_asset_sync_payload(&transaction, account_id, asset_id)?;
            transaction.execute(
                "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id,
                                             operation, payload_json, status, attempt_count,
                                             created_at_utc_ms, next_attempt_at_utc_ms)
                 VALUES(?1, ?2, ?3, 'asset', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
                params![
                    Uuid::now_v7().to_string(),
                    account_id,
                    profile_id,
                    asset_id,
                    metadata,
                    now_utc_ms
                ],
            )?;
        }
        let tags: Vec<String> = serde_json::from_str(&draft.tags_json)?;
        let problem_payload = serde_json::to_string(&serde_json::json!({
            "id": problem_id,
            "accountId": account_id,
            "profileId": profile_id,
            "subject": draft.subject,
            "tags": tags,
            "note": draft.note,
            "assetIds": asset_ids,
            "createdAtUtcMs": now_utc_ms,
            "updatedAtUtcMs": now_utc_ms,
            "revision": 1
        }))?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id,
                                         operation, payload_json, status, attempt_count,
                                         created_at_utc_ms, next_attempt_at_utc_ms)
             VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
            params![
                Uuid::now_v7().to_string(),
                account_id,
                profile_id,
                problem_id,
                problem_payload,
                now_utc_ms
            ],
        )?;
        transaction.execute(
            "DELETE FROM capture_items WHERE id IN
             (SELECT item_id FROM capture_draft_items WHERE draft_id = ?1)",
            [draft.id.as_str()],
        )?;
        transaction.execute(
            "DELETE FROM capture_drafts WHERE id = ?1",
            [draft.id.as_str()],
        )?;
        committed_problem_ids.push(problem_id);
    }
    let remaining_draft_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_drafts WHERE batch_id = ?1",
        [batch_id],
        |row| row.get(0),
    )?;
    let remaining_item_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM capture_items
         WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL",
        [batch_id],
        |row| row.get(0),
    )?;
    let next_state = if remaining_draft_count == 0 && remaining_item_count == 0 {
        CaptureBatchState::Completed
    } else {
        CaptureBatchState::Organizing
    };
    transaction.execute(
        "UPDATE capture_batches SET state = ?2, updated_at_utc_ms = ?3, revision = revision + 1
         WHERE id = ?1",
        params![batch_id, next_state.as_str(), now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(CaptureCommitReport {
        committed_count: u32::try_from(committed_problem_ids.len()).unwrap_or(u32::MAX),
        committed_problem_ids,
        remaining_draft_count: u32::try_from(remaining_draft_count).unwrap_or(u32::MAX),
    })
}

struct ReadyDraft {
    id: String,
    subject: String,
    tags_json: String,
    note: String,
}

struct DraftAssetLink {
    asset_id: String,
    role: String,
}

fn query_ready_drafts(
    transaction: &Transaction<'_>,
    batch_id: &str,
    batch_subject: &str,
) -> Result<Vec<ReadyDraft>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT d.id, COALESCE(NULLIF(trim(d.subject_override), ''), ?2), d.tags_json, d.note
         FROM capture_drafts d
         WHERE d.batch_id = ?1
           AND trim(COALESCE(NULLIF(d.subject_override, ''), ?2)) <> ''
           AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
           AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')
         ORDER BY d.position, d.id",
    )?;
    statement
        .query_map(params![batch_id, batch_subject], |row| {
            Ok(ReadyDraft {
                id: row.get(0)?,
                subject: row.get(1)?,
                tags_json: row.get(2)?,
                note: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn query_draft_asset_links(
    transaction: &Transaction<'_>,
    draft_id: &str,
) -> Result<Vec<DraftAssetLink>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT i.asset_id, di.role FROM capture_draft_items di
         JOIN capture_items i ON i.id = di.item_id
         WHERE di.draft_id = ?1
         ORDER BY CASE di.role WHEN 'question' THEN 0 ELSE 1 END, di.position, di.item_id",
    )?;
    statement
        .query_map([draft_id], |row| {
            Ok(DraftAssetLink {
                asset_id: row.get(0)?,
                role: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn asset_ids_for_role(links: &[DraftAssetLink], role: &str, target_asset_id: &str) -> i64 {
    let mut seen = BTreeSet::new();
    for link in links.iter().filter(|link| link.role == role) {
        if !seen.insert(link.asset_id.as_str()) {
            continue;
        }
        if link.asset_id == target_asset_id {
            return i64::try_from(seen.len() - 1).unwrap_or(i64::MAX);
        }
    }
    i64::try_from(seen.len()).unwrap_or(i64::MAX)
}

fn query_asset_sync_payload(
    transaction: &Transaction<'_>,
    account_id: &str,
    asset_id: &str,
) -> Result<String, CaptureInboxError> {
    let value = transaction.query_row(
        "SELECT id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
                created_at_utc_ms FROM assets WHERE id = ?1 AND account_id = ?2",
        params![asset_id, account_id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "accountId": row.get::<_, String>(1)?,
                "plaintextSha256": row.get::<_, String>(2)?,
                "encryptedPath": row.get::<_, String>(3)?,
                "byteLength": row.get::<_, i64>(4)?,
                "mediaType": row.get::<_, String>(5)?,
                "createdAtUtcMs": row.get::<_, i64>(6)?,
            }))
        },
    )?;
    Ok(serde_json::to_string(&value)?)
}
