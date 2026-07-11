use mistake_trainer_next_lib::infrastructure::database::{DatabaseError, open_encrypted_database};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn linked_sqlite_reports_sqlcipher_support() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let connection = open_encrypted_database(&path, "correct horse battery staple")
        .expect("open encrypted database");

    let version: String = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .expect("SQLCipher version");

    assert!(!version.trim().is_empty());
}

#[test]
fn encrypted_database_cannot_be_read_without_its_key() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    {
        let connection =
            open_encrypted_database(&path, "device-secret").expect("open encrypted database");
        connection
            .execute_batch("CREATE TABLE probe(value TEXT); INSERT INTO probe VALUES ('private');")
            .expect("write encrypted content");
    }

    let plain = Connection::open(&path).expect("open file without a key");
    assert!(
        plain
            .query_row("SELECT value FROM probe", [], |row| row.get::<_, String>(0))
            .is_err()
    );

    let encrypted =
        open_encrypted_database(&path, "device-secret").expect("reopen with correct key");
    let value: String = encrypted
        .query_row("SELECT value FROM probe", [], |row| row.get(0))
        .expect("read encrypted content");
    assert_eq!(value, "private");
}

#[test]
fn empty_database_key_is_rejected_before_opening_a_file() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");

    let error = open_encrypted_database(&path, "   ").expect_err("empty key must fail");
    assert!(matches!(error, DatabaseError::EmptyKey));
    assert!(!path.exists());
}
