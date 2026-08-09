use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database key must not be empty")]
    EmptyKey,
    #[error("encrypted database error")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database schema version {0} is newer than this application supports")]
    UnsupportedSchema(i64),
}

pub fn open_encrypted_database(path: &Path, key: &str) -> Result<Connection, DatabaseError> {
    if key.trim().is_empty() {
        return Err(DatabaseError::EmptyKey);
    }

    let connection = Connection::open(path)?;
    connection.pragma_update(None, "key", key)?;
    connection.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    connection.pragma_update(None, "cipher_memory_security", "ON")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;

    Ok(connection)
}

pub fn open_encrypted_database_read_only(
    path: &Path,
    key: &str,
) -> Result<Connection, DatabaseError> {
    if key.trim().is_empty() {
        return Err(DatabaseError::EmptyKey);
    }

    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.pragma_update(None, "key", key)?;
    connection.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    connection.pragma_update(None, "cipher_memory_security", "ON")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;

    Ok(connection)
}

fn run_migrations_to_v11(connection: &mut Connection) -> Result<(), DatabaseError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0001_initial.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0002_review_sessions.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0003_capture_inbox.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0004_capture_staged_roles.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0005_profile_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        1 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0002_review_sessions.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0003_capture_inbox.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0004_capture_staged_roles.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0005_profile_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        2 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0003_capture_inbox.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0004_capture_staged_roles.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0005_profile_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        3 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!(
                "../../migrations/0004_capture_staged_roles.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0005_profile_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        4 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!(
                "../../migrations/0005_profile_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        5 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!(
                "../../migrations/0006_account_preferences.sql"
            ))?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        6 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0007_review_exam.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        7 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0008_review_focus.sql"))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        8 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!(
                "../../migrations/0009_review_history_index.sql"
            ))?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        9 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!(
                "../../migrations/0010_legacy_import_ledger.sql"
            ))?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        10 => {
            let transaction = connection.transaction()?;
            transaction
                .execute_batch(include_str!("../../migrations/0011_cloud_sync_state.sql"))?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
            Ok(())
        }
        11 => Ok(()),
        newer => Err(DatabaseError::UnsupportedSchema(newer)),
    }
}

pub fn run_migrations(connection: &mut Connection) -> Result<(), DatabaseError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > 17 {
        return Err(DatabaseError::UnsupportedSchema(version));
    }
    if version < 11 {
        run_migrations_to_v11(connection)?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 11 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../../migrations/0012_asset_derivations.sql"))?;
        transaction.pragma_update(None, "user_version", 12)?;
        transaction.commit()?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 12 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../../migrations/0013_sync_merge_state.sql"))?;
        transaction.pragma_update(None, "user_version", 13)?;
        transaction.commit()?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 13 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!(
            "../../migrations/0014_capture_recognition_jobs.sql"
        ))?;
        transaction.pragma_update(None, "user_version", 14)?;
        transaction.commit()?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 14 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!(
            "../../migrations/0015_expand_asset_derivation_positions.sql"
        ))?;
        transaction.pragma_update(None, "user_version", 15)?;
        transaction.commit()?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 15 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!(
            "../../migrations/0016_capture_recognition_pairs.sql"
        ))?;
        transaction.pragma_update(None, "user_version", 16)?;
        transaction.commit()?;
    }
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 16 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!(
            "../../migrations/0017_capture_recognition_pair_state.sql"
        ))?;
        transaction.pragma_update(None, "user_version", 17)?;
        transaction.commit()?;
    }
    Ok(())
}
