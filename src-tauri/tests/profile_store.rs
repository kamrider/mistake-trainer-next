use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::profiles::{CreateProfile, ProfileUseCaseError, create_profile},
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
