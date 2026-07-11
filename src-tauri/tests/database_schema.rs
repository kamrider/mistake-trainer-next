use mistake_trainer_next_lib::infrastructure::database::{open_encrypted_database, run_migrations};
use tempfile::tempdir;

#[test]
fn initial_migration_creates_the_offline_first_core_schema() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "migration-test-key").expect("open encrypted database");

    run_migrations(&mut connection).expect("run migrations");

    let expected = [
        "learner_profiles",
        "problems",
        "assets",
        "problem_assets",
        "review_events",
        "schedule_states",
        "export_snapshots",
        "sync_operations",
        "sync_conflicts",
        "tombstones",
    ];
    for table in expected {
        let found: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("query schema");
        assert_eq!(found, 1, "missing table {table}");
    }
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn plaintext_asset_hash_is_unique_within_an_account() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "dedupe-test-key").expect("open encrypted database");
    run_migrations(&mut connection).expect("run migrations");

    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ("a1", "account-1", "same-hash", "01/a1.blob", 10_i64, "image/png", 1_i64),
    ).expect("first asset");

    let duplicate = connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ("a2", "account-1", "same-hash", "01/a2.blob", 10_i64, "image/png", 2_i64),
    );
    assert!(duplicate.is_err());

    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ("a3", "account-2", "same-hash", "02/a3.blob", 10_i64, "image/png", 3_i64),
    ).expect("same hash in another account is isolated");
}
