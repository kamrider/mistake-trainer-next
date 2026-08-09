use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};

use crate::modules::capture_inbox::{get_capture_batch_detail, remove_encrypted_blob};

use super::{
    CaptureRecognitionError, CaptureRecognitionOperationSummary, CaptureRecognitionRevertReport,
    RevertCaptureRecognition, capture_recognition_operation_ledger::RecognitionOperationLedger,
};

pub fn revert_capture_recognition(
    connection: &mut Connection,
    input: RevertCaptureRecognition,
) -> Result<CaptureRecognitionRevertReport, CaptureRecognitionError> {
    let operation = connection
        .query_row(
            "SELECT o.job_id, o.after_revision, o.created_entity_ids_json,
                    o.reverted_at_utc_ms
             FROM capture_recognition_operations o
             JOIN capture_recognition_jobs j ON j.id = o.job_id
             WHERE o.id = ?1 AND o.batch_id = ?2
               AND j.account_id = ?3 AND j.profile_id = ?4",
            params![
                input.operation_id,
                input.batch_id,
                input.account_id,
                input.profile_id
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(CaptureRecognitionError::RevertConflict)?;
    if operation.3.is_some() {
        return Err(CaptureRecognitionError::RevertConflict);
    }
    let ledger: RecognitionOperationLedger = serde_json::from_str(&operation.2)?;
    let current_revision: i64 = connection
        .query_row(
            "SELECT revision FROM capture_batches
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND state = 'organizing'",
            params![input.batch_id, input.account_id, input.profile_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::RevertConflict)?;
    if u32::try_from(current_revision).unwrap_or(u32::MAX) != input.expected_revision
        || current_revision != operation.1
    {
        return Err(CaptureRecognitionError::RevertConflict);
    }
    validate_recognition_revert_state(connection, &input.batch_id, &ledger)?;

    let transaction = connection.transaction()?;
    validate_recognition_revert_state(&transaction, &input.batch_id, &ledger)?;
    let mut orphan_paths = Vec::new();
    for item in &ledger.created_items {
        transaction.execute(
            "DELETE FROM asset_derivations
             WHERE id = ?1 AND operation_id = ?2 AND derived_capture_item_id = ?3",
            params![item.derivation_id, input.operation_id, item.item_id],
        )?;
    }
    for item in &ledger.created_items {
        transaction.execute(
            "DELETE FROM capture_items WHERE id = ?1 AND batch_id = ?2",
            params![item.item_id, input.batch_id],
        )?;
    }
    for draft in &ledger.created_drafts {
        transaction.execute(
            "DELETE FROM capture_drafts WHERE id = ?1 AND batch_id = ?2",
            params![draft.draft_id, input.batch_id],
        )?;
    }
    for source in &ledger.source_items {
        let restored = transaction.execute(
            "UPDATE capture_items SET superseded_by_derivation_id = NULL
             WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id = ?3",
            params![
                source.item_id,
                input.batch_id,
                source.superseded_by_derivation_id
            ],
        )?;
        if restored != 1 {
            return Err(CaptureRecognitionError::RevertConflict);
        }
        transaction.execute(
            "DELETE FROM capture_source_retention
             WHERE batch_id = ?1 AND source_asset_id = ?2
               AND NOT EXISTS(
                 SELECT 1 FROM capture_items
                  WHERE batch_id = ?1 AND asset_id = ?2
                    AND superseded_by_derivation_id IS NOT NULL
               )",
            params![input.batch_id, source.asset_id],
        )?;
    }
    let mut seen_assets = BTreeSet::new();
    for item in &ledger.created_items {
        if !seen_assets.insert(item.asset_id.clone()) {
            continue;
        }
        let encrypted_path = transaction
            .query_row(
                "SELECT encrypted_path FROM assets WHERE id = ?1 AND account_id = ?2",
                params![item.asset_id, input.account_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(encrypted_path) = encrypted_path else {
            continue;
        };
        let referenced: bool = transaction.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM capture_items WHERE asset_id = ?1
               UNION ALL SELECT 1 FROM problem_assets WHERE asset_id = ?1
             )",
            [item.asset_id.as_str()],
            |row| row.get(0),
        )?;
        if !referenced
            && transaction.execute("DELETE FROM assets WHERE id = ?1", [item.asset_id.as_str()])?
                == 1
        {
            orphan_paths.push(encrypted_path);
        }
    }
    transaction.execute(
        "UPDATE capture_recognition_pairs
         SET state = 'invalidated', resolved_at_utc_ms = ?1
         WHERE operation_id = ?2 AND state = 'active'",
        params![input.now_utc_ms, input.operation_id],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_operations SET reverted_at_utc_ms = ?1
         WHERE id = ?2 AND reverted_at_utc_ms IS NULL",
        params![input.now_utc_ms, input.operation_id],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'cancelled', updated_at_utc_ms = ?1 WHERE id = ?2 AND state = 'applied'",
        params![input.now_utc_ms, operation.0],
    )?;
    transaction.execute(
        "UPDATE capture_batches SET revision = revision + 1, updated_at_utc_ms = ?1
         WHERE id = ?2",
        params![input.now_utc_ms, input.batch_id],
    )?;
    transaction.commit()?;
    for path in orphan_paths {
        let _ = remove_encrypted_blob(&input.blob_root, &path);
    }
    let detail = get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    Ok(CaptureRecognitionRevertReport {
        operation_id: input.operation_id,
        reverted_item_count: u32::try_from(ledger.created_items.len()).unwrap_or(u32::MAX),
        detail,
    })
}

pub fn latest_capture_recognition_operation(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<Option<CaptureRecognitionOperationSummary>, CaptureRecognitionError> {
    let row = connection
        .query_row(
            "SELECT o.id, o.batch_id, o.after_revision, o.created_entity_ids_json,
                    o.reverted_at_utc_ms
             FROM capture_recognition_operations o
             JOIN capture_recognition_jobs j ON j.id = o.job_id
             WHERE o.batch_id = ?1 AND j.account_id = ?2 AND j.profile_id = ?3
             ORDER BY o.created_at_utc_ms DESC, o.id DESC LIMIT 1",
            params![batch_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((operation_id, batch_id, after_revision, ledger_json, reverted_at)) = row else {
        return Ok(None);
    };
    let ledger: RecognitionOperationLedger = serde_json::from_str(&ledger_json)?;
    Ok(Some(CaptureRecognitionOperationSummary {
        operation_id,
        batch_id,
        after_revision: u32::try_from(after_revision).unwrap_or(u32::MAX),
        created_item_count: u32::try_from(ledger.created_items.len()).unwrap_or(u32::MAX),
        reverted: reverted_at.is_some(),
    }))
}

fn validate_recognition_revert_state(
    connection: &Connection,
    batch_id: &str,
    ledger: &RecognitionOperationLedger,
) -> Result<(), CaptureRecognitionError> {
    for source in &ledger.source_items {
        let current = connection
            .query_row(
                "SELECT asset_id, superseded_by_derivation_id FROM capture_items
                 WHERE id = ?1 AND batch_id = ?2",
                params![source.item_id, batch_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        if current
            != Some((
                source.asset_id.clone(),
                Some(source.superseded_by_derivation_id.clone()),
            ))
        {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    for item in &ledger.created_items {
        let current = connection
            .query_row(
                "SELECT i.asset_id, i.source_sequence, i.staged_role,
                        i.superseded_by_derivation_id, di.draft_id, di.role, di.position
                 FROM capture_items i
                 LEFT JOIN capture_draft_items di ON di.item_id = i.id
                 WHERE i.id = ?1 AND i.batch_id = ?2",
                params![item.item_id, batch_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()?;
        if current
            != Some((
                item.asset_id.clone(),
                item.source_sequence,
                item.staged_role.clone(),
                None,
                item.draft_id.clone(),
                item.role.clone(),
                item.position,
            ))
        {
            return Err(CaptureRecognitionError::RevertConflict);
        }
        let referenced_or_derived: bool = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM problem_assets WHERE asset_id = ?1
               UNION ALL
               SELECT 1 FROM asset_derivations
                WHERE source_capture_item_id = ?2 AND id <> ?3
             )",
            params![item.asset_id, item.item_id, item.derivation_id],
            |row| row.get(0),
        )?;
        if referenced_or_derived {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    for draft in &ledger.created_drafts {
        let current = connection
            .query_row(
                "SELECT position, subject_override, tags_json, note FROM capture_drafts
                 WHERE id = ?1 AND batch_id = ?2",
                params![draft.draft_id, batch_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        if current != Some((draft.position, None, "[]".to_owned(), String::new())) {
            return Err(CaptureRecognitionError::RevertConflict);
        }
    }
    Ok(())
}
