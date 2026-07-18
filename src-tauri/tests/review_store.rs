use mistake_trainer_next_lib::{
    domain::review::SimpleRating,
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        problems::{
            change_problem_status, create_problem, AssetRole, CaptureAsset, ChangeProblemStatus,
            CreateProblem, ProblemStatusFilter,
        },
        profiles::{create_profile, CreateProfile},
        review::{
            list_review_queue, start_manual_review_queue, submit_review, ReviewQueueQuery,
            ReviewUseCaseError, StartManualReview, SubmitReview,
        },
    },
};
use tempfile::tempdir;

fn create_fixture() -> (tempfile::TempDir, rusqlite::Connection, String, String) {
    let directory = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "review-key").unwrap();
    run_migrations(&mut connection).unwrap();
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 100,
        },
    )
    .unwrap();
    let problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"question".to_vec(),
            }],
            now_utc_ms: 200,
        },
    )
    .unwrap();
    (directory, connection, profile.id, problem.id)
}

#[test]
fn review_queue_includes_new_and_due_problems_and_supports_manual_selection() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    let now = 1_700_000_000_000_i64;

    let initial = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("initial queue");
    assert!(!initial.resumed);
    assert_eq!(initial.mode, "due");
    assert_eq!(initial.completed_count, 0);
    assert_eq!(initial.total_count, 1);
    assert_eq!(initial.items.len(), 1);
    assert_eq!(initial.items[0].problem_id, problem_id);
    assert_eq!(initial.items[0].review_count, 0);

    submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: problem_id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 1_000,
            occurred_at_utc_ms: now,
        },
    )
    .expect("review");
    let session: (i64, String) = connection
        .query_row(
            "SELECT current_index, status FROM review_sessions ORDER BY created_at_utc_ms LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("persisted session progress");
    assert_eq!(session, (1, "completed".to_owned()));

    let due = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("due queue");
    assert!(due.items.is_empty());
    assert_eq!(due.total_count, 0);

    let manual = start_manual_review_queue(
        &mut connection,
        StartManualReview {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_ids: vec![problem_id.clone()],
            now_utc_ms: now,
        },
    )
    .expect("manual queue");
    assert_eq!(manual.mode, "manual");
    assert_eq!(manual.items.len(), 1);
    assert_eq!(manual.items[0].problem_id, problem_id);
    assert_eq!(manual.items[0].review_count, 1);
}

#[test]
fn resumed_session_reports_persisted_progress_and_original_total() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let second = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            subject: "物理".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"second-question".to_vec(),
            }],
            now_utc_ms: 201,
        },
    )
    .unwrap();
    let now = 1_700_000_000_000_i64;

    let initial = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start session");
    assert_eq!(initial.total_count, 2);

    submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: first_problem_id,
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 1_200,
            occurred_at_utc_ms: now + 1,
        },
    )
    .expect("submit first item");

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id,
            now_utc_ms: now + 2,
        },
    )
    .expect("resume session");

    assert!(resumed.resumed);
    assert_eq!(resumed.completed_count, 1);
    assert_eq!(resumed.total_count, 2);
    assert_eq!(resumed.items.len(), 1);
    assert_eq!(resumed.items[0].problem_id, second.id);
}

#[test]
fn remembered_review_commits_event_schedule_and_outbox_together() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    let now = 1_700_000_000_000_i64;

    list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start persisted review session");

    let result = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_id: problem_id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 2_400,
            occurred_at_utc_ms: now,
        },
    )
    .expect("submit review");

    assert_eq!(result.rating, "good");
    assert!(result.due_at_utc_ms > now as f64);
    assert_eq!(result.algorithm_version, "fsrs-6.6.1");
    let counts: (i64, i64, i64) = connection.query_row(
        "SELECT (SELECT count(*) FROM review_events WHERE problem_id = ?1), (SELECT count(*) FROM schedule_states WHERE problem_id = ?1), (SELECT count(*) FROM sync_operations WHERE entity_type = 'review_event' AND entity_id = ?2)",
        [&problem_id, &result.event_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(counts, (1, 1, 1));
    let payload: String = connection
        .query_row(
            "SELECT payload_json FROM sync_operations WHERE entity_type = 'review_event' AND entity_id = ?1",
            [&result.event_id],
            |row| row.get(0),
        )
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(payload["deviceId"], "device-1");
    assert_eq!(payload["durationMs"], 2_400);
    assert_eq!(payload["occurredAtUtcMs"], now);
    assert_eq!(payload["algorithmVersion"], "fsrs-6.6.1");
}

#[test]
fn identical_event_history_produces_identical_schedule_state() {
    let (_directory_a, mut a, profile_a, problem_a) = create_fixture();
    let (_directory_b, mut b, profile_b, problem_b) = create_fixture();
    let now = 1_700_000_000_000_i64;

    for (connection, profile_id) in [(&mut a, profile_a.clone()), (&mut b, profile_b.clone())] {
        list_review_queue(
            connection,
            ReviewQueueQuery {
                account_id: "account-1".to_owned(),
                profile_id,
                now_utc_ms: now,
            },
        )
        .expect("start persisted review session");
    }

    let first = submit_review(
        &mut a,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_a,
            problem_id: problem_a,
            device_id: "device-a".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 1_500,
            occurred_at_utc_ms: now,
        },
    )
    .unwrap();
    let second = submit_review(
        &mut b,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_b,
            problem_id: problem_b,
            device_id: "device-b".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 2_500,
            occurred_at_utc_ms: now,
        },
    )
    .unwrap();

    assert_eq!(first.due_at_utc_ms, second.due_at_utc_ms);
    assert_eq!(first.stability, second.stability);
    assert_eq!(first.difficulty, second.difficulty);
}

#[test]
fn resumed_session_removes_archived_items_before_advancing() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let second = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            subject: "物理".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"second-question".to_vec(),
            }],
            now_utc_ms: 201,
        },
    )
    .unwrap();
    let now = 1_700_000_000_000_i64;

    let initial = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start two-item session");
    assert_eq!(initial.items.len(), 2);
    assert_eq!(initial.items[0].problem_id, first_problem_id);

    change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![first_problem_id],
            target_status: ProblemStatusFilter::Archived,
            now_utc_ms: now + 1,
        },
    )
    .expect("archive current session item");

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 2,
        },
    )
    .expect("resume without archived item");
    assert!(resumed.resumed);
    assert_eq!(resumed.completed_count, 0);
    assert_eq!(resumed.total_count, 1);
    assert_eq!(resumed.items.len(), 1);
    assert_eq!(resumed.items[0].problem_id, second.id);

    submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: second.id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 900,
            occurred_at_utc_ms: now + 3,
        },
    )
    .expect("submit remaining active item");

    let session: (String, i64, String) = connection
        .query_row(
            "SELECT problem_ids_json, current_index, status FROM review_sessions WHERE status = 'completed' ORDER BY updated_at_utc_ms DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        session,
        (
            serde_json::json!([second.id]).to_string(),
            1,
            "completed".to_owned()
        )
    );
}

#[test]
fn review_cannot_cross_account_or_profile_boundaries() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    let before: i64 = connection
        .query_row("SELECT count(*) FROM review_events", [], |row| row.get(0))
        .unwrap();

    let error = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "other-account".to_owned(),
            profile_id,
            problem_id,
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Forgot.into_fsrs(),
            duration_ms: 800,
            occurred_at_utc_ms: 1_700_000_000_000,
        },
    )
    .expect_err("cross-account review must fail");

    assert!(matches!(error, ReviewUseCaseError::ProblemNotFound));
    let after: i64 = connection
        .query_row("SELECT count(*) FROM review_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn manual_review_deck_preserves_selection_order_and_resumes_without_route_ids() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let create = |connection: &mut rusqlite::Connection, subject: &str, now_utc_ms: i64| {
        create_problem(
            connection,
            &directory.path().join("assets"),
            &[47_u8; 32],
            CreateProblem {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                subject: subject.to_owned(),
                note: String::new(),
                assets: vec![CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: format!("{subject}-question").into_bytes(),
                }],
                now_utc_ms,
            },
        )
        .unwrap()
    };
    let second = create(&mut connection, "物理", 201);
    let third = create(&mut connection, "化学", 202);

    let started = start_manual_review_queue(
        &mut connection,
        StartManualReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![
                third.id.clone(),
                first_problem_id.clone(),
                second.id.clone(),
            ],
            now_utc_ms: 1_700_000_000_000,
        },
    )
    .expect("start ordered manual deck");

    assert_eq!(started.mode, "manual");
    assert!(!started.resumed);
    assert_eq!(started.completed_count, 0);
    assert_eq!(started.total_count, 3);
    assert_eq!(
        started
            .items
            .iter()
            .map(|item| item.problem_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            third.id.as_str(),
            first_problem_id.as_str(),
            second.id.as_str()
        ]
    );

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id,
            now_utc_ms: 1_700_000_000_001,
        },
    )
    .expect("resume manual deck without route IDs");

    assert_eq!(resumed.session_id, started.session_id);
    assert_eq!(resumed.mode, "manual");
    assert!(resumed.resumed);
    assert_eq!(resumed.total_count, 3);
    assert_eq!(
        resumed
            .items
            .iter()
            .map(|item| item.problem_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            third.id.as_str(),
            first_problem_id.as_str(),
            second.id.as_str()
        ]
    );
}

#[test]
fn invalid_manual_decks_leave_the_existing_session_unchanged() {
    let (directory, mut connection, profile_id, problem_id) = create_fixture();
    let now = 1_700_000_000_000_i64;
    list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start due session");
    let before: (String, String, String, i64, String, i64, i64) = connection
        .query_row(
            "SELECT id, mode, problem_ids_json, current_index, status, created_at_utc_ms, updated_at_utc_ms
             FROM review_sessions WHERE status = 'active'",
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

    let other_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "另一档案".to_owned(),
            now_utc_ms: now + 1,
        },
    )
    .unwrap();
    let foreign = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: other_profile.id,
            subject: "外部".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"foreign-question".to_vec(),
            }],
            now_utc_ms: now + 2,
        },
    )
    .unwrap();
    let archived = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            subject: "已归档".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"archived-question".to_vec(),
            }],
            now_utc_ms: now + 3,
        },
    )
    .unwrap();
    change_problem_status(
        &mut connection,
        ChangeProblemStatus {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![archived.id.clone()],
            target_status: ProblemStatusFilter::Archived,
            now_utc_ms: now + 4,
        },
    )
    .unwrap();

    let invalid_selections = vec![
        vec![],
        vec![problem_id.clone(), problem_id],
        (0..101).map(|index| format!("missing-{index}")).collect(),
        vec!["missing-problem".to_owned()],
        vec![foreign.id],
        vec![archived.id],
    ];
    for (index, problem_ids) in invalid_selections.into_iter().enumerate() {
        let error = start_manual_review_queue(
            &mut connection,
            StartManualReview {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                problem_ids,
                now_utc_ms: now + 10 + index as i64,
            },
        )
        .expect_err("invalid selection must fail");
        assert!(matches!(error, ReviewUseCaseError::InvalidManualSelection));

        let after: (String, String, String, i64, String, i64, i64) = connection
            .query_row(
                "SELECT id, mode, problem_ids_json, current_index, status, created_at_utc_ms, updated_at_utc_ms
                 FROM review_sessions WHERE status = 'active'",
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
        assert_eq!(after, before);
    }
}
