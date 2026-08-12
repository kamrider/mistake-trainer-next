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
        "profile_preferences",
        "account_preferences",
        "legacy_imports",
        "legacy_import_entities",
        "asset_derivations",
        "capture_source_retention",
        "sync_entity_snapshots",
        "capture_recognition_jobs",
        "capture_recognition_job_items",
        "capture_recognition_suggestions",
        "capture_recognition_operations",
        "capture_recognition_pairs",
        "capture_recognition_pair_items",
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
        18
    );
}

#[test]
fn version_nine_library_adds_reversible_legacy_import_ledger_without_changing_rows() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "legacy-ledger-upgrade-key").expect("open database");

    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 9).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', 'existing', 1, 1, 1)",
        [],
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade schema v9 to v10");

    assert_eq!(
        connection
            .query_row(
                "SELECT name FROM learner_profiles WHERE id = 'profile'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "existing"
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    let import_columns = connection
        .prepare("SELECT name FROM pragma_table_info('legacy_imports') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        import_columns,
        [
            "id",
            "account_id",
            "source_fingerprint",
            "member_count",
            "problem_count",
            "asset_count",
            "review_count",
            "status",
            "created_at_utc_ms",
            "rolled_back_at_utc_ms"
        ]
    );
    let entity_columns = connection
        .prepare("SELECT name FROM pragma_table_info('legacy_import_entities') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        entity_columns,
        ["import_id", "entity_type", "entity_id", "created_by_import"]
    );
    let index_columns = connection
        .prepare(
            "SELECT name FROM pragma_index_info('legacy_import_entities_import_idx') ORDER BY seqno",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(index_columns, ["import_id", "entity_type"]);
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
        18
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
fn version_five_library_adds_active_profile_preferences_without_changing_existing_rows() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "profile-selection-upgrade-key").expect("open database");

    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0002_review_sessions.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0003_capture_inbox.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0004_capture_staged_roles.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0005_profile_preferences.sql"))
        .unwrap();
    connection.pragma_update(None, "user_version", 5).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', '小树', 1, 1, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO profile_preferences(account_id, profile_id, enabled_subjects_json, custom_subjects_json, capture_sound_enabled, updated_at_utc_ms)
         VALUES('account', 'profile', '[\"数学\"]', '[]', 1, 2)",
        [],
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade schema v5 to v7");

    let profile: (String, String) = connection
        .query_row(
            "SELECT p.name, pref.enabled_subjects_json
         FROM learner_profiles p JOIN profile_preferences pref ON pref.profile_id = p.id
         WHERE p.id = 'profile'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(profile, ("小树".to_owned(), "[\"数学\"]".to_owned()));
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
}

#[test]
fn version_six_library_adds_exam_state_without_changing_existing_session_progress() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection = open_encrypted_database(&path, "exam-upgrade-key").expect("open database");

    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0002_review_sessions.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0003_capture_inbox.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0004_capture_staged_roles.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0005_profile_preferences.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0006_account_preferences.sql"))
        .unwrap();
    connection.pragma_update(None, "user_version", 6).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', '小树', 1, 1, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO review_sessions(
             id, account_id, profile_id, mode, problem_ids_json, current_index,
             status, created_at_utc_ms, updated_at_utc_ms
         ) VALUES('session', 'account', 'profile', 'manual', '[\"problem-a\",\"problem-b\"]', 1, 'active', 2, 3)",
        [],
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade schema v6 to v7");

    let session: (String, String, i64, Option<String>, i64, i64, i64) = connection
        .query_row(
            "SELECT mode, experience, current_index, exam_phase, exam_question_index,
                    exam_correct_count, exam_wrong_count
             FROM review_sessions WHERE id = 'session'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        session,
        ("manual".to_owned(), "review".to_owned(), 1, None, 0, 0, 0)
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );

    let invalid_phase = connection.execute(
        "UPDATE review_sessions SET experience = 'exam', exam_phase = 'finished' WHERE id = 'session'",
        [],
    );
    assert!(invalid_phase.is_err());
    let invalid_position = connection.execute(
        "UPDATE review_sessions SET exam_question_index = -1 WHERE id = 'session'",
        [],
    );
    assert!(invalid_position.is_err());
}

#[test]
fn version_seven_library_adds_focus_state_without_changing_existing_preferences_or_session() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "focus-upgrade-key").expect("open database");

    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0002_review_sessions.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0003_capture_inbox.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0004_capture_staged_roles.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0005_profile_preferences.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0006_account_preferences.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0007_review_exam.sql"))
        .unwrap();
    connection.pragma_update(None, "user_version", 7).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', '小树', 1, 1, 1)",
        [],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO profile_preferences(
             account_id, profile_id, enabled_subjects_json, custom_subjects_json,
             capture_sound_enabled, updated_at_utc_ms
         ) VALUES('account', 'profile', '[\"数学\"]', '[\"编程\"]', 0, 2)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_sessions(
             id, account_id, profile_id, mode, problem_ids_json, current_index,
             status, created_at_utc_ms, updated_at_utc_ms, experience, exam_phase,
             exam_question_index, exam_correct_count, exam_wrong_count
         ) VALUES('session', 'account', 'profile', 'manual', '[\"problem-a\",\"problem-b\"]',
                  1, 'active', 3, 4, 'review', NULL, 0, 0, 0)",
            [],
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v7 to v8");

    let preference: (String, String, i64, String) = connection
        .query_row(
            "SELECT enabled_subjects_json, custom_subjects_json, capture_sound_enabled,
                    review_focus_policy
             FROM profile_preferences WHERE profile_id = 'profile'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        preference,
        (
            "[\"数学\"]".to_owned(),
            "[\"编程\"]".to_owned(),
            0,
            "off".to_owned(),
        )
    );
    let session: (String, String, i64, String, i64, Option<String>, i64, i64) = connection
        .query_row(
            "SELECT mode, experience, current_index, focus_policy, focus_round,
                    focus_order_json, focus_next_number, focus_elapsed_ms
             FROM review_sessions WHERE id = 'session'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        session,
        (
            "manual".to_owned(),
            "review".to_owned(),
            1,
            "off".to_owned(),
            0,
            None,
            0,
            0,
        )
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );

    assert!(connection.execute(
        "UPDATE profile_preferences SET review_focus_policy = 'sometimes' WHERE profile_id = 'profile'",
        [],
    ).is_err());
    assert!(connection.execute(
        "UPDATE review_sessions SET focus_order_json = '[1,2]', focus_next_number = 0 WHERE id = 'session'",
        [],
    ).is_err());
    assert!(connection.execute(
        "UPDATE review_sessions SET focus_order_json = NULL, focus_next_number = 1 WHERE id = 'session'",
        [],
    ).is_err());
}

#[test]
fn version_eight_library_adds_review_history_index_without_changing_existing_rows() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "history-index-upgrade-key").expect("open database");

    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 8).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', '小树', 1, 1, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, status, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('problem', 'account', 'profile', '数学', '保留历史', 'active', 2, 2, 1)",
        [],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO review_events(
             id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
             occurred_at_utc_ms, algorithm_version, parameter_version
         ) VALUES('event', 'account', 'profile', 'problem', 'device', 'good', 1234,
                  10, 'fsrs-6.6.1', 'default-6.6.1')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO schedule_states(
             problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms,
             algorithm_version, parameter_version, rebuilt_at_utc_ms
         ) VALUES('problem', 20, 2.5, 4.0, 10, 'fsrs-6.6.1', 'default-6.6.1', 11)",
            [],
        )
        .unwrap();

    let before: (String, String, i64, i64, f64, f64) = connection
        .query_row(
            "SELECT e.rating, e.algorithm_version, e.duration_ms, s.due_at_utc_ms,
                s.stability, s.difficulty
         FROM review_events e JOIN schedule_states s ON s.problem_id = e.problem_id
         WHERE e.id = 'event'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v8 to v9");

    let after: (String, String, i64, i64, f64, f64) = connection
        .query_row(
            "SELECT e.rating, e.algorithm_version, e.duration_ms, s.due_at_utc_ms,
                s.stability, s.difficulty
         FROM review_events e JOIN schedule_states s ON s.problem_id = e.problem_id
         WHERE e.id = 'event'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(after, before);
    let index_columns = connection
        .prepare(
            "SELECT name FROM pragma_index_info('review_events_profile_time_idx') ORDER BY seqno",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        index_columns,
        ["account_id", "profile_id", "occurred_at_utc_ms", "id"]
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
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

#[test]
fn version_ten_library_adds_cloud_sync_state_without_changing_existing_data() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "cloud-sync-upgrade-key").expect("open database");

    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 10).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('profile', 'account', 'existing', 1, 2, 3)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, status, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('problem', 'account', 'profile', 'math', 'preserve me', 'active', 3, 4, 5)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES('asset', 'account', 'known-sha256', 'aa/asset.blob', 123, 'image/png', 6)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation,
              payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
         VALUES('operation', 'account', 'profile', 'problem', 'problem', 'upsert',
              '{\"stale\":true}', 'processing', 2, 7, 8)",
        [],
    ).unwrap();

    let before: (String, String, i64, String, i64) = connection
        .query_row(
            "SELECT p.note, a.plaintext_sha256, a.byte_length, s.payload_json, s.attempt_count
         FROM problems p
         JOIN assets a ON a.account_id = p.account_id
         JOIN sync_operations s ON s.entity_id = p.id
         WHERE p.id = 'problem'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v10 to v11");

    let after: (String, String, i64, String, i64) = connection
        .query_row(
            "SELECT p.note, a.plaintext_sha256, a.byte_length, s.payload_json, s.attempt_count
         FROM problems p
         JOIN assets a ON a.account_id = p.account_id
         JOIN sync_operations s ON s.entity_id = p.id
         WHERE p.id = 'problem'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM cloud_sync_state WHERE account_id = 'account' AND pull_cursor = 0",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap(),
        1
    );
    let outbox_columns = connection
        .prepare("SELECT name FROM pragma_table_info('sync_operations') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for required in ["lease_id", "lease_expires_at_utc_ms", "last_error_code"] {
        assert!(outbox_columns.iter().any(|column| column == required));
    }
}

#[test]
fn version_eleven_library_adds_non_destructive_crop_ledger_without_changing_capture_items() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "crop-ledger-upgrade-key").expect("open database");
    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 11).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
         VALUES('profile', 'account', 'existing', 1, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES('asset', 'account', 'hash', 'aa/asset.mtb', 10, 'image/png', 2)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO capture_batches(id, account_id, profile_id, subject, state, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('batch', 'account', 'profile', 'math', 'organizing', 3, 3, 1)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO capture_items(id, batch_id, asset_id, client_upload_id, source_name, source_sequence, width, height, created_at_utc_ms, staged_role)
         VALUES('item', 'batch', 'asset', 'upload', 'photo.png', 0, 20, 30, 4, 'answer')",
        [],
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade schema v11 to v12");

    let item: (String, i64, i64, String, Option<String>) = connection
        .query_row(
            "SELECT source_name, width, height, staged_role, superseded_by_derivation_id
         FROM capture_items WHERE id = 'item'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        item,
        ("photo.png".to_owned(), 20, 30, "answer".to_owned(), None)
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
}

#[test]
fn version_twelve_library_adds_sync_merge_state_without_changing_open_conflicts() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "sync-merge-upgrade-key").expect("open database");
    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
        include_str!("../migrations/0012_asset_derivations.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 12).unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               'conflict', 'account', 'profile', 'problem', 'problem', 'note',
               '\"local\"', '\"remote\"', 1, 2
             )",
            [],
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v12 to the latest version");

    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_conflicts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_entity_snapshots", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );

    let conflict_columns = connection
        .prepare("SELECT name FROM pragma_table_info('sync_conflicts') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        conflict_columns,
        [
            "id",
            "account_id",
            "profile_id",
            "entity_type",
            "entity_id",
            "field_name",
            "local_value_json",
            "remote_value_json",
            "base_revision",
            "created_at_utc_ms",
            "resolved_at_utc_ms",
            "resolution",
            "resolved_value_json",
        ]
    );

    let duplicate_open = connection.execute(
        "INSERT INTO sync_conflicts(
           id, account_id, profile_id, entity_type, entity_id, field_name,
           local_value_json, remote_value_json, base_revision, created_at_utc_ms
         ) VALUES(
           'duplicate', 'account', 'profile', 'problem', 'problem', 'note',
           '\"new local\"', '\"new remote\"', 2, 3
         )",
        [],
    );
    assert!(duplicate_open.is_err());

    connection
        .execute(
            "UPDATE sync_conflicts
             SET resolution = 'local', resolved_value_json = local_value_json,
                 resolved_at_utc_ms = 4
             WHERE id = 'conflict'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               'next', 'account', 'profile', 'problem', 'problem', 'note',
               '\"later local\"', '\"later remote\"', 3, 5
             )",
            [],
        )
        .expect("a later conflict is allowed after audit resolution");
}

#[test]
fn version_thirteen_library_adds_recognition_jobs_without_changing_capture_rows() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "recognition-upgrade-key").expect("open database");
    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
        include_str!("../migrations/0012_asset_derivations.sql"),
        include_str!("../migrations/0013_sync_merge_state.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 13).unwrap();
    connection
        .execute(
            "INSERT INTO learner_profiles(
           id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
         ) VALUES('profile', 'account', 'existing', 1, 2, 3)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO assets(
           id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
           created_at_utc_ms
         ) VALUES('asset', 'account', 'hash', 'blobs/aa/asset.mtb', 10, 'image/png', 4)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO capture_batches(
           id, account_id, profile_id, subject, state, created_at_utc_ms,
           updated_at_utc_ms, revision
         ) VALUES('batch', 'account', 'profile', '数学', 'organizing', 5, 6, 7)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO capture_items(
           id, batch_id, asset_id, client_upload_id, source_name, source_sequence,
           width, height, created_at_utc_ms, staged_role
         ) VALUES(
           'item', 'batch', 'asset', 'upload', 'question.png', 0, 1200, 900, 8,
           'question'
         )",
            [],
        )
        .unwrap();
    let before = connection
        .query_row(
            "SELECT b.subject, b.state, b.revision, i.source_name, i.staged_role,
                i.superseded_by_derivation_id, a.plaintext_sha256, a.encrypted_path
         FROM capture_batches b
         JOIN capture_items i ON i.batch_id = b.id
         JOIN assets a ON a.id = i.asset_id
         WHERE b.id = 'batch'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v13 to current");

    let after = connection
        .query_row(
            "SELECT b.subject, b.state, b.revision, i.source_name, i.staged_role,
                i.superseded_by_derivation_id, a.plaintext_sha256, a.encrypted_path
         FROM capture_batches b
         JOIN capture_items i ON i.batch_id = b.id
         JOIN assets a ON a.id = i.asset_id
         WHERE b.id = 'batch'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    for table in [
        "capture_recognition_jobs",
        "capture_recognition_job_items",
        "capture_recognition_suggestions",
        "capture_recognition_operations",
    ] {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1,
            "missing table {table}"
        );
    }
}

#[test]
fn version_fourteen_library_preserves_derivations_and_expands_the_position_budget() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "derivation-position-upgrade-key").expect("open database");
    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
        include_str!("../migrations/0012_asset_derivations.sql"),
        include_str!("../migrations/0013_sync_merge_state.sql"),
        include_str!("../migrations/0014_capture_recognition_jobs.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 14).unwrap();
    connection
        .execute_batch(
            "INSERT INTO learner_profiles(
                 id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('profile', 'account', 'existing', 1, 2, 3);
             INSERT INTO assets(
                 id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
                 created_at_utc_ms
             ) VALUES
                 ('source-asset', 'account', 'source-hash', 'blobs/source.mtb', 10, 'image/png', 4),
                 ('derived-asset', 'account', 'derived-hash', 'blobs/derived.mtb', 9, 'image/png', 5);
             INSERT INTO capture_batches(
                 id, account_id, profile_id, subject, state, created_at_utc_ms,
                 updated_at_utc_ms, revision
             ) VALUES('batch', 'account', 'profile', '数学', 'organizing', 5, 6, 7);
             INSERT INTO asset_derivations(
                 id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                 source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                 engine, engine_version, confidence, created_at_utc_ms
             ) VALUES(
                 'derivation-9', 'operation', 'account', 'batch', 'source-asset', 'derived-asset',
                 'source-item', 'derived-item-9', 9, 'crop', '{}',
                 'fixture', '1', 0.8, 8
             );",
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v14 to current");

    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT position FROM asset_derivations WHERE id = 'derivation-9'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        9
    );
    connection
        .execute(
            "INSERT INTO asset_derivations(
                 id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                 source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                 engine, engine_version, confidence, created_at_utc_ms
             ) VALUES(
                 'derivation-149', 'operation', 'account', 'batch', 'source-asset', 'derived-asset',
                 'source-item', 'derived-item-149', 149, 'crop', '{}',
                 'fixture', '1', 0.8, 9
             )",
            [],
        )
        .expect("position 149 is within the capture batch budget");
    assert!(
        connection
            .execute(
                "INSERT INTO asset_derivations(
                     id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
                     source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
                     engine, engine_version, confidence, created_at_utc_ms
                 ) VALUES(
                     'derivation-150', 'operation', 'account', 'batch', 'source-asset',
                     'derived-asset', 'source-item', 'derived-item-150', 150, 'crop', '{}',
                     'fixture', '1', 0.8, 10
                 )",
                [],
            )
            .is_err(),
        "position 150 must remain outside the batch capacity"
    );
}

#[test]
fn version_fifteen_library_adds_empty_pair_suggestions_without_changing_existing_rows() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "recognition-pair-upgrade-key").expect("open database");
    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
        include_str!("../migrations/0012_asset_derivations.sql"),
        include_str!("../migrations/0013_sync_merge_state.sql"),
        include_str!("../migrations/0014_capture_recognition_jobs.sql"),
        include_str!("../migrations/0015_expand_asset_derivation_positions.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 15).unwrap();
    connection
        .execute_batch(
            "INSERT INTO learner_profiles(
                 id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('profile', 'account', 'existing', 1, 2, 3);
             INSERT INTO assets(
                 id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type,
                 created_at_utc_ms
             ) VALUES('asset', 'account', 'hash', 'blobs/asset.mtb', 10, 'image/png', 4);
             INSERT INTO capture_batches(
                 id, account_id, profile_id, subject, state, created_at_utc_ms,
                 updated_at_utc_ms, revision
             ) VALUES('batch', 'account', 'profile', '数学', 'organizing', 5, 6, 7);
             INSERT INTO capture_items(
                 id, batch_id, asset_id, client_upload_id, source_name, source_sequence,
                 width, height, created_at_utc_ms, staged_role
             ) VALUES(
                 'item', 'batch', 'asset', 'upload', 'question.png', 0, 1200, 900, 8,
                 'question'
             );",
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v15 to v18");

    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT b.subject, b.revision, i.source_name, i.staged_role
                 FROM capture_batches b
                 JOIN capture_items i ON i.batch_id = b.id
                 WHERE b.id = 'batch'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap(),
        (
            "数学".to_owned(),
            7,
            "question.png".to_owned(),
            "question".to_owned(),
        )
    );
    for table in [
        "capture_recognition_pairs",
        "capture_recognition_pair_items",
    ] {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1,
            "missing table {table}"
        );
    }
}

#[test]
fn version_eighteen_adds_bounded_learning_goal_defaults() {
    let directory = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "key").unwrap();
    run_migrations(&mut connection).unwrap();
    connection
        .execute(
            "INSERT INTO learner_profiles(
                 id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('profile', 'account', '学习者', 1, 1, 1)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO profile_preferences(
                 account_id, profile_id, enabled_subjects_json, custom_subjects_json,
                 capture_sound_enabled, updated_at_utc_ms
             ) VALUES('account', 'profile', '[\"数学\"]', '[]', 1, 2)",
            [],
        )
        .unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT daily_review_target, daily_minutes_target
                 FROM profile_preferences WHERE profile_id = 'profile'",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        (20, 20)
    );
    assert!(
        connection
            .execute(
                "UPDATE profile_preferences SET daily_review_target = 0
                 WHERE profile_id = 'profile'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE profile_preferences SET daily_minutes_target = 241
                 WHERE profile_id = 'profile'",
                [],
            )
            .is_err()
    );
}

#[test]
fn version_sixteen_library_adds_durable_pair_state_without_changing_existing_pairs() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "pair-state-upgrade-key").expect("open database");

    for migration in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_review_sessions.sql"),
        include_str!("../migrations/0003_capture_inbox.sql"),
        include_str!("../migrations/0004_capture_staged_roles.sql"),
        include_str!("../migrations/0005_profile_preferences.sql"),
        include_str!("../migrations/0006_account_preferences.sql"),
        include_str!("../migrations/0007_review_exam.sql"),
        include_str!("../migrations/0008_review_focus.sql"),
        include_str!("../migrations/0009_review_history_index.sql"),
        include_str!("../migrations/0010_legacy_import_ledger.sql"),
        include_str!("../migrations/0011_cloud_sync_state.sql"),
        include_str!("../migrations/0012_asset_derivations.sql"),
        include_str!("../migrations/0013_sync_merge_state.sql"),
        include_str!("../migrations/0014_capture_recognition_jobs.sql"),
        include_str!("../migrations/0015_expand_asset_derivation_positions.sql"),
        include_str!("../migrations/0016_capture_recognition_pairs.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 16).unwrap();
    connection
        .execute_batch(
            "INSERT INTO learner_profiles(
               id, account_id, name, created_at_utc_ms, updated_at_utc_ms
             ) VALUES('profile', 'account', 'existing', 1, 1);
             INSERT INTO capture_batches(
               id, account_id, profile_id, subject, state,
               created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('batch', 'account', 'profile', '', 'organizing', 1, 1, 1);
             INSERT INTO capture_recognition_jobs(
               id, account_id, profile_id, batch_id, state, engine, engine_version,
               model_component_id, total_items, processed_items,
               created_at_utc_ms, updated_at_utc_ms
             ) VALUES(
               'job', 'account', 'profile', 'batch', 'applied',
               'fixture', '1', 'ppocrv6_small', 1, 1, 1, 1
             );
             INSERT INTO capture_recognition_operations(
               id, job_id, batch_id, before_revision, after_revision,
               created_entity_ids_json, created_at_utc_ms
             ) VALUES('operation', 'job', 'batch', 1, 2, '{}', 2);
             INSERT INTO capture_recognition_pairs(
               id, operation_id, pair_slot, confidence_basis_points, created_at_utc_ms
             ) VALUES('pair', 'operation', 0, 9000, 3);",
        )
        .unwrap();

    run_migrations(&mut connection).expect("upgrade schema v16 to v18");

    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT state, resolved_at_utc_ms, created_at_utc_ms
                 FROM capture_recognition_pairs WHERE id = 'pair'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap(),
        ("active".to_owned(), None, 3)
    );
}
