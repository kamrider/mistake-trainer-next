use std::path::Path;

use rusqlite::Connection;
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

pub fn run_migrations(connection: &mut Connection) -> Result<(), DatabaseError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0001_initial.sql"))?;
            transaction.execute_batch(include_str!("../../migrations/0002_review_sessions.sql"))?;
            transaction.pragma_update(None, "user_version", 2)?;
            transaction.commit()?;
            Ok(())
        }
        1 => {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../../migrations/0002_review_sessions.sql"))?;
            transaction.pragma_update(None, "user_version", 2)?;
            transaction.commit()?;
            Ok(())
        }
        2 => Ok(()),
        newer => Err(DatabaseError::UnsupportedSchema(newer)),
    }
}
