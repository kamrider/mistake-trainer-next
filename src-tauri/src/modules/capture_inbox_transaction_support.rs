use rusqlite::{Transaction, params};

pub(super) fn invalidate_active_pairs_for_item(
    transaction: &Transaction<'_>,
    item_id: &str,
    now_utc_ms: i64,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_recognition_pairs
         SET state = 'invalidated', resolved_at_utc_ms = ?2
         WHERE state = 'active'
           AND EXISTS(
             SELECT 1
             FROM capture_recognition_pair_items pair_item
             WHERE pair_item.pair_id = capture_recognition_pairs.id
               AND pair_item.item_id = ?1
           )",
        params![item_id, now_utc_ms],
    )?;
    Ok(())
}

pub(super) fn touch_batch(
    transaction: &Transaction<'_>,
    batch_id: &str,
    now_utc_ms: i64,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "UPDATE capture_batches SET updated_at_utc_ms = ?2, revision = revision + 1 WHERE id = ?1",
        params![batch_id, now_utc_ms],
    )?;
    Ok(())
}

pub(super) fn repack_link_positions(
    transaction: &Transaction<'_>,
    draft_id: &str,
    role: &str,
) -> Result<(), rusqlite::Error> {
    let item_ids = {
        let mut statement = transaction.prepare(
            "SELECT item_id FROM capture_draft_items
             WHERE draft_id = ?1 AND role = ?2 ORDER BY position, item_id",
        )?;
        statement
            .query_map(params![draft_id, role], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (position, item_id) in item_ids.iter().enumerate() {
        transaction.execute(
            "UPDATE capture_draft_items SET position = ?1 WHERE item_id = ?2",
            params![i64::try_from(position).unwrap_or(i64::MAX), item_id],
        )?;
    }
    Ok(())
}

pub(super) fn delete_asset_row_if_orphan(
    transaction: &Transaction<'_>,
    asset_id: &str,
) -> Result<bool, rusqlite::Error> {
    let referenced: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM capture_items WHERE asset_id = ?1
            UNION ALL SELECT 1 FROM problem_assets WHERE asset_id = ?1
         )",
        [asset_id],
        |row| row.get(0),
    )?;
    if referenced {
        return Ok(false);
    }
    Ok(transaction.execute("DELETE FROM assets WHERE id = ?1", [asset_id])? == 1)
}
