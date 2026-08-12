use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        problem_bulk_metadata::{ProblemBulkMetadata, update_problem_bulk_metadata},
        problems::ProblemUseCaseError,
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

#[test]
fn bulk_metadata_updates_active_selection_atomically_with_one_outbox_each() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "bulk-metadata-key")
            .expect("database");
    run_migrations(&mut connection).expect("migrations");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    insert_problem(
        &connection,
        "p1",
        &profile.id,
        "数学",
        r#"["函数","保留"]"#,
        1,
    );
    insert_problem(
        &connection,
        "p2",
        &profile.id,
        "物理",
        r#"["保留","旧标签"]"#,
        4,
    );
    let outbox_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox before");

    let report = update_problem_bulk_metadata(
        &mut connection,
        ProblemBulkMetadata {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            problem_ids: vec!["p1".to_owned(), "p2".to_owned()],
            subject: Some(" 代数 ".to_owned()),
            add_tags: vec!["重点".to_owned(), "保留".to_owned()],
            remove_tags: vec!["函数".to_owned(), "旧标签".to_owned()],
            now_utc_ms: 100,
        },
    )
    .expect("bulk update");

    assert_eq!(report.updated_count, 2);
    let rows = connection
        .prepare("SELECT id, subject, tags_json, revision FROM problems ORDER BY id")
        .expect("prepare")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows");
    assert_eq!(
        rows,
        vec![
            (
                "p1".to_owned(),
                "代数".to_owned(),
                r#"["保留","重点"]"#.to_owned(),
                2
            ),
            (
                "p2".to_owned(),
                "代数".to_owned(),
                r#"["保留","重点"]"#.to_owned(),
                5
            ),
        ]
    );
    let outbox_after: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox after");
    assert_eq!(outbox_after - outbox_before, 2);
}

#[test]
fn invalid_or_mixed_selection_rolls_back_every_row_and_outbox() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "bulk-rollback-key")
            .expect("database");
    run_migrations(&mut connection).expect("migrations");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    insert_problem(&connection, "active", &profile.id, "数学", "[]", 1);
    insert_problem(&connection, "archived", &profile.id, "物理", "[]", 1);
    connection
        .execute(
            "UPDATE problems SET status = 'archived' WHERE id = 'archived'",
            [],
        )
        .expect("archive");
    let outbox_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox before");

    let result = update_problem_bulk_metadata(
        &mut connection,
        ProblemBulkMetadata {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            problem_ids: vec!["active".to_owned(), "archived".to_owned()],
            subject: Some("统一科目".to_owned()),
            add_tags: vec![],
            remove_tags: vec![],
            now_utc_ms: 100,
        },
    );

    assert!(matches!(result, Err(ProblemUseCaseError::ProblemNotFound)));
    let active_subject: String = connection
        .query_row(
            "SELECT subject FROM problems WHERE id = 'active'",
            [],
            |row| row.get(0),
        )
        .expect("active subject");
    assert_eq!(active_subject, "数学");
    let outbox_after: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox after");
    assert_eq!(outbox_after, outbox_before);
}

#[test]
fn duplicate_or_invalid_tag_input_is_rejected_before_mutation() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "bulk-validation-key")
            .expect("database");
    run_migrations(&mut connection).expect("migrations");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    insert_problem(&connection, "p1", &profile.id, "数学", "[]", 1);

    for input in [
        ProblemBulkMetadata {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            problem_ids: vec!["p1".to_owned(), "p1".to_owned()],
            subject: Some("代数".to_owned()),
            add_tags: vec![],
            remove_tags: vec![],
            now_utc_ms: 100,
        },
        ProblemBulkMetadata {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            problem_ids: vec!["p1".to_owned()],
            subject: None,
            add_tags: vec!["过长".repeat(16)],
            remove_tags: vec![],
            now_utc_ms: 100,
        },
    ] {
        assert!(update_problem_bulk_metadata(&mut connection, input).is_err());
    }

    let row: (String, i64) = connection
        .query_row(
            "SELECT subject, revision FROM problems WHERE id = 'p1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("problem");
    assert_eq!(row, ("数学".to_owned(), 1));
}

fn insert_problem(
    connection: &rusqlite::Connection,
    id: &str,
    profile_id: &str,
    subject: &str,
    tags_json: &str,
    revision: i64,
) {
    connection
        .execute(
            "INSERT INTO problems(id, account_id, profile_id, subject, tags_json, note, status, created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, 'account-1', ?2, ?3, ?4, '', 'active', 20, 20, ?5)",
            rusqlite::params![id, profile_id, subject, tags_json, revision],
        )
        .expect("problem fixture");
}
