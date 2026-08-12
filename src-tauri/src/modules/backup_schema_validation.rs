use rusqlite::{Connection, OptionalExtension};

use super::{BackupError, MAX_DATABASE_BYTES};

pub(super) fn ensure_database_budget(connection: &Connection) -> Result<(), BackupError> {
    let page_count = pragma_u64(connection, "page_count")?;
    let page_size = pragma_u64(connection, "page_size")?;
    let estimated_bytes = page_count
        .checked_mul(page_size)
        .ok_or(BackupError::TooLarge)?;
    if estimated_bytes > MAX_DATABASE_BYTES {
        return Err(BackupError::TooLarge);
    }
    Ok(())
}

fn pragma_u64(connection: &Connection, name: &str) -> Result<u64, BackupError> {
    let value: rusqlite::types::Value =
        connection.pragma_query_value(None, name, |row| row.get(0))?;
    match value {
        rusqlite::types::Value::Integer(value) => {
            u64::try_from(value).map_err(|_| BackupError::TooLarge)
        }
        rusqlite::types::Value::Text(value) => {
            value.parse::<u64>().map_err(|_| BackupError::TooLarge)
        }
        _ => Err(BackupError::TooLarge),
    }
}

pub(super) fn ensure_single_account(
    connection: &Connection,
    account_id: &str,
    schema_version: i64,
) -> Result<(), BackupError> {
    let recognition_tables = [
        "capture_recognition_jobs",
        "capture_recognition_job_items",
        "capture_recognition_suggestions",
        "capture_recognition_operations",
    ];
    let recognition_table_count = recognition_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 14 && recognition_table_count != 0)
        || (schema_version >= 14 && recognition_table_count != recognition_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 14 {
        if !index_columns_match(
            connection,
            "capture_recognition_jobs_batch_idx",
            &["account_id", "profile_id", "batch_id", "updated_at_utc_ms"],
        )? {
            return Err(BackupError::Integrity);
        }
        let has_foreign_recognition: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM capture_recognition_jobs job
               LEFT JOIN learner_profiles profile ON profile.id = job.profile_id
               LEFT JOIN capture_batches batch ON batch.id = job.batch_id
               WHERE job.account_id <> ?1
                  OR profile.account_id <> ?1
                  OR batch.account_id <> ?1
                  OR batch.profile_id <> job.profile_id
               LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_recognition != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let recognition_pair_tables = [
        "capture_recognition_pairs",
        "capture_recognition_pair_items",
    ];
    let recognition_pair_table_count = recognition_pair_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 16 && recognition_pair_table_count != 0)
        || (schema_version >= 16 && recognition_pair_table_count != recognition_pair_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    if schema_version == 16
        && !table_columns_match(
            connection,
            "capture_recognition_pairs",
            &[
                "id",
                "operation_id",
                "pair_slot",
                "confidence_basis_points",
                "created_at_utc_ms",
            ],
        )?
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 17
        && !table_columns_match(
            connection,
            "capture_recognition_pairs",
            &[
                "id",
                "operation_id",
                "pair_slot",
                "confidence_basis_points",
                "created_at_utc_ms",
                "state",
                "resolved_at_utc_ms",
            ],
        )?
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 16
        && (!table_columns_match(
            connection,
            "capture_recognition_pair_items",
            &["pair_id", "item_id", "role"],
        )? || !index_columns_match(
            connection,
            "capture_recognition_pairs_operation_idx",
            &["operation_id", "pair_slot", "id"],
        )? || !index_columns_match(
            connection,
            "capture_recognition_pair_items_pair_idx",
            &["pair_id", "role", "item_id"],
        )?)
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 16 {
        let has_invalid_pair_relationship: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM capture_recognition_pairs pair
               LEFT JOIN capture_recognition_operations operation
                 ON operation.id = pair.operation_id
               LEFT JOIN capture_recognition_jobs job
                 ON job.id = operation.job_id
               LEFT JOIN capture_batches operation_batch
                 ON operation_batch.id = operation.batch_id
               LEFT JOIN learner_profiles profile
                 ON profile.id = job.profile_id
               LEFT JOIN capture_recognition_pair_items pair_item
                 ON pair_item.pair_id = pair.id
               LEFT JOIN capture_items item
                 ON item.id = pair_item.item_id
               WHERE operation.id IS NULL
                  OR job.id IS NULL
                  OR operation_batch.id IS NULL
                  OR profile.id IS NULL
                  OR (pair_item.item_id IS NOT NULL AND item.id IS NULL)
                  OR job.account_id <> ?1
                  OR profile.account_id <> ?1
                  OR operation_batch.account_id <> ?1
                  OR job.profile_id <> operation_batch.profile_id
                  OR job.batch_id <> operation.batch_id
                  OR item.batch_id <> operation.batch_id
               LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_invalid_pair_relationship != 0 {
            return Err(BackupError::Integrity);
        }
    }
    if schema_version >= 17 {
        let has_invalid_pair_state: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM capture_recognition_pairs
               WHERE (state = 'active' AND resolved_at_utc_ms IS NOT NULL)
                  OR (state <> 'active' AND resolved_at_utc_ms IS NULL)
               LIMIT 1
             )",
            [],
            |row| row.get(0),
        )?;
        if has_invalid_pair_state != 0 {
            return Err(BackupError::Integrity);
        }
    }
    let has_sync_snapshot_table = table_exists(connection, "sync_entity_snapshots")?;
    let has_conflict_resolution = column_exists(connection, "sync_conflicts", "resolution")?;
    let has_conflict_resolved_value =
        column_exists(connection, "sync_conflicts", "resolved_value_json")?;
    if (schema_version < 13
        && (has_sync_snapshot_table || has_conflict_resolution || has_conflict_resolved_value))
        || (schema_version >= 13
            && (!has_sync_snapshot_table
                || !has_conflict_resolution
                || !has_conflict_resolved_value))
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 13 {
        if !table_columns_match(
            connection,
            "sync_entity_snapshots",
            &[
                "account_id",
                "profile_id",
                "entity_type",
                "entity_id",
                "revision",
                "payload_json",
                "updated_at_utc_ms",
            ],
        )? || !index_columns_match(
            connection,
            "sync_entity_snapshots_profile_idx",
            &["account_id", "profile_id", "entity_type", "entity_id"],
        )? || !index_columns_match(
            connection,
            "sync_conflicts_open_field_idx",
            &["account_id", "entity_type", "entity_id", "field_name"],
        )? {
            return Err(BackupError::Integrity);
        }
        let has_foreign_snapshot: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sync_entity_snapshots WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_snapshot != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let derivation_tables = ["asset_derivations", "capture_source_retention"];
    let derivation_table_count = derivation_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    let has_superseded_column =
        column_exists(connection, "capture_items", "superseded_by_derivation_id")?;
    if (schema_version < 12 && (derivation_table_count != 0 || has_superseded_column))
        || (schema_version >= 12
            && (derivation_table_count != derivation_tables.len() || !has_superseded_column))
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 12
        && (!table_columns_match(
            connection,
            "asset_derivations",
            &[
                "id",
                "operation_id",
                "account_id",
                "batch_id",
                "source_asset_id",
                "derived_asset_id",
                "source_capture_item_id",
                "derived_capture_item_id",
                "position",
                "kind",
                "recipe_json",
                "engine",
                "engine_version",
                "confidence",
                "created_at_utc_ms",
            ],
        )? || !table_columns_match(
            connection,
            "capture_source_retention",
            &[
                "batch_id",
                "source_asset_id",
                "retain_until_utc_ms",
                "reason",
                "created_at_utc_ms",
            ],
        )? || !index_columns_match(
            connection,
            "capture_items_active_sequence_idx",
            &[
                "batch_id",
                "superseded_by_derivation_id",
                "source_sequence",
                "id",
            ],
        )?)
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 12 {
        let has_foreign_derivation: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM asset_derivations WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_derivation != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let cloud_tables = ["cloud_sync_state", "cloud_asset_transfers"];
    let cloud_table_count = cloud_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 11 && cloud_table_count != 0)
        || (schema_version >= 11 && cloud_table_count != cloud_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    let cloud_outbox_columns = ["lease_id", "lease_expires_at_utc_ms", "last_error_code"];
    let present_cloud_outbox_columns = cloud_outbox_columns
        .iter()
        .map(|column| column_exists(connection, "sync_operations", column))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 11 && present_cloud_outbox_columns != 0)
        || (schema_version >= 11 && present_cloud_outbox_columns != cloud_outbox_columns.len())
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 11 {
        if !table_columns_match(
            connection,
            "cloud_sync_state",
            &[
                "account_id",
                "pull_cursor",
                "last_attempt_at_utc_ms",
                "last_success_at_utc_ms",
                "last_error_code",
                "remote_user_fingerprint",
            ],
        )? || !table_columns_match(
            connection,
            "cloud_asset_transfers",
            &[
                "asset_id",
                "upload_url",
                "confirmed_offset",
                "expires_at_utc_ms",
                "updated_at_utc_ms",
            ],
        )? || !index_columns_match(
            connection,
            "sync_operations_lease_idx",
            &["status", "lease_expires_at_utc_ms"],
        )? {
            return Err(BackupError::Integrity);
        }
        let has_foreign_cloud_state: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM cloud_sync_state WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_cloud_state != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let legacy_tables = ["legacy_imports", "legacy_import_entities"];
    let legacy_table_count = legacy_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 10 && legacy_table_count != 0)
        || (schema_version >= 10 && legacy_table_count != legacy_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    if schema_version >= 10 {
        if !table_columns_match(
            connection,
            "legacy_imports",
            &[
                "id",
                "account_id",
                "source_fingerprint",
                "member_count",
                "problem_count",
                "asset_count",
                "review_count",
                "status",
                "created_at_utc_ms",
                "rolled_back_at_utc_ms",
            ],
        )? || !table_columns_match(
            connection,
            "legacy_import_entities",
            &["import_id", "entity_type", "entity_id", "created_by_import"],
        )? || !index_columns_match(
            connection,
            "legacy_import_entities_import_idx",
            &["import_id", "entity_type"],
        )? {
            return Err(BackupError::Integrity);
        }
        let has_foreign_import: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM legacy_imports WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_import != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    if schema_version >= 9
        && !index_columns_match(
            connection,
            "review_events_profile_time_idx",
            &["account_id", "profile_id", "occurred_at_utc_ms", "id"],
        )?
    {
        return Err(BackupError::Integrity);
    }
    let has_foreign_account: i64 = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM (
             SELECT account_id FROM learner_profiles
             UNION ALL SELECT account_id FROM problems
             UNION ALL SELECT account_id FROM assets
             UNION ALL SELECT account_id FROM review_events
             UNION ALL SELECT account_id FROM export_snapshots
             UNION ALL SELECT account_id FROM sync_operations
             UNION ALL SELECT account_id FROM sync_conflicts
             UNION ALL SELECT account_id FROM tombstones
           ) WHERE account_id <> ?1 LIMIT 1
         )",
        [account_id],
        |row| row.get(0),
    )?;
    if has_foreign_account != 0 {
        return Err(BackupError::ForeignAccountData);
    }
    let has_review_sessions = table_exists(connection, "review_sessions")?;
    if (schema_version == 1 && has_review_sessions) || (schema_version >= 2 && !has_review_sessions)
    {
        return Err(BackupError::Integrity);
    }
    if has_review_sessions {
        if schema_version >= 7 {
            for column in [
                "experience",
                "exam_phase",
                "exam_question_index",
                "exam_correct_count",
                "exam_wrong_count",
            ] {
                if !column_exists(connection, "review_sessions", column)? {
                    return Err(BackupError::Integrity);
                }
            }
        }
        if schema_version >= 8 {
            for column in [
                "focus_policy",
                "focus_round",
                "focus_order_json",
                "focus_next_number",
                "focus_elapsed_ms",
            ] {
                if !column_exists(connection, "review_sessions", column)? {
                    return Err(BackupError::Integrity);
                }
            }
        }
        let has_foreign_session: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM review_sessions WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_session != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }

    let capture_tables = [
        "capture_batches",
        "capture_drafts",
        "capture_items",
        "capture_draft_items",
    ];
    let capture_table_count = capture_tables
        .iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|exists| *exists)
        .count();
    if (schema_version < 3 && capture_table_count != 0)
        || (schema_version >= 3 && capture_table_count != capture_tables.len())
    {
        return Err(BackupError::Integrity);
    }
    if capture_table_count == capture_tables.len() {
        let has_foreign_capture_batch: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM capture_batches WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_capture_batch != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let has_profile_preferences = table_exists(connection, "profile_preferences")?;
    if (schema_version < 5 && has_profile_preferences)
        || (schema_version >= 5 && !has_profile_preferences)
    {
        return Err(BackupError::Integrity);
    }
    if has_profile_preferences {
        if schema_version >= 8
            && !column_exists(connection, "profile_preferences", "review_focus_policy")?
        {
            return Err(BackupError::Integrity);
        }
        if schema_version >= 18
            && (!column_definition_matches(
                connection,
                "profile_preferences",
                "daily_review_target",
                "INTEGER",
                "20",
            )? || !column_definition_matches(
                connection,
                "profile_preferences",
                "daily_minutes_target",
                "INTEGER",
                "20",
            )? || !table_sql_contains_compact(
                connection,
                "profile_preferences",
                "check(daily_review_targetbetween1and200)",
            )? || !table_sql_contains_compact(
                connection,
                "profile_preferences",
                "check(daily_minutes_targetbetween5and240)",
            )?)
        {
            return Err(BackupError::Integrity);
        }
        if schema_version >= 18 {
            let has_invalid_learning_goal: i64 = connection.query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM profile_preferences
                   WHERE daily_review_target IS NULL
                      OR typeof(daily_review_target) <> 'integer'
                      OR daily_review_target NOT BETWEEN 1 AND 200
                      OR daily_minutes_target IS NULL
                      OR typeof(daily_minutes_target) <> 'integer'
                      OR daily_minutes_target NOT BETWEEN 5 AND 240
                   LIMIT 1
                 )",
                [],
                |row| row.get(0),
            )?;
            if has_invalid_learning_goal != 0 {
                return Err(BackupError::Integrity);
            }
        }
        let has_foreign_preferences: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM profile_preferences WHERE account_id <> ?1 LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if has_foreign_preferences != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    let has_account_preferences = table_exists(connection, "account_preferences")?;
    if (schema_version < 6 && has_account_preferences)
        || (schema_version >= 6 && !has_account_preferences)
    {
        return Err(BackupError::Integrity);
    }
    if has_account_preferences {
        let invalid_account_preferences: i64 = connection.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM account_preferences ap
               LEFT JOIN learner_profiles p ON p.id = ap.active_profile_id
               WHERE ap.account_id <> ?1
                  OR p.id IS NULL
                  OR p.account_id <> ap.account_id
               LIMIT 1
             )",
            [account_id],
            |row| row.get(0),
        )?;
        if invalid_account_preferences != 0 {
            return Err(BackupError::ForeignAccountData);
        }
    }
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, BackupError> {
    let exists: i64 = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
         )",
        [table],
        |row| row.get(0),
    )?;
    Ok(exists != 0)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool, BackupError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info('{table}')"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns.iter().any(|candidate| candidate == column))
}

fn column_definition_matches(
    connection: &Connection,
    table: &str,
    column: &str,
    expected_type: &str,
    expected_default: &str,
) -> Result<bool, BackupError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info('{table}')"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name != column {
            continue;
        }
        let column_type: String = row.get(2)?;
        let not_null: i64 = row.get(3)?;
        let default_value: Option<String> = row.get(4)?;
        return Ok(column_type.eq_ignore_ascii_case(expected_type)
            && not_null == 1
            && default_value.as_deref() == Some(expected_default));
    }
    Ok(false)
}

fn table_sql_contains_compact(
    connection: &Connection,
    table: &str,
    expected: &str,
) -> Result<bool, BackupError> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()?;
    let compact = sql
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    Ok(compact.contains(&expected.to_ascii_lowercase()))
}

fn table_columns_match(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> Result<bool, BackupError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info('{table}')"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied()))
}
fn index_columns_match(
    connection: &Connection,
    index: &str,
    expected: &[&str],
) -> Result<bool, BackupError> {
    let mut statement = connection.prepare(&format!("PRAGMA index_info('{index}')"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(2))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied()))
}
