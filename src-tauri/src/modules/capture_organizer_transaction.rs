use std::{collections::BTreeSet, path::Path};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use uuid::Uuid;

use super::capture_inbox_repository::query_batch;
use super::capture_inbox_transaction_support::{
    delete_asset_row_if_orphan, invalidate_active_pairs_for_item, repack_link_positions,
    touch_batch,
};
use super::{
    ApplyCaptureLayout, ApplyCapturePairSuggestions, CaptureBatchDetail, CaptureBatchState,
    CaptureInboxError, CaptureLayoutMode, MAX_CAPTURE_BATCH_ITEMS, MergeCaptureCard,
    MoveCaptureItem, StageCaptureItemRole, UpdateCaptureDraft, ensure_organizing_revision,
    get_capture_batch_detail, normalize_subject, remove_encrypted_blob,
    validate_relative_asset_path,
};

pub fn apply_capture_layout(
    connection: &mut Connection,
    input: ApplyCaptureLayout,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    if batch.state != CaptureBatchState::Organizing {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != input.expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    let question_count = usize::try_from(input.question_images_per_draft).unwrap_or(0);
    let answer_count = usize::try_from(input.answer_images_per_draft).unwrap_or(0);
    if matches!(input.mode, CaptureLayoutMode::Alternating)
        && (question_count == 0 || answer_count == 0 || question_count > 10 || answer_count > 10)
    {
        return Err(CaptureInboxError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    let item_ids = query_batch_item_ids(&transaction, &input.batch_id)?;
    invalidate_active_pairs_for_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.execute(
        "DELETE FROM capture_drafts WHERE batch_id = ?1",
        [input.batch_id.as_str()],
    )?;
    match input.mode {
        CaptureLayoutMode::Manual => {}
        CaptureLayoutMode::QuestionsOnly => {
            for (position, item_id) in item_ids.iter().enumerate() {
                let draft_id =
                    insert_draft(&transaction, &input.batch_id, position, input.now_utc_ms)?;
                insert_link(&transaction, &draft_id, item_id, "question", 0)?;
            }
        }
        CaptureLayoutMode::Alternating => {
            let group_size = question_count + answer_count;
            for (draft_position, group) in item_ids.chunks(group_size).enumerate() {
                let draft_id = insert_draft(
                    &transaction,
                    &input.batch_id,
                    draft_position,
                    input.now_utc_ms,
                )?;
                for (position, item_id) in group.iter().take(question_count).enumerate() {
                    insert_link(&transaction, &draft_id, item_id, "question", position)?;
                }
                for (position, item_id) in group.iter().skip(question_count).enumerate() {
                    insert_link(&transaction, &draft_id, item_id, "answer", position)?;
                }
            }
        }
        CaptureLayoutMode::Split => {
            let default_split = item_ids.len().div_ceil(2);
            let split = input
                .split_index
                .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
                .unwrap_or(default_split)
                .min(item_ids.len());
            let (questions, answers) = item_ids.split_at(split);
            for (position, question_id) in questions.iter().enumerate() {
                let draft_id =
                    insert_draft(&transaction, &input.batch_id, position, input.now_utc_ms)?;
                insert_link(&transaction, &draft_id, question_id, "question", 0)?;
                if let Some(answer_id) = answers.get(position) {
                    insert_link(&transaction, &draft_id, answer_id, "answer", 0)?;
                }
            }
        }
    }
    transaction.execute(
        "UPDATE capture_batches SET updated_at_utc_ms = ?2, revision = revision + 1 WHERE id = ?1",
        params![input.batch_id, input.now_utc_ms],
    )?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

fn query_batch_item_ids(
    transaction: &Transaction<'_>,
    batch_id: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = transaction.prepare(
        "SELECT id FROM capture_items
         WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL
         ORDER BY source_sequence, id",
    )?;
    statement
        .query_map([batch_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
}

fn insert_draft(
    transaction: &Transaction<'_>,
    batch_id: &str,
    position: usize,
    now_utc_ms: i64,
) -> Result<String, rusqlite::Error> {
    insert_draft_with_subject(transaction, batch_id, position, None, now_utc_ms)
}

fn insert_draft_with_subject(
    transaction: &Transaction<'_>,
    batch_id: &str,
    position: usize,
    subject_override: Option<&str>,
    now_utc_ms: i64,
) -> Result<String, rusqlite::Error> {
    let id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO capture_drafts(
             id, batch_id, position, subject_override, created_at_utc_ms, updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            id,
            batch_id,
            i64::try_from(position).unwrap_or(i64::MAX),
            subject_override,
            now_utc_ms,
        ],
    )?;
    Ok(id)
}

fn repack_draft_positions(
    transaction: &Transaction<'_>,
    batch_id: &str,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_drafts SET position = position + 1000 WHERE batch_id = ?1",
        [batch_id],
    )?;
    let draft_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM capture_drafts WHERE batch_id = ?1 ORDER BY position, id")?;
        statement
            .query_map([batch_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (position, draft_id) in draft_ids.iter().enumerate() {
        transaction.execute(
            "UPDATE capture_drafts SET position = ?1 WHERE id = ?2 AND batch_id = ?3",
            params![
                i64::try_from(position).unwrap_or(i64::MAX),
                draft_id,
                batch_id,
            ],
        )?;
    }
    Ok(())
}

fn insert_link(
    transaction: &Transaction<'_>,
    draft_id: &str,
    item_id: &str,
    role: &str,
    position: usize,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "INSERT INTO capture_draft_items(draft_id, item_id, role, position) VALUES(?1, ?2, ?3, ?4)",
        params![
            draft_id,
            item_id,
            role,
            i64::try_from(position).unwrap_or(i64::MAX)
        ],
    )?;
    transaction.execute(
        "UPDATE capture_items SET staged_role = ?2 WHERE id = ?1",
        params![item_id, role],
    )?;
    Ok(())
}

fn invalidate_active_pairs_for_batch(
    transaction: &Transaction<'_>,
    batch_id: &str,
    now_utc_ms: i64,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_recognition_pairs
         SET state = 'invalidated', resolved_at_utc_ms = ?2
         WHERE state = 'active'
           AND EXISTS(
             SELECT 1
             FROM capture_recognition_operations operation
             WHERE operation.id = capture_recognition_pairs.operation_id
               AND operation.batch_id = ?1
           )",
        params![batch_id, now_utc_ms],
    )?;
    Ok(())
}

pub fn move_capture_item(
    connection: &mut Connection,
    input: MoveCaptureItem,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let target_role = match (&input.target_draft_id, input.target_role.as_deref()) {
        (None, None) => None,
        (Some(_), Some("question")) => Some("question"),
        (Some(_), Some("answer")) => Some("answer"),
        _ => return Err(CaptureInboxError::InvalidInput),
    };
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let transaction = connection.transaction()?;
    let item_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM capture_items
         WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id IS NULL)",
        params![input.item_id, input.batch_id],
        |row| row.get(0),
    )?;
    if !item_exists {
        return Err(CaptureInboxError::ItemNotFound);
    }
    invalidate_active_pairs_for_item(&transaction, &input.item_id, input.now_utc_ms)?;
    let source = transaction
        .query_row(
            "SELECT draft_id, role FROM capture_draft_items WHERE item_id = ?1",
            [input.item_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    transaction.execute(
        "DELETE FROM capture_draft_items WHERE item_id = ?1",
        [input.item_id.as_str()],
    )?;
    if let Some((source_draft_id, source_role)) = source {
        repack_link_positions(&transaction, &source_draft_id, &source_role)?;
    }
    if let (Some(target_draft_id), Some(role)) = (&input.target_draft_id, target_role) {
        let target_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM capture_drafts WHERE id = ?1 AND batch_id = ?2)",
            params![target_draft_id, input.batch_id],
            |row| row.get(0),
        )?;
        if !target_exists {
            return Err(CaptureInboxError::DraftNotFound);
        }
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM capture_draft_items WHERE draft_id = ?1 AND role = ?2",
            params![target_draft_id, role],
            |row| row.get(0),
        )?;
        let position = i64::from(input.target_position).min(count);
        transaction.execute(
            "UPDATE capture_draft_items SET position = position + 1
             WHERE draft_id = ?1 AND role = ?2 AND position >= ?3",
            params![target_draft_id, role, position],
        )?;
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![target_draft_id, input.item_id, role, position],
        )?;
        transaction.execute(
            "UPDATE capture_items SET staged_role = ?2 WHERE id = ?1",
            params![input.item_id, role],
        )?;
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn stage_capture_item_role(
    connection: &mut Connection,
    input: StageCaptureItemRole,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    if input.staged_role != "question" && input.staged_role != "answer" {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let transaction = connection.transaction()?;
    invalidate_active_pairs_for_item(&transaction, &input.item_id, input.now_utc_ms)?;
    let changed = transaction.execute(
        "UPDATE capture_items SET staged_role = ?1
         WHERE id = ?2 AND batch_id = ?3 AND superseded_by_derivation_id IS NULL",
        params![input.staged_role, input.item_id, input.batch_id],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::ItemNotFound);
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn merge_capture_card(
    connection: &mut Connection,
    input: MergeCaptureCard,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    if input.item_ids.is_empty() || input.item_ids.len() > MAX_CAPTURE_BATCH_ITEMS as usize {
        return Err(CaptureInboxError::InvalidInput);
    }
    let unique_item_ids = input.item_ids.iter().collect::<BTreeSet<_>>();
    if unique_item_ids.len() != input.item_ids.len() {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let new_draft_subject = normalize_subject(input.new_draft_subject.as_deref().unwrap_or(""))?;
    let subject_override = if new_draft_subject.is_empty() || new_draft_subject == batch.subject {
        None
    } else {
        Some(new_draft_subject.as_str())
    };
    let transaction = connection.transaction()?;
    let target_draft_id = if let Some(target_draft_id) = input.target_draft_id {
        let target_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM capture_drafts WHERE id = ?1 AND batch_id = ?2)",
            params![target_draft_id, input.batch_id],
            |row| row.get(0),
        )?;
        if !target_exists {
            return Err(CaptureInboxError::DraftNotFound);
        }
        target_draft_id
    } else {
        let next_position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM capture_drafts WHERE batch_id = ?1",
            [input.batch_id.as_str()],
            |row| row.get(0),
        )?;
        insert_draft_with_subject(
            &transaction,
            &input.batch_id,
            usize::try_from(next_position).unwrap_or(usize::MAX),
            subject_override,
            input.now_utc_ms,
        )?
    };

    for item_id in &input.item_ids {
        invalidate_active_pairs_for_item(&transaction, item_id, input.now_utc_ms)?;
        let staged_role = transaction
            .query_row(
                "SELECT staged_role FROM capture_items
                 WHERE id = ?1 AND batch_id = ?2 AND superseded_by_derivation_id IS NULL",
                params![item_id, input.batch_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(CaptureInboxError::ItemNotFound)?;
        let source = transaction
            .query_row(
                "SELECT draft_id, role FROM capture_draft_items WHERE item_id = ?1",
                [item_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        transaction.execute(
            "DELETE FROM capture_draft_items WHERE item_id = ?1",
            [item_id.as_str()],
        )?;
        if let Some((source_draft_id, source_role)) = source {
            repack_link_positions(&transaction, &source_draft_id, &source_role)?;
        }
        let position: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM capture_draft_items WHERE draft_id = ?1 AND role = ?2",
            params![target_draft_id, staged_role],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
             VALUES(?1, ?2, ?3, ?4)",
            params![target_draft_id, item_id, staged_role, position],
        )?;
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn apply_capture_pair_suggestions(
    connection: &mut Connection,
    input: ApplyCapturePairSuggestions,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    if input.pair_ids.is_empty()
        || input.pair_ids.len() > MAX_CAPTURE_BATCH_ITEMS as usize
        || input.pair_ids.iter().collect::<BTreeSet<_>>().len() != input.pair_ids.len()
    {
        return Err(CaptureInboxError::InvalidInput);
    }
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let transaction = connection.transaction()?;
    let mut next_position: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM capture_drafts WHERE batch_id = ?1",
        [input.batch_id.as_str()],
        |row| row.get(0),
    )?;
    for pair_id in &input.pair_ids {
        let expected_item_count: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM capture_recognition_pairs pair
             JOIN capture_recognition_operations operation
               ON operation.id = pair.operation_id
              AND operation.batch_id = ?2
              AND operation.reverted_at_utc_ms IS NULL
             JOIN capture_recognition_pair_items pair_item
               ON pair_item.pair_id = pair.id
             WHERE pair.id = ?1 AND pair.state = 'active'",
            params![pair_id, input.batch_id],
            |row| row.get(0),
        )?;
        let rows = {
            let mut statement = transaction.prepare(
                "SELECT pair_item.item_id, pair_item.role
                 FROM capture_recognition_pairs pair
                 JOIN capture_recognition_operations operation
                   ON operation.id = pair.operation_id
                  AND operation.batch_id = ?2
                  AND operation.reverted_at_utc_ms IS NULL
                 JOIN capture_recognition_pair_items pair_item
                   ON pair_item.pair_id = pair.id
                 JOIN capture_items item
                   ON item.id = pair_item.item_id
                  AND item.batch_id = ?2
                  AND item.superseded_by_derivation_id IS NULL
                  AND item.staged_role = pair_item.role
                 LEFT JOIN capture_draft_items draft_item ON draft_item.item_id = item.id
                 WHERE pair.id = ?1
                   AND pair.state = 'active'
                   AND draft_item.item_id IS NULL
                 ORDER BY CASE pair_item.role WHEN 'question' THEN 0 ELSE 1 END,
                          item.source_sequence, item.id",
            )?;
            statement
                .query_map(params![pair_id, input.batch_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        if i64::try_from(rows.len()).unwrap_or(i64::MAX) != expected_item_count
            || rows.is_empty()
            || !rows.iter().any(|(_, role)| role == "question")
            || !rows.iter().any(|(_, role)| role == "answer")
        {
            return Err(CaptureInboxError::InvalidInput);
        }
        let draft_id = insert_draft_with_subject(
            &transaction,
            &input.batch_id,
            usize::try_from(next_position).unwrap_or(usize::MAX),
            None,
            input.now_utc_ms,
        )?;
        next_position = next_position.saturating_add(1);
        let mut question_position = 0usize;
        let mut answer_position = 0usize;
        for (item_id, role) in rows {
            let position = if role == "question" {
                let position = question_position;
                question_position += 1;
                position
            } else {
                let position = answer_position;
                answer_position += 1;
                position
            };
            insert_link(&transaction, &draft_id, &item_id, &role, position)?;
        }
        let changed = transaction.execute(
            "UPDATE capture_recognition_pairs
             SET state = 'applied', resolved_at_utc_ms = ?2
             WHERE id = ?1 AND state = 'active'",
            params![pair_id, input.now_utc_ms],
        )?;
        if changed != 1 {
            return Err(CaptureInboxError::InvalidInput);
        }
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn delete_capture_draft(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    draft_id: &str,
    expected_revision: u32,
    now_utc_ms: i64,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    ensure_organizing_revision(&batch, expected_revision)?;
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "DELETE FROM capture_drafts WHERE id = ?1 AND batch_id = ?2",
        params![draft_id, batch_id],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::DraftNotFound);
    }
    repack_draft_positions(&transaction, batch_id)?;
    touch_batch(&transaction, batch_id, now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(connection, account_id, profile_id, batch_id)
}

pub fn update_capture_draft(
    connection: &mut Connection,
    input: UpdateCaptureDraft,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )?;
    ensure_organizing_revision(&batch, input.expected_revision)?;
    let subject = normalize_subject(&input.subject)?;
    let note = input.note.trim();
    if note.chars().count() > 500 || input.tags.len() > 20 {
        return Err(CaptureInboxError::InvalidInput);
    }
    let mut seen = BTreeSet::new();
    let tags = input
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            if tag.chars().count() > 30 {
                Err(CaptureInboxError::InvalidInput)
            } else {
                Ok(tag)
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|tag| seen.insert(tag.clone()))
        .collect::<Vec<_>>();
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE capture_drafts
         SET subject_override = ?1, tags_json = ?2, note = ?3, updated_at_utc_ms = ?4
         WHERE id = ?5 AND batch_id = ?6",
        params![
            subject,
            serde_json::to_string(&tags)?,
            note,
            input.now_utc_ms,
            input.draft_id,
            input.batch_id
        ],
    )?;
    if changed != 1 {
        return Err(CaptureInboxError::DraftNotFound);
    }
    touch_batch(&transaction, &input.batch_id, input.now_utc_ms)?;
    transaction.commit()?;
    get_capture_batch_detail(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )
}

pub fn remove_capture_item(
    connection: &mut Connection,
    blob_root: &Path,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    expected_revision: u32,
    item_id: &str,
    now_utc_ms: i64,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    if batch.state == CaptureBatchState::Completed {
        return Err(CaptureInboxError::InvalidState);
    }
    if batch.revision != expected_revision {
        return Err(CaptureInboxError::RevisionConflict);
    }
    let is_crop_derivative: bool = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM asset_derivations
           WHERE derived_capture_item_id = ?1 AND batch_id = ?2
         )",
        params![item_id, batch_id],
        |row| row.get(0),
    )?;
    if is_crop_derivative {
        return Err(CaptureInboxError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    invalidate_active_pairs_for_item(&transaction, item_id, now_utc_ms)?;
    let asset = transaction
        .query_row(
            "SELECT a.id, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    validate_relative_asset_path(&asset.1)?;
    transaction.execute("DELETE FROM capture_items WHERE id = ?1", [item_id])?;
    touch_batch(&transaction, batch_id, now_utc_ms)?;
    let orphan = delete_asset_row_if_orphan(&transaction, &asset.0)?;
    transaction.commit()?;
    if orphan {
        let _ = remove_encrypted_blob(blob_root, &asset.1);
    }
    get_capture_batch_detail(connection, account_id, profile_id, batch_id)
}

pub fn discard_capture_batch(
    connection: &mut Connection,
    blob_root: &Path,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<(), CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let transaction = connection.transaction()?;
    let candidates = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT a.id, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.batch_id = ?1 AND a.account_id = ?2",
        )?;
        statement
            .query_map(params![batch_id, account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (_, encrypted_path) in &candidates {
        validate_relative_asset_path(encrypted_path)?;
    }
    transaction.execute(
        "DELETE FROM capture_batches WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
        params![batch_id, account_id, profile_id],
    )?;
    let mut orphan_paths = Vec::new();
    for (asset_id, encrypted_path) in candidates {
        if delete_asset_row_if_orphan(&transaction, &asset_id)? {
            orphan_paths.push(encrypted_path);
        }
    }
    transaction.commit()?;
    for encrypted_path in orphan_paths {
        let _ = remove_encrypted_blob(blob_root, &encrypted_path);
    }
    Ok(())
}
