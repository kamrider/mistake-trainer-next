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
        9
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
        9
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
        9
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
        9
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
    connection.execute(
        "INSERT INTO profile_preferences(
             account_id, profile_id, enabled_subjects_json, custom_subjects_json,
             capture_sound_enabled, updated_at_utc_ms
         ) VALUES('account', 'profile', '[\"数学\"]', '[\"编程\"]', 0, 2)",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO review_sessions(
             id, account_id, profile_id, mode, problem_ids_json, current_index,
             status, created_at_utc_ms, updated_at_utc_ms, experience, exam_phase,
             exam_question_index, exam_correct_count, exam_wrong_count
         ) VALUES('session', 'account', 'profile', 'manual', '[\"problem-a\",\"problem-b\"]',
                  1, 'active', 3, 4, 'review', NULL, 0, 0, 0)",
        [],
    ).unwrap();

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
        9
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
    connection.execute(
        "INSERT INTO review_events(
             id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
             occurred_at_utc_ms, algorithm_version, parameter_version
         ) VALUES('event', 'account', 'profile', 'problem', 'device', 'good', 1234,
                  10, 'fsrs-6.6.1', 'default-6.6.1')",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO schedule_states(
             problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms,
             algorithm_version, parameter_version, rebuilt_at_utc_ms
         ) VALUES('problem', 20, 2.5, 4.0, 10, 'fsrs-6.6.1', 'default-6.6.1', 11)",
        [],
    ).unwrap();

    let before: (String, String, i64, i64, f64, f64) = connection.query_row(
        "SELECT e.rating, e.algorithm_version, e.duration_ms, s.due_at_utc_ms,
                s.stability, s.difficulty
         FROM review_events e JOIN schedule_states s ON s.problem_id = e.problem_id
         WHERE e.id = 'event'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).unwrap();

    run_migrations(&mut connection).expect("upgrade schema v8 to v9");

    let after: (String, String, i64, i64, f64, f64) = connection.query_row(
        "SELECT e.rating, e.algorithm_version, e.duration_ms, s.due_at_utc_ms,
                s.stability, s.difficulty
         FROM review_events e JOIN schedule_states s ON s.problem_id = e.problem_id
         WHERE e.id = 'event'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).unwrap();
    assert_eq!(after, before);
    let index_columns = connection
        .prepare("SELECT name FROM pragma_index_info('review_events_profile_time_idx') ORDER BY seqno")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(index_columns, ["account_id", "profile_id", "occurred_at_utc_ms", "id"]);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        9
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
