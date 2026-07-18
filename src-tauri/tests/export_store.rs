use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        exports::{
            CreateExportSnapshot, ExportCandidateSource, ExportError, ExportLayout,
            create_export_snapshot, delete_export_snapshot, list_deleted_export_snapshots,
            list_export_candidates, list_export_snapshots, restore_export_snapshot,
        },
        problems::{
            AssetRole, CaptureAsset, ChangeProblemStatus, CreateProblem, ProblemStatusFilter,
            change_problem_status, create_problem,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

#[test]
fn export_snapshots_validate_selection_and_write_sync_operations_atomically() {
    let directory = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "key").unwrap();
    run_migrations(&mut connection).unwrap();
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[8_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"q".to_vec(),
            }],
            now_utc_ms: 20,
        },
    )
    .unwrap();
    let second = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[8_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "物理".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"q2".to_vec(),
            }],
            now_utc_ms: 21,
        },
    )
    .unwrap();

    let invalid = create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            title: "无效".to_owned(),
            problem_ids: vec!["not-owned".to_owned()],
            layout: ExportLayout::QuestionAnswerAlternating,
            now_utc_ms: 30,
        },
    )
    .expect_err("unknown problems must not create a snapshot");
    assert!(matches!(invalid, ExportError::ProblemNotFound));

    let snapshot = create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            title: "  第一次复盘  ".to_owned(),
            problem_ids: vec![second.id.clone(), problem.id.clone(), second.id.clone()],
            layout: ExportLayout::QuestionsThenAnswers,
            now_utc_ms: 40,
        },
    )
    .unwrap();
    assert_eq!(snapshot.title, "第一次复盘");
    assert_eq!(snapshot.problem_count, 2);
    let stored_order: String = connection
        .query_row(
            "SELECT problem_ids_json FROM export_snapshots WHERE id = ?1",
            [&snapshot.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        stored_order,
        serde_json::json!([second.id, problem.id]).to_string()
    );
    let listed = list_export_snapshots(&connection, "account-1", &profile.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, snapshot.id);
    let create_outbox: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sync_operations WHERE entity_type = 'export_snapshot' AND entity_id = ?1 AND operation = 'upsert'",
        [&snapshot.id], |row| row.get(0),
    ).unwrap();
    assert_eq!(create_outbox, 1);

    let wrong_profile = delete_export_snapshot(
        &mut connection,
        "account-1",
        "other-profile",
        &snapshot.id,
        49,
    )
    .expect_err("another profile must not delete the snapshot");
    assert!(matches!(wrong_profile, ExportError::SnapshotNotFound));
    delete_export_snapshot(&mut connection, "account-1", &profile.id, &snapshot.id, 50).unwrap();
    assert!(
        list_export_snapshots(&connection, "account-1", &profile.id)
            .unwrap()
            .is_empty()
    );
    let retained: (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM export_snapshots WHERE id = ?1),
                (SELECT COUNT(*) FROM tombstones WHERE entity_type = 'export_snapshot' AND entity_id = ?1)",
        [&snapshot.id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!(retained, (1, 1));
    let deleted = list_deleted_export_snapshots(&connection, "account-1", &profile.id).unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].snapshot.id, snapshot.id);
    assert_eq!(deleted[0].deleted_at_utc_ms, 50.0);
    assert_eq!(
        deleted[0].purge_after_utc_ms,
        (50_i64 + 30_i64 * 86_400_000_i64) as f64
    );
    let (delete_outbox, delete_payload, deleted_revision): (i64, String, i64) = connection.query_row(
        "SELECT COUNT(*), payload_json,
                (SELECT revision FROM export_snapshots WHERE id = ?1)
         FROM sync_operations WHERE entity_type = 'export_snapshot' AND entity_id = ?1 AND operation = 'delete'",
        [&snapshot.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(delete_outbox, 1);
    let delete_payload: serde_json::Value = serde_json::from_str(&delete_payload).unwrap();
    assert_eq!(delete_payload["baseRevision"], 1);
    assert_eq!(delete_payload["revision"], 2);
    assert_eq!(deleted_revision, 2);

    restore_export_snapshot(&mut connection, "account-1", &profile.id, &snapshot.id, 60).unwrap();
    assert_eq!(
        list_export_snapshots(&connection, "account-1", &profile.id)
            .unwrap()
            .len(),
        1
    );
    assert!(
        list_deleted_export_snapshots(&connection, "account-1", &profile.id)
            .unwrap()
            .is_empty()
    );
    let (restore_outbox, restore_payload, restored_revision): (i64, String, i64) = connection.query_row(
        "SELECT COUNT(*), payload_json,
                (SELECT revision FROM export_snapshots WHERE id = ?1)
         FROM sync_operations WHERE entity_type = 'export_snapshot' AND entity_id = ?1 AND operation = 'restore'",
        [&snapshot.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(restore_outbox, 1);
    let restore_payload: serde_json::Value = serde_json::from_str(&restore_payload).unwrap();
    assert_eq!(restore_payload["baseRevision"], 2);
    assert_eq!(restore_payload["revision"], 3);
    assert_eq!(restored_revision, 3);
}

#[test]
fn export_candidates_are_profile_scoped_ordered_and_side_effect_free() {
    let directory = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "key").unwrap();
    run_migrations(&mut connection).unwrap();
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let other_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "另一档案".to_owned(),
            now_utc_ms: 11,
        },
    )
    .unwrap();
    let create = |connection: &mut rusqlite::Connection,
                  profile_id: &str,
                  subject: &str,
                  now_utc_ms: i64| {
        create_problem(
            connection,
            &directory.path().join("assets"),
            &[8_u8; 32],
            CreateProblem {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.to_owned(),
                subject: subject.to_owned(),
                note: format!("{subject}复盘"),
                assets: vec![
                    CaptureAsset {
                        role: AssetRole::Question,
                        media_type: "image/png".to_owned(),
                        bytes: format!("{subject}-question").into_bytes(),
                    },
                    CaptureAsset {
                        role: AssetRole::Answer,
                        media_type: "image/png".to_owned(),
                        bytes: format!("{subject}-answer").into_bytes(),
                    },
                ],
                now_utc_ms,
            },
        )
        .unwrap()
    };
    let new_problem = create(&mut connection, &profile.id, "数学", 20);
    let due_problem = create(&mut connection, &profile.id, "物理", 21);
    let future_problem = create(&mut connection, &profile.id, "化学", 22);
    let trashed_problem = create(&mut connection, &profile.id, "历史", 23);
    let _foreign_problem = create(&mut connection, &other_profile.id, "外部", 24);

    connection
        .execute(
            "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, algorithm_version, parameter_version, rebuilt_at_utc_ms)
             VALUES(?1, 900, 1, 5, 'test', 'test', 30), (?2, 2000, 1, 5, 'test', 'test', 30)",
            rusqlite::params![due_problem.id, future_problem.id],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
             VALUES('review-1', 'account-1', ?1, ?2, 'device-1', 'good', 1000, 800, 'test', 'test')",
            rusqlite::params![profile.id, due_problem.id],
        )
        .unwrap();
    change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            problem_ids: vec![trashed_problem.id.clone()],
            target_status: ProblemStatusFilter::Trashed,
            now_utc_ms: 40,
        },
    )
    .unwrap();
    connection
        .execute(
            "INSERT INTO review_sessions(id, account_id, profile_id, mode, problem_ids_json, current_index, status, created_at_utc_ms, updated_at_utc_ms)
             VALUES('session-1', 'account-1', ?1, 'due', ?2, 3, 'completed', 50, 60)",
            rusqlite::params![
                profile.id,
                serde_json::json!([
                    future_problem.id.clone(),
                    due_problem.id.clone(),
                    trashed_problem.id.clone()
                ])
                .to_string()
            ],
        )
        .unwrap();
    let sessions_before: Vec<(String, String, i64)> = connection
        .prepare("SELECT id, status, current_index FROM review_sessions ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let due = list_export_candidates(
        &connection,
        "account-1",
        &profile.id,
        ExportCandidateSource::Due,
        1_000,
    )
    .unwrap();
    assert_eq!(
        due.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
        vec![new_problem.id.as_str(), due_problem.id.as_str()]
    );
    assert_eq!(due[0].question_asset_count, 1);
    assert_eq!(due[0].answer_asset_count, 1);
    assert_eq!(due[1].review_count, 1);

    let recent = list_export_candidates(
        &connection,
        "account-1",
        &profile.id,
        ExportCandidateSource::LatestReviewSession,
        1_000,
    )
    .unwrap();
    assert_eq!(
        recent
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![future_problem.id.as_str(), due_problem.id.as_str()]
    );

    let all = list_export_candidates(
        &connection,
        "account-1",
        &profile.id,
        ExportCandidateSource::AllActive,
        1_000,
    )
    .unwrap();
    assert_eq!(all.len(), 3);
    assert!(
        all.iter()
            .all(|item| item.subject != "外部" && item.subject != "历史")
    );

    let sessions_after: Vec<(String, String, i64)> = connection
        .prepare("SELECT id, status, current_index FROM review_sessions ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(sessions_after, sessions_before);
}
