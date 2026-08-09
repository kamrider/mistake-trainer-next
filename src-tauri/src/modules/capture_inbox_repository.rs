use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{
    CaptureBatchDetail, CaptureBatchState, CaptureBatchSummary, CaptureDraftSummary,
    CaptureInboxError, CaptureItemSummary, CapturePairSuggestionSummary,
};

pub fn list_capture_batches(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<Vec<CaptureBatchSummary>, CaptureInboxError> {
    let mut statement = connection.prepare(
        "SELECT b.id, b.subject, b.state,
                (SELECT COUNT(*) FROM capture_items i WHERE i.batch_id = b.id AND i.superseded_by_derivation_id IS NULL),
                (SELECT COUNT(*) FROM capture_drafts d WHERE d.batch_id = b.id),
                (SELECT COUNT(*) FROM capture_drafts d
                 WHERE d.batch_id = b.id
                   AND trim(COALESCE(NULLIF(d.subject_override, ''), b.subject)) <> ''
                   AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
                   AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')),
                b.updated_at_utc_ms, b.revision
         FROM capture_batches b
         WHERE b.account_id = ?1 AND b.profile_id = ?2
         ORDER BY CASE b.state WHEN 'collecting' THEN 0 WHEN 'organizing' THEN 1 ELSE 2 END,
                  b.updated_at_utc_ms DESC, b.id",
    )?;
    statement
        .query_map(params![account_id, profile_id], map_batch_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CaptureInboxError::from)
}

pub fn get_capture_batch_detail(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchDetail, CaptureInboxError> {
    let batch = query_batch(connection, account_id, profile_id, batch_id)?;
    let mut item_statement = connection.prepare(
        "SELECT i.id, i.source_name, i.source_sequence, a.media_type, a.byte_length,
                i.width, i.height, i.staged_role, di.draft_id, di.role, di.position,
                derivation.id, derivation.source_capture_item_id
         FROM capture_items i
         JOIN assets a ON a.id = i.asset_id AND a.account_id = ?1
         LEFT JOIN capture_draft_items di ON di.item_id = i.id
         LEFT JOIN asset_derivations derivation ON derivation.derived_capture_item_id = i.id
         WHERE i.batch_id = ?2 AND i.superseded_by_derivation_id IS NULL
         ORDER BY i.source_sequence, i.id",
    )?;
    let items = item_statement
        .query_map(params![account_id, batch_id], map_item_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut draft_statement = connection.prepare(
        "SELECT id, position, COALESCE(NULLIF(subject_override, ''), ?2), tags_json, note
         FROM capture_drafts WHERE batch_id = ?1 ORDER BY position, id",
    )?;
    let mut drafts = draft_statement
        .query_map(params![batch_id, batch.subject], |row| {
            let tags_json: String = row.get(3)?;
            Ok(CaptureDraftSummary {
                id: row.get(0)?,
                position: row.get(1)?,
                subject: row.get(2)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                note: row.get(4)?,
                question_item_ids: Vec::new(),
                answer_item_ids: Vec::new(),
                ready: false,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let draft_indexes = drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| (draft.id.clone(), index))
        .collect::<HashMap<_, _>>();
    for item in &items {
        let (Some(draft_id), Some(role)) = (&item.draft_id, &item.role) else {
            continue;
        };
        if let Some(index) = draft_indexes.get(draft_id) {
            if role == "question" {
                drafts[*index].question_item_ids.push(item.id.clone());
            } else if role == "answer" {
                drafts[*index].answer_item_ids.push(item.id.clone());
            }
        }
    }
    for draft in &mut drafts {
        draft.ready = !draft.subject.trim().is_empty()
            && !draft.question_item_ids.is_empty()
            && !draft.answer_item_ids.is_empty();
    }
    let unassigned_item_ids = items
        .iter()
        .filter(|item| item.draft_id.is_none())
        .map(|item| item.id.clone())
        .collect();
    let mut pair_statement = connection.prepare(
        "SELECT pair.id, pair.confidence_basis_points, pair_item.item_id, pair_item.role,
                (SELECT COUNT(*)
                   FROM capture_recognition_pair_items stored_item
                  WHERE stored_item.pair_id = pair.id)
         FROM capture_recognition_pairs pair
         JOIN capture_recognition_operations operation
           ON operation.id = pair.operation_id AND operation.batch_id = ?1
         JOIN capture_recognition_pair_items pair_item ON pair_item.pair_id = pair.id
         JOIN capture_items item
           ON item.id = pair_item.item_id
          AND item.batch_id = ?1
          AND item.superseded_by_derivation_id IS NULL
          AND item.staged_role = pair_item.role
         LEFT JOIN capture_draft_items draft_item ON draft_item.item_id = item.id
         WHERE pair.state = 'active'
           AND operation.reverted_at_utc_ms IS NULL
           AND draft_item.item_id IS NULL
         ORDER BY operation.created_at_utc_ms, pair.pair_slot,
                  CASE pair_item.role WHEN 'question' THEN 0 ELSE 1 END,
                  item.source_sequence, item.id",
    )?;
    let pair_rows = pair_statement
        .query_map([batch_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u16>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut pair_suggestions = Vec::<CapturePairSuggestionSummary>::new();
    let mut pair_indexes = HashMap::<String, usize>::new();
    let mut pair_expected_item_counts = HashMap::<String, i64>::new();
    for (pair_id, confidence_basis_points, item_id, role, expected_item_count) in pair_rows {
        pair_expected_item_counts.insert(pair_id.clone(), expected_item_count);
        let index = if let Some(index) = pair_indexes.get(&pair_id) {
            *index
        } else {
            let index = pair_suggestions.len();
            pair_indexes.insert(pair_id.clone(), index);
            pair_suggestions.push(CapturePairSuggestionSummary {
                id: pair_id,
                question_item_ids: Vec::new(),
                answer_item_ids: Vec::new(),
                confidence_basis_points,
            });
            index
        };
        if role == "question" {
            pair_suggestions[index].question_item_ids.push(item_id);
        } else if role == "answer" {
            pair_suggestions[index].answer_item_ids.push(item_id);
        }
    }
    pair_suggestions.retain(|suggestion| {
        !suggestion.question_item_ids.is_empty()
            && !suggestion.answer_item_ids.is_empty()
            && i64::try_from(suggestion.question_item_ids.len() + suggestion.answer_item_ids.len())
                .unwrap_or(i64::MAX)
                == pair_expected_item_counts
                    .get(&suggestion.id)
                    .copied()
                    .unwrap_or_default()
    });
    Ok(CaptureBatchDetail {
        batch,
        items,
        drafts,
        unassigned_item_ids,
        pair_suggestions,
    })
}

pub fn query_batch(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<CaptureBatchSummary, CaptureInboxError> {
    connection
        .query_row(
            "SELECT b.id, b.subject, b.state,
                    (SELECT COUNT(*) FROM capture_items i WHERE i.batch_id = b.id AND i.superseded_by_derivation_id IS NULL),
                    (SELECT COUNT(*) FROM capture_drafts d WHERE d.batch_id = b.id),
                    (SELECT COUNT(*) FROM capture_drafts d
                     WHERE d.batch_id = b.id
                       AND trim(COALESCE(NULLIF(d.subject_override, ''), b.subject)) <> ''
                       AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'question')
                       AND EXISTS(SELECT 1 FROM capture_draft_items di WHERE di.draft_id = d.id AND di.role = 'answer')),
                    b.updated_at_utc_ms, b.revision
             FROM capture_batches b
             WHERE b.id = ?1 AND b.account_id = ?2 AND b.profile_id = ?3",
            params![batch_id, account_id, profile_id],
            map_batch_row,
        )
        .optional()?
        .ok_or(CaptureInboxError::BatchNotFound)
}

pub fn get_capture_item(
    connection: &Connection,
    account_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureItemSummary, CaptureInboxError> {
    connection
        .query_row(
            "SELECT i.id, i.source_name, i.source_sequence, a.media_type, a.byte_length,
                    i.width, i.height, i.staged_role, di.draft_id, di.role, di.position,
                    derivation.id, derivation.source_capture_item_id
             FROM capture_items i
             JOIN assets a ON a.id = i.asset_id AND a.account_id = ?1
             LEFT JOIN capture_draft_items di ON di.item_id = i.id
             LEFT JOIN asset_derivations derivation ON derivation.derived_capture_item_id = i.id
             WHERE i.batch_id = ?2 AND i.id = ?3",
            params![account_id, batch_id, item_id],
            map_item_row,
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)
}

fn map_batch_row(row: &Row<'_>) -> rusqlite::Result<CaptureBatchSummary> {
    let state: String = row.get(2)?;
    Ok(CaptureBatchSummary {
        id: row.get(0)?,
        subject: row.get(1)?,
        state: parse_state(&state),
        item_count: row.get(3)?,
        draft_count: row.get(4)?,
        ready_count: row.get(5)?,
        updated_at_utc_ms: row.get::<_, i64>(6)? as f64,
        revision: row.get(7)?,
    })
}

fn map_item_row(row: &Row<'_>) -> rusqlite::Result<CaptureItemSummary> {
    Ok(CaptureItemSummary {
        id: row.get(0)?,
        source_name: row.get(1)?,
        source_sequence: row.get(2)?,
        media_type: row.get(3)?,
        byte_length: row.get::<_, i64>(4)? as f64,
        width: row.get(5)?,
        height: row.get(6)?,
        staged_role: row.get(7)?,
        draft_id: row.get(8)?,
        role: row.get(9)?,
        position: row.get(10)?,
        crop_derivation_id: row.get(11)?,
        crop_source_item_id: row.get(12)?,
    })
}

fn parse_state(value: &str) -> CaptureBatchState {
    match value {
        "collecting" => CaptureBatchState::Collecting,
        "completed" => CaptureBatchState::Completed,
        _ => CaptureBatchState::Organizing,
    }
}
