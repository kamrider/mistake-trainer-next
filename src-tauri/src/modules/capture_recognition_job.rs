use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    CaptureRecognitionDecision, CaptureRecognitionError, CaptureRecognitionJob,
    CaptureRecognitionJobState, CaptureRecognitionReviewBand, CaptureRecognitionRole,
    CaptureRecognitionSuggestion, CaptureRecognitionSuggestionState, ClaimedCaptureRecognitionItem,
    CreateCaptureRecognitionJob, MAX_JOB_ITEMS, ReviewCaptureRecognitionSuggestion,
    StoreCaptureRecognitionSuggestion, capture_item_snapshot_hash, review_band, validate_regions,
};

pub fn create_or_resume_recognition_job(
    connection: &mut Connection,
    input: CreateCaptureRecognitionJob,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if input.item_ids.is_empty()
        || input.item_ids.len() > MAX_JOB_ITEMS
        || input.engine.trim().is_empty()
        || input.engine.len() > 60
        || input.engine_version.trim().is_empty()
        || input.engine_version.len() > 60
    {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let unique = input.item_ids.iter().collect::<BTreeSet<_>>();
    if unique.len() != input.item_ids.len() {
        return Err(CaptureRecognitionError::InvalidInput);
    }

    let batch_state = connection
        .query_row(
            "SELECT state FROM capture_batches
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.batch_id, input.account_id, input.profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::BatchNotFound)?;
    if batch_state != "organizing" {
        return Err(CaptureRecognitionError::InvalidState);
    }

    if let Some(existing) = get_active_recognition_job(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.batch_id,
    )? {
        return Ok(existing);
    }

    let mut snapshots = Vec::with_capacity(input.item_ids.len());
    for item_id in &input.item_ids {
        snapshots.push(capture_item_snapshot_hash(
            connection,
            &input.account_id,
            &input.profile_id,
            &input.batch_id,
            item_id,
        )?);
    }

    let id = Uuid::now_v7().to_string();
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO capture_recognition_jobs(
           id, account_id, profile_id, batch_id, state, engine, engine_version,
           model_component_id, total_items, processed_items, created_at_utc_ms,
           updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, 'queued', ?5, ?6, 'ppocrv6_small', ?7, 0, ?8, ?8)",
        params![
            id,
            input.account_id,
            input.profile_id,
            input.batch_id,
            input.engine,
            input.engine_version,
            i64::try_from(input.item_ids.len()).unwrap_or(i64::MAX),
            input.now_utc_ms,
        ],
    )?;
    for (position, (item_id, snapshot)) in input.item_ids.iter().zip(snapshots).enumerate() {
        transaction.execute(
            "INSERT INTO capture_recognition_job_items(
               job_id, item_id, source_snapshot_hash, position, state
             ) VALUES(?1, ?2, ?3, ?4, 'pending')",
            params![
                id,
                item_id,
                snapshot.as_slice(),
                i64::try_from(position).unwrap_or(i64::MAX),
            ],
        )?;
    }
    transaction.commit()?;
    get_recognition_job_by_id(connection, &input.account_id, &input.profile_id, &id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn get_active_recognition_job(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionError> {
    let id = connection
        .query_row(
            "SELECT id FROM capture_recognition_jobs
             WHERE account_id = ?1 AND profile_id = ?2 AND batch_id = ?3
               AND state IN ('queued', 'running', 'review')
             ORDER BY updated_at_utc_ms DESC, id DESC
             LIMIT 1",
            params![account_id, profile_id, batch_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match id {
        Some(id) => get_recognition_job_by_id(connection, account_id, profile_id, &id),
        None => Ok(None),
    }
}

pub fn get_recognition_job_by_id(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionError> {
    let row = connection
        .query_row(
            "SELECT id, batch_id, state, total_items, processed_items,
                    created_at_utc_ms, updated_at_utc_ms
             FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![job_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((id, batch_id, state, total_items, processed_items, created, updated)) = row else {
        return Ok(None);
    };
    Ok(Some(CaptureRecognitionJob {
        suggestions: list_suggestions(connection, &id)?,
        id,
        batch_id,
        state: CaptureRecognitionJobState::from_database(&state)?,
        total_items: u32::try_from(total_items).unwrap_or(u32::MAX),
        processed_items: u32::try_from(processed_items).unwrap_or(u32::MAX),
        created_at_utc_ms: created as f64,
        updated_at_utc_ms: updated as f64,
    }))
}

pub fn store_recognition_suggestion(
    connection: &mut Connection,
    input: StoreCaptureRecognitionSuggestion,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    validate_regions(&input.regions, input.confidence_basis_points)?;
    let job_item_exists: bool = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM capture_recognition_jobs j
           JOIN capture_recognition_job_items ji ON ji.job_id = j.id
           WHERE j.id = ?1 AND j.account_id = ?2 AND j.profile_id = ?3
             AND ji.item_id = ?4 AND j.state IN ('queued', 'running')
         )",
        params![
            input.job_id,
            input.account_id,
            input.profile_id,
            input.item_id
        ],
        |row| row.get(0),
    )?;
    if !job_item_exists {
        return Err(CaptureRecognitionError::ItemNotFound);
    }
    let band = review_band(input.confidence_basis_points);
    let suggestion_id = Uuid::now_v7().to_string();
    let regions_json = serde_json::to_string(&input.regions)?;
    let reasons_json = serde_json::to_string(&input.reason_codes)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO capture_recognition_suggestions(
           id, job_id, item_id, regions_json, confidence_basis_points,
           review_band, state, reason_codes_json
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'proposed', ?7)
         ON CONFLICT(job_id, item_id) DO UPDATE SET
           regions_json = excluded.regions_json,
           confidence_basis_points = excluded.confidence_basis_points,
           review_band = excluded.review_band,
           state = 'proposed',
           reason_codes_json = excluded.reason_codes_json,
           reviewed_at_utc_ms = NULL",
        params![
            suggestion_id,
            input.job_id,
            input.item_id,
            regions_json,
            input.confidence_basis_points,
            band.as_str(),
            reasons_json,
        ],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = 'complete'
         WHERE job_id = ?1 AND item_id = ?2",
        params![input.job_id, input.item_id],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET processed_items = (
           SELECT COUNT(*) FROM capture_recognition_job_items
           WHERE job_id = ?1 AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
         ),
         state = CASE
           WHEN (SELECT COUNT(*) FROM capture_recognition_job_items
                 WHERE job_id = ?1
                   AND state NOT IN ('complete', 'no_suggestion', 'stale', 'failed')) = 0
           THEN 'review'
           ELSE 'running'
         END,
         updated_at_utc_ms = ?2
         WHERE id = ?1",
        params![input.job_id, input.now_utc_ms],
    )?;
    transaction.commit()?;
    get_recognition_job_by_id(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.job_id,
    )?
    .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn review_recognition_suggestion(
    connection: &mut Connection,
    input: ReviewCaptureRecognitionSuggestion,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if let Some(regions) = input.edited_regions.as_deref() {
        let confidence = regions
            .iter()
            .map(|region| region.confidence_basis_points)
            .min()
            .unwrap_or_default();
        validate_regions(regions, confidence)?;
    }
    let job_state = connection
        .query_row(
            "SELECT state FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![input.job_id, input.account_id, input.profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::JobNotFound)?;
    if job_state != "review" {
        return Err(CaptureRecognitionError::InvalidState);
    }
    let suggestion = connection
        .query_row(
            "SELECT state, review_band, regions_json
             FROM capture_recognition_suggestions
             WHERE id = ?1 AND job_id = ?2",
            params![input.suggestion_id, input.job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(CaptureRecognitionError::ItemNotFound)?;
    if suggestion.0 == "stale"
        || (input.decision == CaptureRecognitionDecision::Accepted && suggestion.1 == "low")
    {
        return Err(CaptureRecognitionError::InvalidSuggestion);
    }
    let regions_json = match input.edited_regions {
        Some(regions) => serde_json::to_string(&regions)?,
        None => suggestion.2,
    };
    connection.execute(
        "UPDATE capture_recognition_suggestions
         SET state = ?1, regions_json = ?2, reviewed_at_utc_ms = ?3
         WHERE id = ?4 AND job_id = ?5",
        params![
            match input.decision {
                CaptureRecognitionDecision::Accepted => "accepted",
                CaptureRecognitionDecision::Rejected => "rejected",
            },
            regions_json,
            input.now_utc_ms,
            input.suggestion_id,
            input.job_id,
        ],
    )?;
    connection.execute(
        "UPDATE capture_recognition_jobs SET updated_at_utc_ms = ?1 WHERE id = ?2",
        params![input.now_utc_ms, input.job_id],
    )?;
    get_recognition_job_by_id(
        connection,
        &input.account_id,
        &input.profile_id,
        &input.job_id,
    )?
    .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn cancel_recognition_job(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    let changed = connection.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'cancelled', updated_at_utc_ms = ?1
         WHERE id = ?2 AND account_id = ?3 AND profile_id = ?4
           AND state IN ('queued', 'running')",
        params![now_utc_ms, job_id, account_id, profile_id],
    )?;
    if changed == 0 {
        let state = connection
            .query_row(
                "SELECT state FROM capture_recognition_jobs
                 WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
                params![job_id, account_id, profile_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(CaptureRecognitionError::JobNotFound)?;
        if state != "cancelled" {
            return Err(CaptureRecognitionError::InvalidState);
        }
    }
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn reset_abandoned_recognition_work(
    connection: &mut Connection,
    now_utc_ms: i64,
) -> Result<u32, CaptureRecognitionError> {
    let transaction = connection.transaction()?;
    // Pairing anchors are intentionally memory-only. Re-run every item in an
    // interrupted job so a restart cannot reuse an opaque slot for a different
    // question or leave a matching answer detached from its question.
    transaction.execute(
        "DELETE FROM capture_recognition_suggestions
         WHERE job_id IN (
           SELECT id FROM capture_recognition_jobs WHERE state = 'running'
         )",
        [],
    )?;
    let reset_items = transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = 'pending'
         WHERE state != 'pending'
           AND job_id IN (
             SELECT id FROM capture_recognition_jobs WHERE state = 'running'
           )",
        [],
    )?;
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'queued',
             processed_items = 0,
             updated_at_utc_ms = ?1
         WHERE state = 'running'",
        [now_utc_ms],
    )?;
    transaction.commit()?;
    Ok(u32::try_from(reset_items).unwrap_or(u32::MAX))
}

pub fn claim_next_recognition_item(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    now_utc_ms: i64,
) -> Result<Option<ClaimedCaptureRecognitionItem>, CaptureRecognitionError> {
    let job_state = connection
        .query_row(
            "SELECT state FROM capture_recognition_jobs
             WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3",
            params![job_id, account_id, profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(CaptureRecognitionError::JobNotFound)?;
    if !matches!(job_state.as_str(), "queued" | "running") {
        return Ok(None);
    }
    let transaction = connection.transaction()?;
    let row = transaction
        .query_row(
            "SELECT ji.item_id, j.batch_id, a.encrypted_path, a.media_type,
                    i.staged_role, ji.source_snapshot_hash
             FROM capture_recognition_job_items ji
             JOIN capture_recognition_jobs j ON j.id = ji.job_id
             JOIN capture_items i ON i.id = ji.item_id AND i.batch_id = j.batch_id
             JOIN assets a ON a.id = i.asset_id AND a.account_id = j.account_id
             WHERE ji.job_id = ?1 AND ji.state = 'pending'
               AND j.account_id = ?2 AND j.profile_id = ?3
             ORDER BY ji.position
             LIMIT 1",
            params![job_id, account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((item_id, batch_id, encrypted_path, media_type, staged_role, snapshot)) = row else {
        transaction.execute(
            "UPDATE capture_recognition_jobs
             SET processed_items = (
               SELECT COUNT(*) FROM capture_recognition_job_items
               WHERE job_id = ?1
                 AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
             ),
             state = CASE
               WHEN EXISTS(
                 SELECT 1 FROM capture_recognition_job_items
                 WHERE job_id = ?1 AND state = 'running'
               ) THEN 'running'
               ELSE 'review'
             END,
             updated_at_utc_ms = ?2
             WHERE id = ?1 AND state IN ('queued', 'running')",
            params![job_id, now_utc_ms],
        )?;
        transaction.commit()?;
        return Ok(None);
    };
    let snapshot: [u8; 32] = snapshot
        .try_into()
        .map_err(|_| CaptureRecognitionError::InvalidInput)?;
    let role = match staged_role.as_str() {
        "answer" => CaptureRecognitionRole::Answer,
        "question" => CaptureRecognitionRole::Question,
        _ => return Err(CaptureRecognitionError::InvalidInput),
    };
    let changed = transaction.execute(
        "UPDATE capture_recognition_job_items SET state = 'running'
         WHERE job_id = ?1 AND item_id = ?2 AND state = 'pending'",
        params![job_id, item_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::InvalidState);
    }
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'running', updated_at_utc_ms = ?1
         WHERE id = ?2 AND state IN ('queued', 'running')",
        params![now_utc_ms, job_id],
    )?;
    transaction.commit()?;
    Ok(Some(ClaimedCaptureRecognitionItem {
        item_id,
        batch_id,
        encrypted_path,
        media_type,
        staged_role: role,
        source_snapshot_hash: snapshot,
    }))
}

pub fn finish_recognition_item_without_suggestion(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    item_id: &str,
    state: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if !matches!(state, "no_suggestion" | "stale" | "failed") {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE capture_recognition_job_items
         SET state = ?1
         WHERE job_id = ?2 AND item_id = ?3 AND state IN ('pending', 'running')",
        params![state, job_id, item_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::ItemNotFound);
    }
    let owned: bool = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM capture_recognition_jobs
           WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3
             AND state IN ('queued', 'running')
         )",
        params![job_id, account_id, profile_id],
        |row| row.get(0),
    )?;
    if !owned {
        return Err(CaptureRecognitionError::JobNotFound);
    }
    transaction.execute(
        "UPDATE capture_recognition_jobs
         SET processed_items = (
           SELECT COUNT(*) FROM capture_recognition_job_items
           WHERE job_id = ?1
             AND state IN ('complete', 'no_suggestion', 'stale', 'failed')
         ),
         state = CASE
           WHEN EXISTS(
             SELECT 1 FROM capture_recognition_job_items
             WHERE job_id = ?1 AND state IN ('pending', 'running')
           ) THEN 'running'
           ELSE 'review'
         END,
         updated_at_utc_ms = ?2
         WHERE id = ?1",
        params![job_id, now_utc_ms],
    )?;
    transaction.commit()?;
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

pub fn fail_recognition_job(
    connection: &mut Connection,
    account_id: &str,
    profile_id: &str,
    job_id: &str,
    failure_code: &str,
    now_utc_ms: i64,
) -> Result<CaptureRecognitionJob, CaptureRecognitionError> {
    if failure_code.trim().is_empty() || failure_code.len() > 80 {
        return Err(CaptureRecognitionError::InvalidInput);
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE capture_recognition_job_items SET state = 'failed'
         WHERE job_id = ?1 AND state IN ('pending', 'running')",
        [job_id],
    )?;
    let changed = transaction.execute(
        "UPDATE capture_recognition_jobs
         SET state = 'failed', processed_items = total_items,
             failure_code = ?1, updated_at_utc_ms = ?2
         WHERE id = ?3 AND account_id = ?4 AND profile_id = ?5
           AND state IN ('queued', 'running')",
        params![failure_code, now_utc_ms, job_id, account_id, profile_id],
    )?;
    if changed != 1 {
        return Err(CaptureRecognitionError::JobNotFound);
    }
    transaction.commit()?;
    get_recognition_job_by_id(connection, account_id, profile_id, job_id)?
        .ok_or(CaptureRecognitionError::JobNotFound)
}

fn list_suggestions(
    connection: &Connection,
    job_id: &str,
) -> Result<Vec<CaptureRecognitionSuggestion>, CaptureRecognitionError> {
    let mut statement = connection.prepare(
        "SELECT id, item_id, regions_json, confidence_basis_points,
                review_band, state, reason_codes_json
         FROM capture_recognition_suggestions
         WHERE job_id = ?1
         ORDER BY rowid",
    )?;
    let rows = statement.query_map([job_id], |row| {
        let regions_json = row.get::<_, String>(2)?;
        let reasons_json = row.get::<_, String>(6)?;
        let confidence = row.get::<_, i64>(3)?;
        Ok(CaptureRecognitionSuggestion {
            id: row.get(0)?,
            item_id: row.get(1)?,
            regions: serde_json::from_str(&regions_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            confidence_basis_points: u16::try_from(confidence).unwrap_or(u16::MAX),
            review_band: CaptureRecognitionReviewBand::from_database(&row.get::<_, String>(4)?)?,
            state: CaptureRecognitionSuggestionState::from_database(&row.get::<_, String>(5)?)?,
            reason_codes: serde_json::from_str(&reasons_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CaptureRecognitionError::from)
}
