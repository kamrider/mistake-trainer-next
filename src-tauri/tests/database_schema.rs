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
        "review_sessions",
        "capture_batches",
        "capture_drafts",
        "capture_items",
        "capture_draft_items",
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
        4
    );
}

#[test]
fn version_two_library_upgrades_without_changing_existing_problem_data() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "capture-upgrade-key").expect("open encrypted database");

    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .expect("initial schema");
    connection
        .execute_batch(include_str!("../migrations/0002_review_sessions.sql"))
        .expect("review schema");
    connection.pragma_update(None, "user_version", 2).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision) VALUES('profile', 'account', 'existing', 1, 1, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, status, created_at_utc_ms, updated_at_utc_ms, revision) VALUES('problem', 'account', 'profile', '数学', '保留', 'active', 2, 2, 1)",
        [],
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade to capture inbox");

    let problem: (String, String) = connection
        .query_row(
            "SELECT subject, note FROM problems WHERE id = 'problem'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(problem, ("数学".to_owned(), "保留".to_owned()));
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        4
    );

    let staged_role: String = connection
        .query_row(
            "SELECT dflt_value FROM pragma_table_info('capture_items') WHERE name = 'staged_role'",
            [],
            |row| row.get(0),
        )
        .expect("staged role column");
    assert_eq!(staged_role, "'question'");
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
