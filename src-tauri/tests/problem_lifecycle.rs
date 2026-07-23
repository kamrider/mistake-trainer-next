use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        problems::{
            AssetRole, CaptureAsset, ChangeProblemStatus, CreateProblem, ProblemStatusFilter,
            ProblemUseCaseError, UpdateProblem, change_problem_status, create_problem,
            update_problem,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

fn fixture() -> (tempfile::TempDir, rusqlite::Connection, String, String) {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "lifecycle-key")
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
    let problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[91_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: "旧笔记".to_owned(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"question".to_vec(),
            }],
            now_utc_ms: 20,
        },
    )
    .expect("problem");
    (directory, connection, profile.id, problem.id)
}

fn insert_open_problem_conflict(
    connection: &rusqlite::Connection,
    profile_id: &str,
    problem_id: &str,
) {
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-444444444444',
               'account-1', ?1, 'problem', ?2, 'note',
               '\"旧笔记\"', '\"云端笔记\"', 1, 25
             )",
            rusqlite::params![profile_id, problem_id],
        )
        .expect("open conflict");
}

#[test]
fn unresolved_conflict_blocks_problem_edits_and_status_changes() {
    let (_directory, mut connection, profile_id, problem_id) = fixture();
    insert_open_problem_conflict(&connection, &profile_id, &problem_id);

    let update_error = update_problem(
        &mut connection,
        UpdateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: problem_id.clone(),
            subject: "物理".to_owned(),
            tags: vec![],
            note: "不应写入".to_owned(),
            time_limit_seconds: None,
            now_utc_ms: 30,
        },
    )
    .expect_err("open conflict must block editing");
    assert!(matches!(update_error, ProblemUseCaseError::ConflictPending));

    let status_error = change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_ids: vec![problem_id.clone()],
            target_status: ProblemStatusFilter::Archived,
            now_utc_ms: 31,
        },
    )
    .expect_err("open conflict must block status changes");
    assert!(matches!(status_error, ProblemUseCaseError::ConflictPending));

    let unchanged: (String, String, String, i64) = connection
        .query_row(
            "SELECT subject, note, status, revision FROM problems WHERE id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("unchanged problem");
    assert_eq!(
        unchanged,
        (
            "数学".to_owned(),
            "旧笔记".to_owned(),
            "active".to_owned(),
            1
        )
    );
}

#[test]
fn edit_updates_revision_and_outbox_in_one_transaction() {
    let (_directory, mut connection, profile_id, problem_id) = fixture();
    update_problem(
        &mut connection,
        UpdateProblem {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_id: problem_id.clone(),
            subject: "高等数学".to_owned(),
            tags: vec![" 函数 ".to_owned(), "粗心".to_owned(), "函数".to_owned()],
            note: "先检查定义域".to_owned(),
            time_limit_seconds: Some(180),
            now_utc_ms: 30,
        },
    )
    .expect("update");

    let row: (String, String, String, Option<i32>, i64) = connection
        .query_row(
            "SELECT subject, tags_json, note, time_limit_seconds, revision FROM problems WHERE id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .expect("updated row");
    assert_eq!(
        row,
        (
            "高等数学".to_owned(),
            "[\"函数\",\"粗心\"]".to_owned(),
            "先检查定义域".to_owned(),
            Some(180),
            2
        )
    );
    let outbox: i64 = connection
        .query_row(
            "SELECT count(*) FROM sync_operations WHERE entity_id = ?1 AND operation = 'upsert'",
            [&problem_id],
            |row| row.get(0),
        )
        .expect("outbox");
    assert_eq!(outbox, 2);
    let payload: String = connection
        .query_row(
            "SELECT payload_json FROM sync_operations WHERE entity_id = ?1 ORDER BY created_at_utc_ms DESC LIMIT 1",
            [&problem_id],
            |row| row.get(0),
        )
        .expect("update payload");
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(payload["baseRevision"], 1);
    assert_eq!(payload["revision"], 2);
    assert_eq!(payload["updatedAtUtcMs"], 30);
    assert_eq!(payload["timeLimitSeconds"], 180);
    assert_eq!(payload["tags"], serde_json::json!(["函数", "粗心"]));
}

#[test]
fn invalid_time_limit_does_not_change_problem_or_outbox() {
    let (_directory, mut connection, profile_id, problem_id) = fixture();
    let error = update_problem(
        &mut connection,
        UpdateProblem {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_id: problem_id.clone(),
            subject: "math".to_owned(),
            tags: vec![],
            note: "should not persist".to_owned(),
            time_limit_seconds: Some(0),
            now_utc_ms: 30,
        },
    )
    .expect_err("zero-second limit must be rejected");

    assert!(matches!(error, ProblemUseCaseError::InvalidTimeLimit));
    let row: (String, String, Option<i32>, i64) = connection
        .query_row(
            "SELECT subject, note, time_limit_seconds, revision FROM problems WHERE id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("unchanged problem");
    assert_eq!(row.0, "数学");
    assert_eq!(row.1, "旧笔记");
    assert_eq!(row.2, None);
    assert_eq!(row.3, 1);
    let outbox_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sync_operations WHERE entity_id = ?1",
            [&problem_id],
            |row| row.get(0),
        )
        .expect("outbox count");
    assert_eq!(outbox_count, 1);
}

#[test]
fn invalid_tags_do_not_change_problem_or_outbox() {
    let (_directory, mut connection, profile_id, problem_id) = fixture();
    let error = update_problem(
        &mut connection,
        UpdateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: problem_id.clone(),
            subject: "math".to_owned(),
            tags: (0..21).map(|index| format!("tag-{index}")).collect(),
            note: "should not persist".to_owned(),
            time_limit_seconds: None,
            now_utc_ms: 30,
        },
    )
    .expect_err("more than twenty tags must be rejected");

    assert!(matches!(error, ProblemUseCaseError::InvalidTags));
    let error = update_problem(
        &mut connection,
        UpdateProblem {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_id: problem_id.clone(),
            subject: "math".to_owned(),
            tags: vec!["x".repeat(31)],
            note: "should not persist".to_owned(),
            time_limit_seconds: None,
            now_utc_ms: 31,
        },
    )
    .expect_err("a tag longer than thirty characters must be rejected");
    assert!(matches!(error, ProblemUseCaseError::InvalidTags));
    let row: (String, String, i64) = connection
        .query_row(
            "SELECT subject, tags_json, revision FROM problems WHERE id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("unchanged problem");
    assert_eq!(row.0, "数学");
    assert_eq!(row.1, "[]");
    assert_eq!(row.2, 1);
    let outbox_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sync_operations WHERE entity_id = ?1",
            [&problem_id],
            |row| row.get(0),
        )
        .expect("outbox count");
    assert_eq!(outbox_count, 1);
}

#[test]
fn trash_and_restore_manage_tombstone_revision_and_outbox() {
    let (_directory, mut connection, profile_id, problem_id) = fixture();
    change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![problem_id.clone()],
            target_status: ProblemStatusFilter::Trashed,
            now_utc_ms: 100,
        },
    )
    .expect("trash");
    let tombstone: (i64, i64) = connection
        .query_row(
            "SELECT revision, purge_after_utc_ms FROM tombstones WHERE entity_id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("tombstone");
    assert_eq!(tombstone.0, 2);
    assert_eq!(tombstone.1, 100 + 30 * 86_400_000);
    let delete_payload: String = connection
        .query_row(
            "SELECT payload_json FROM sync_operations WHERE entity_id = ?1 AND operation = 'delete'",
            [&problem_id],
            |row| row.get(0),
        )
        .expect("delete payload");
    let delete_payload: serde_json::Value = serde_json::from_str(&delete_payload).unwrap();
    assert_eq!(delete_payload["deletedAtUtcMs"], 100);
    assert_eq!(
        delete_payload["purgeAfterUtcMs"],
        100_i64 + 30 * 86_400_000_i64
    );

    change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_ids: vec![problem_id.clone()],
            target_status: ProblemStatusFilter::Active,
            now_utc_ms: 200,
        },
    )
    .expect("restore");
    let row: (String, i64) = connection
        .query_row(
            "SELECT status, revision FROM problems WHERE id = ?1",
            [&problem_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("restored row");
    assert_eq!(row, ("active".to_owned(), 3));
    let tombstones: i64 = connection
        .query_row("SELECT count(*) FROM tombstones", [], |row| row.get(0))
        .expect("tombstone count");
    assert_eq!(tombstones, 0);
}
