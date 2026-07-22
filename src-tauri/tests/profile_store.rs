use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::profiles::{
        CreateProfile, DeleteProfile, ProfileUseCaseError, RenameProfile, create_profile,
        delete_profile, list_profiles, persist_active_profile, rename_profile,
    },
};
use rusqlite::params;
use tempfile::tempdir;

#[test]
fn creating_a_profile_commits_the_profile_and_outbox_together() {
    let directory = tempdir().expect("temp directory");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "profile-key")
            .expect("open encrypted database");
    run_migrations(&mut connection).expect("migrate database");

    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "  小树  ".to_owned(),
            now_utc_ms: 1_700_000_000_000,
        },
    )
    .expect("create profile");

    assert_eq!(profile.name, "小树");
    let profile_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM learner_profiles WHERE id = ?1",
            [&profile.id],
            |row| row.get(0),
        )
        .unwrap();
    let operation: (String, String, String) = connection
        .query_row(
            "SELECT entity_type, entity_id, operation FROM sync_operations WHERE entity_id = ?1",
            [&profile.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(profile_count, 1);
    assert_eq!(
        operation,
        (
            "learner_profile".to_owned(),
            profile.id,
            "upsert".to_owned()
        )
    );
}

#[test]
fn profile_management_is_ordered_scoped_and_persists_the_active_profile() {
    let directory = tempdir().expect("temp directory");
    let mut connection = open_encrypted_database(
        &directory.path().join("library.db"),
        "profile-management-key",
    )
    .unwrap();
    run_migrations(&mut connection).unwrap();
    let first = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let second = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "竞赛档案".to_owned(),
            now_utc_ms: 20,
        },
    )
    .unwrap();
    create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "其他账户".to_owned(),
            now_utc_ms: 5,
        },
    )
    .unwrap();

    let profiles = list_profiles(&connection, "account-1").unwrap();
    assert_eq!(
        profiles
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        vec![first.id.as_str(), second.id.as_str()]
    );

    let renamed = rename_profile(
        &mut connection,
        RenameProfile {
            account_id: "account-1".to_owned(),
            profile_id: second.id.clone(),
            name: "  物理竞赛  ".to_owned(),
            now_utc_ms: 30,
        },
    )
    .unwrap();
    assert_eq!(renamed.name, "物理竞赛");
    assert_eq!(renamed.revision, 2);
    let rename_outbox: (String, String, String) = connection
        .query_row(
            "SELECT entity_type, operation, payload_json FROM sync_operations
         WHERE entity_id = ?1 ORDER BY created_at_utc_ms DESC LIMIT 1",
            [&second.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (rename_outbox.0.as_str(), rename_outbox.1.as_str()),
        ("learner_profile", "upsert")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rename_outbox.2).unwrap()["name"],
        "物理竞赛"
    );

    let selected = persist_active_profile(&mut connection, "account-1", &second.id, 40).unwrap();
    assert_eq!(selected.name, "物理竞赛");
    let stored_active: String = connection
        .query_row(
            "SELECT active_profile_id FROM account_preferences WHERE account_id = 'account-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_active, second.id);
}

#[test]
fn duplicate_names_and_forged_profile_ids_leave_state_unchanged() {
    let directory = tempdir().expect("temp directory");
    let mut connection = open_encrypted_database(
        &directory.path().join("library.db"),
        "profile-rejection-key",
    )
    .unwrap();
    run_migrations(&mut connection).unwrap();
    let first = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let second = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "第二档案".to_owned(),
            now_utc_ms: 20,
        },
    )
    .unwrap();
    let foreign = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "外部档案".to_owned(),
            now_utc_ms: 30,
        },
    )
    .unwrap();
    persist_active_profile(&mut connection, "account-1", &first.id, 35).unwrap();

    let duplicate = rename_profile(
        &mut connection,
        RenameProfile {
            account_id: "account-1".to_owned(),
            profile_id: second.id.clone(),
            name: "小树".to_owned(),
            now_utc_ms: 40,
        },
    )
    .expect_err("duplicate names are explicit");
    assert!(matches!(duplicate, ProfileUseCaseError::DuplicateName));

    for forged_id in ["missing-profile", foreign.id.as_str()] {
        assert!(matches!(
            persist_active_profile(&mut connection, "account-1", forged_id, 50),
            Err(ProfileUseCaseError::NotFound)
        ));
    }
    let unchanged: (String, i64, String) = connection
        .query_row(
            "SELECT name, revision,
                (SELECT active_profile_id FROM account_preferences WHERE account_id = 'account-1')
         FROM learner_profiles WHERE id = ?1",
            [&second.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(unchanged, ("第二档案".to_owned(), 1, first.id));
}

#[test]
fn invalid_profile_name_leaves_no_profile_or_outbox_rows() {
    let directory = tempdir().expect("temp directory");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "profile-key")
            .expect("open encrypted database");
    run_migrations(&mut connection).expect("migrate database");

    let error = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "../other".to_owned(),
            now_utc_ms: 1_700_000_000_000,
        },
    )
    .expect_err("path-like profile must fail");

    assert!(matches!(error, ProfileUseCaseError::InvalidName(_)));
    let profiles: i64 = connection
        .query_row("SELECT count(*) FROM learner_profiles", [], |row| {
            row.get(0)
        })
        .unwrap();
    let operations: i64 = connection
        .query_row("SELECT count(*) FROM sync_operations", [], |row| row.get(0))
        .unwrap();
    assert_eq!((profiles, operations), (0, 0));
}

#[test]
fn deleting_the_active_profile_cascades_private_data_and_preserves_shared_assets() {
    let directory = tempdir().expect("temp directory");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "profile-delete-key")
            .unwrap();
    run_migrations(&mut connection).unwrap();
    let first = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "日常学习".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let second = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "竞赛强化".to_owned(),
            now_utc_ms: 20,
        },
    )
    .unwrap();
    persist_active_profile(&mut connection, "account-1", &second.id, 30).unwrap();

    for (asset_id, hash, path) in [
        (
            "018f0000-0000-7000-8000-000000000001",
            "a".repeat(64),
            "aa/shared.blob",
        ),
        (
            "018f0000-0000-7000-8000-000000000002",
            "b".repeat(64),
            "bb/problem-only.blob",
        ),
        (
            "018f0000-0000-7000-8000-000000000003",
            "c".repeat(64),
            "cc/capture-only.blob",
        ),
    ] {
        connection.execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES(?1, 'account-1', ?2, ?3, 8, 'image/png', 40)",
            params![asset_id, hash, path],
        ).unwrap();
    }
    for (problem_id, profile_id) in [
        ("018f0000-0000-7000-8000-000000000011", first.id.as_str()),
        ("018f0000-0000-7000-8000-000000000012", second.id.as_str()),
    ] {
        connection.execute(
            "INSERT INTO problems(id, account_id, profile_id, subject, created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, 'account-1', ?2, '数学', 40, 40, 1)",
            params![problem_id, profile_id],
        ).unwrap();
    }
    connection.execute(
        "INSERT INTO problem_assets(problem_id, asset_id, role, position) VALUES
         ('018f0000-0000-7000-8000-000000000011', '018f0000-0000-7000-8000-000000000001', 'question', 0),
         ('018f0000-0000-7000-8000-000000000012', '018f0000-0000-7000-8000-000000000001', 'question', 0),
         ('018f0000-0000-7000-8000-000000000012', '018f0000-0000-7000-8000-000000000002', 'answer', 0)",
        [],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO review_events(
           id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
           occurred_at_utc_ms, algorithm_version, parameter_version
         ) VALUES(
           '018f0000-0000-7000-8000-000000000031', 'account-1', ?1,
           '018f0000-0000-7000-8000-000000000012',
           '018f0000-0000-7000-8000-000000000032', 'good', 1200, 40, 'fsrs-6', 'default-1'
         )",
            [&second.id],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO schedule_states(
           problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms,
           algorithm_version, parameter_version, rebuilt_at_utc_ms
         ) VALUES(
           '018f0000-0000-7000-8000-000000000012', 100, 1, 5, 40,
           'fsrs-6', 'default-1', 40
         )",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO export_snapshots(
           id, account_id, profile_id, title, problem_ids_json, configuration_json,
           created_at_utc_ms, revision
         ) VALUES(
           '018f0000-0000-7000-8000-000000000033', 'account-1', ?1,
           '待删除导出', '[\"018f0000-0000-7000-8000-000000000012\"]', '{}', 40, 1
         )",
            [&second.id],
        )
        .unwrap();
    connection.execute(
        "INSERT INTO capture_batches(id, account_id, profile_id, subject, state, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES('018f0000-0000-7000-8000-000000000021', 'account-1', ?1, '物理', 'organizing', 40, 40, 1)",
        [&second.id],
    ).unwrap();
    connection.execute(
        "INSERT INTO capture_items(id, batch_id, asset_id, client_upload_id, source_name, source_sequence, width, height, created_at_utc_ms)
         VALUES('018f0000-0000-7000-8000-000000000022', '018f0000-0000-7000-8000-000000000021',
                '018f0000-0000-7000-8000-000000000003', 'capture-delete', 'capture.png', 0, 10, 10, 40)",
        [],
    ).unwrap();

    let mismatch = delete_profile(
        &mut connection,
        DeleteProfile {
            account_id: "account-1".to_owned(),
            profile_id: second.id.clone(),
            confirmation_name: "竞赛".to_owned(),
            now_utc_ms: 49,
        },
    )
    .expect_err("confirmation is checked inside the deletion transaction");
    assert!(matches!(
        mismatch,
        ProfileUseCaseError::ConfirmationMismatch
    ));

    let receipt = delete_profile(
        &mut connection,
        DeleteProfile {
            account_id: "account-1".to_owned(),
            profile_id: second.id.clone(),
            confirmation_name: second.name.clone(),
            now_utc_ms: 50,
        },
    )
    .expect("delete active profile");

    assert_eq!(receipt.deleted_profile_id, second.id);
    assert_eq!(receipt.active_profile.id, first.id);
    assert_eq!(
        receipt
            .orphan_assets
            .iter()
            .map(|asset| asset.encrypted_path.as_str())
            .collect::<Vec<_>>(),
        vec!["bb/problem-only.blob", "cc/capture-only.blob"]
    );
    let active: String = connection
        .query_row(
            "SELECT active_profile_id FROM account_preferences WHERE account_id = 'account-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active, first.id);
    let deleted_profile_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM learner_profiles WHERE id = ?1",
            [&receipt.deleted_profile_id],
            |row| row.get(0),
        )
        .unwrap();
    let deleted_problem_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM problems WHERE profile_id = ?1",
            [&receipt.deleted_profile_id],
            |row| row.get(0),
        )
        .unwrap();
    let deleted_review_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM review_events WHERE profile_id = ?1",
            [&receipt.deleted_profile_id],
            |row| row.get(0),
        )
        .unwrap();
    let deleted_schedule_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM schedule_states WHERE problem_id = '018f0000-0000-7000-8000-000000000012'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let deleted_export_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM export_snapshots WHERE profile_id = ?1",
            [&receipt.deleted_profile_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        (
            deleted_profile_rows,
            deleted_problem_rows,
            deleted_review_rows,
            deleted_schedule_rows,
            deleted_export_rows,
        ),
        (0, 0, 0, 0, 0)
    );
    let remaining_assets: Vec<String> = {
        let mut statement = connection
            .prepare("SELECT encrypted_path FROM assets ORDER BY encrypted_path")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(remaining_assets, vec!["aa/shared.blob"]);
    let profile_delete: (Option<String>, String, String) = connection
        .query_row(
            "SELECT profile_id, entity_type, operation FROM sync_operations
         WHERE entity_id = ?1 ORDER BY created_at_utc_ms DESC LIMIT 1",
            [&receipt.deleted_profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        profile_delete,
        (None, "learner_profile".to_owned(), "delete".to_owned())
    );
    let tombstone_profile: Option<String> = connection.query_row(
        "SELECT profile_id FROM tombstones WHERE entity_type = 'learner_profile' AND entity_id = ?1",
        [&receipt.deleted_profile_id],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(tombstone_profile, None);
}

#[test]
fn deleting_the_last_or_a_foreign_profile_is_rejected_without_mutation() {
    let directory = tempdir().expect("temp directory");
    let mut connection = open_encrypted_database(
        &directory.path().join("library.db"),
        "profile-delete-rejection-key",
    )
    .unwrap();
    run_migrations(&mut connection).unwrap();
    let only = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "唯一档案".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    persist_active_profile(&mut connection, "account-1", &only.id, 20).unwrap();
    let foreign = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "其他账户".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();

    let last = delete_profile(
        &mut connection,
        DeleteProfile {
            account_id: "account-1".to_owned(),
            profile_id: only.id.clone(),
            confirmation_name: only.name.clone(),
            now_utc_ms: 30,
        },
    )
    .expect_err("last profile must remain");
    assert!(matches!(last, ProfileUseCaseError::LastProfile));
    let forged = delete_profile(
        &mut connection,
        DeleteProfile {
            account_id: "account-1".to_owned(),
            profile_id: foreign.id,
            confirmation_name: foreign.name,
            now_utc_ms: 40,
        },
    )
    .expect_err("foreign profile must be hidden");
    assert!(matches!(forged, ProfileUseCaseError::NotFound));

    assert_eq!(list_profiles(&connection, "account-1").unwrap().len(), 1);
    let delete_operations: i64 = connection.query_row(
        "SELECT count(*) FROM sync_operations WHERE account_id = 'account-1' AND operation = 'delete'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(delete_operations, 0);
}
