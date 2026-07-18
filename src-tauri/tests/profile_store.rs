use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::profiles::{
        CreateProfile, ProfileUseCaseError, RenameProfile, create_profile, list_profiles,
        persist_active_profile, rename_profile,
    },
};
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
