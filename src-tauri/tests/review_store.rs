use mistake_trainer_next_lib::{
    domain::review::SimpleRating,
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        preferences::{ReviewFocusPolicy, SaveReviewPreferences, save_review_preferences},
        problems::{
            AssetRole, CaptureAsset, ChangeProblemStatus, CreateProblem, ProblemStatusFilter,
            change_problem_status, create_problem,
        },
        profiles::{CreateProfile, create_profile},
        review::{
            BeginExamGrading, NavigateExam, QuickReviewPreset, ReviewQueueQuery,
            ReviewUseCaseError, StartExamReview, StartManualReview, StartQuickReview, SubmitReview,
            begin_exam_grading, list_review_queue, navigate_exam, start_exam_review_queue,
            start_manual_review_queue, start_quick_review_queue, submit_review,
        },
        review_focus::{
            FocusNumberSelection, ReviewFocusError, SkipReviewFocus, select_focus_number,
            skip_focus_round,
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
fn exam_hides_grading_until_the_persisted_answering_pass_is_complete() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let second_problem = create_problem(
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
                bytes: b"exam-second-question".to_vec(),
            }],
            now_utc_ms: 201,
        },
    )
    .unwrap();
    let now = 1_700_100_000_000_i64;

    list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("create the due session that the exam replaces");

    let started = start_exam_review_queue(
        &mut connection,
        StartExamReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![second_problem.id.clone(), first_problem_id.clone()],
            now_utc_ms: now + 1,
        },
    )
    .expect("start exam");
    assert_eq!(started.mode, "exam");
    assert_eq!(started.exam_phase.as_deref(), Some("answering"));
    assert_eq!(started.exam_question_index, 0);
    assert_eq!(started.completed_count, 0);
    assert_eq!(started.total_count, 2);
    assert_eq!(
        started
            .items
            .iter()
            .map(|item| item.problem_id.as_str())
            .collect::<Vec<_>>(),
        vec![second_problem.id.as_str(), first_problem_id.as_str()]
    );

    assert!(matches!(
        begin_exam_grading(
            &mut connection,
            BeginExamGrading {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                now_utc_ms: now + 2,
            }
        ),
        Err(ReviewUseCaseError::InvalidExamState)
    ));

    let navigated = navigate_exam(
        &mut connection,
        NavigateExam {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            position: 1,
            now_utc_ms: now + 2,
        },
    )
    .expect("persist navigation");
    assert_eq!(navigated.exam_question_index, 1);
    assert!(matches!(
        navigate_exam(
            &mut connection,
            NavigateExam {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                position: 2,
                now_utc_ms: now + 3,
            }
        ),
        Err(ReviewUseCaseError::InvalidExamState)
    ));

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 4,
        },
    )
    .expect("resume answering pass");
    assert!(resumed.resumed);
    assert_eq!(resumed.mode, "exam");
    assert_eq!(resumed.exam_phase.as_deref(), Some("answering"));
    assert_eq!(resumed.exam_question_index, 1);

    let early_grade = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: second_problem.id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Forgot.into_fsrs(),
            duration_ms: 900,
            occurred_at_utc_ms: now + 5,
        },
    );
    assert!(matches!(
        early_grade,
        Err(ReviewUseCaseError::SessionOutOfSync)
    ));
    let event_count: i64 = connection
        .query_row("SELECT count(*) FROM review_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(event_count, 0, "a forbidden early grade must roll back");

    let grading = begin_exam_grading(
        &mut connection,
        BeginExamGrading {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 6,
        },
    )
    .expect("begin grading");
    assert_eq!(grading.exam_phase.as_deref(), Some("grading"));
    assert_eq!(grading.exam_question_index, 0);

    submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: second_problem.id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Forgot.into_fsrs(),
            duration_ms: 1_000,
            occurred_at_utc_ms: now + 7,
        },
    )
    .expect("grade wrong");
    let after_wrong = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 8,
        },
    )
    .expect("resume grading");
    assert_eq!(after_wrong.completed_count, 1);
    assert_eq!(after_wrong.exam_correct_count, 0);
    assert_eq!(after_wrong.exam_wrong_count, 1);
    assert_eq!(after_wrong.items[0].problem_id, first_problem_id);

    submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: first_problem_id,
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 1_100,
            occurred_at_utc_ms: now + 9,
        },
    )
    .expect("grade correct");
    let completed: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT status, current_index, exam_correct_count, exam_wrong_count
             FROM review_sessions WHERE id = ?1",
            [started.session_id.unwrap()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(completed, ("completed".to_owned(), 2, 1, 1));
}

#[test]
fn answering_exam_keeps_the_same_question_selected_when_an_earlier_card_is_archived() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let second_problem = create_problem(
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
                bytes: b"exam-archive-second".to_vec(),
            }],
            now_utc_ms: 201,
        },
    )
    .unwrap();
    let third_problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[47_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            subject: "化学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"exam-archive-third".to_vec(),
            }],
            now_utc_ms: 202,
        },
    )
    .unwrap();
    let now = 1_700_200_000_000_i64;

    start_exam_review_queue(
        &mut connection,
        StartExamReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: vec![
                first_problem_id.clone(),
                second_problem.id.clone(),
                third_problem.id.clone(),
            ],
            now_utc_ms: now,
        },
    )
    .unwrap();
    navigate_exam(
        &mut connection,
        NavigateExam {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            position: 1,
            now_utc_ms: now + 1,
        },
    )
    .unwrap();
    connection
        .execute(
            "UPDATE problems SET status = 'archived' WHERE id = ?1",
            [&first_problem_id],
        )
        .unwrap();

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id,
            now_utc_ms: now + 2,
        },
    )
    .unwrap();

    assert_eq!(resumed.exam_question_index, 0);
    assert_eq!(
        resumed
            .items
            .iter()
            .map(|item| item.problem_id.as_str())
            .collect::<Vec<_>>(),
        vec![second_problem.id.as_str(), third_problem.id.as_str()]
    );
    assert_eq!(
        resumed.items[resumed.exam_question_index as usize].problem_id,
        second_problem.id
    );
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

#[test]
fn session_start_focus_board_is_stable_restart_safe_and_fail_closed() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    save_review_preferences(
        &connection,
        "account-1",
        &profile_id,
        SaveReviewPreferences {
            focus_policy: ReviewFocusPolicy::SessionStart,
        },
        300,
    )
    .unwrap();
    let now = 1_700_300_000_000_i64;

    let started = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start focus-enabled session");
    let focus = started.focus.expect("session start focus");
    assert_eq!(focus.kind, "warmup");
    assert_eq!(focus.round_index, 0);
    assert_eq!(focus.next_number, 1);
    assert_eq!(focus.elapsed_ms, 0);
    let mut sorted = focus.numbers.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (1..=25).collect::<Vec<_>>());

    let resumed = list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 1,
        },
    )
    .expect("resume same focus board");
    assert_eq!(resumed.focus.as_ref().unwrap().numbers, focus.numbers);
    assert_eq!(resumed.focus.as_ref().unwrap().next_number, 1);

    let before_wrong: (String, i64, i64) = connection
        .query_row(
            "SELECT focus_order_json, focus_next_number, focus_elapsed_ms
             FROM review_sessions WHERE status = 'active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert!(matches!(
        select_focus_number(
            &mut connection,
            FocusNumberSelection {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                number: 2,
                elapsed_ms: 50,
                now_utc_ms: now + 2,
            }
        ),
        Err(ReviewFocusError::StateChanged)
    ));
    let after_wrong: (String, i64, i64) = connection
        .query_row(
            "SELECT focus_order_json, focus_next_number, focus_elapsed_ms
             FROM review_sessions WHERE status = 'active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(after_wrong, before_wrong);

    let progressed = select_focus_number(
        &mut connection,
        FocusNumberSelection {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            number: 1,
            elapsed_ms: 1_234,
            now_utc_ms: now + 3,
        },
    )
    .expect("persist the expected number")
    .expect("focus remains active");
    assert_eq!(progressed.next_number, 2);
    assert_eq!(progressed.elapsed_ms, 1_234);

    let blocked = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id,
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 100,
            occurred_at_utc_ms: now + 4,
        },
    );
    assert!(matches!(blocked, Err(ReviewUseCaseError::SessionOutOfSync)));
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM review_events", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );

    let skipped = skip_focus_round(
        &mut connection,
        SkipReviewFocus {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 5,
        },
    )
    .expect("skip focus round");
    assert!(skipped.is_none());
    let persisted: (i64, Option<String>, i64) = connection
        .query_row(
            "SELECT focus_round, focus_order_json, focus_next_number
             FROM review_sessions WHERE status = 'active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(persisted, (1, None, 0));
}

#[test]
fn focus_completion_clamps_elapsed_and_rejects_foreign_profile() {
    let (_directory, mut connection, profile_id, _problem_id) = create_fixture();
    save_review_preferences(
        &connection,
        "account-1",
        &profile_id,
        SaveReviewPreferences {
            focus_policy: ReviewFocusPolicy::SessionStart,
        },
        350,
    )
    .unwrap();
    let now = 1_700_350_000_000_i64;
    list_review_queue(
        &mut connection,
        ReviewQueueQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now,
        },
    )
    .expect("start focus-enabled session");

    let foreign = select_focus_number(
        &mut connection,
        FocusNumberSelection {
            account_id: "account-1".to_owned(),
            profile_id: "foreign-profile".to_owned(),
            number: 1,
            elapsed_ms: 10,
            now_utc_ms: now + 1,
        },
    );
    assert!(matches!(foreign, Err(ReviewFocusError::StateChanged)));

    for number in 1..=25 {
        let result = select_focus_number(
            &mut connection,
            FocusNumberSelection {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                number,
                elapsed_ms: u32::MAX,
                now_utc_ms: now + i64::from(number) + 1,
            },
        )
        .expect("persist focus number");
        if number < 25 {
            let state = result.expect("round remains active");
            assert_eq!(state.next_number, number + 1);
            assert_eq!(state.elapsed_ms, 3_600_000);
        } else {
            assert!(result.is_none());
        }
    }

    let persisted: (i64, Option<String>, i64, i64) = connection
        .query_row(
            "SELECT focus_round, focus_order_json, focus_next_number, focus_elapsed_ms
             FROM review_sessions WHERE status = 'active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(persisted, (1, None, 0, 3_600_000));
}

#[test]
fn every_ten_focus_is_created_transactionally_and_never_trails_completion() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let mut problem_ids = vec![first_problem_id];
    for index in 1..11 {
        let problem = create_problem(
            &mut connection,
            &directory.path().join("assets"),
            &[47_u8; 32],
            CreateProblem {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                subject: format!("科目{index}"),
                note: String::new(),
                assets: vec![CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: format!("focus-question-{index}").into_bytes(),
                }],
                now_utc_ms: 200 + index,
            },
        )
        .unwrap();
        problem_ids.push(problem.id);
    }
    save_review_preferences(
        &connection,
        "account-1",
        &profile_id,
        SaveReviewPreferences {
            focus_policy: ReviewFocusPolicy::EveryTen,
        },
        400,
    )
    .unwrap();
    let now = 1_700_400_000_000_i64;
    let started = start_manual_review_queue(
        &mut connection,
        StartManualReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_ids: problem_ids.clone(),
            now_utc_ms: now,
        },
    )
    .unwrap();
    assert!(started.focus.is_none());

    for (index, problem_id) in problem_ids.iter().take(10).enumerate() {
        let result = submit_review(
            &mut connection,
            SubmitReview {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                problem_id: problem_id.clone(),
                device_id: "device-1".to_owned(),
                rating: SimpleRating::Remembered.into_fsrs(),
                duration_ms: 100,
                occurred_at_utc_ms: now + index as i64 + 1,
            },
        )
        .unwrap();
        if index < 9 {
            assert!(result.focus.is_none());
        } else {
            let focus = result.focus.expect("focus follows card ten");
            assert_eq!(focus.kind, "break");
            assert_eq!(focus.next_number, 1);
        }
    }
    let session: (i64, String, i64) = connection
        .query_row(
            "SELECT current_index, status, focus_round FROM review_sessions WHERE status='active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(session, (10, "active".to_owned(), 0));

    let blocked = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: problem_ids[10].clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 100,
            occurred_at_utc_ms: now + 20,
        },
    );
    assert!(matches!(blocked, Err(ReviewUseCaseError::SessionOutOfSync)));

    skip_focus_round(
        &mut connection,
        SkipReviewFocus {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            now_utc_ms: now + 21,
        },
    )
    .unwrap();
    let completed = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            problem_id: problem_ids[10].clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered.into_fsrs(),
            duration_ms: 100,
            occurred_at_utc_ms: now + 22,
        },
    )
    .unwrap();
    assert!(
        completed.focus.is_none(),
        "a completed session has no trailing focus round"
    );
    let completed_row: (String, Option<String>) = connection
        .query_row(
            "SELECT status, focus_order_json FROM review_sessions ORDER BY created_at_utc_ms DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(completed_row, ("completed".to_owned(), None));
}

#[test]
fn exam_sessions_explicitly_ignore_focus_preferences() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    save_review_preferences(
        &connection,
        "account-1",
        &profile_id,
        SaveReviewPreferences {
            focus_policy: ReviewFocusPolicy::SessionStart,
        },
        500,
    )
    .unwrap();

    let exam = start_exam_review_queue(
        &mut connection,
        StartExamReview {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_ids: vec![problem_id],
            now_utc_ms: 1_700_500_000_000,
        },
    )
    .unwrap();

    assert!(exam.focus.is_none());
    let policy: String = connection
        .query_row(
            "SELECT focus_policy FROM review_sessions WHERE status='active'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(policy, "off");
}

#[test]
fn quick_review_enforces_limits_due_order_filters_and_recent_again() {
    let (directory, mut connection, profile_id, first_problem_id) = create_fixture();
    let now = 1_700_000_000_000_i64;
    let mut problem_ids = vec![first_problem_id];
    for index in 0..12_i64 {
        let problem = create_problem(
            &mut connection,
            &directory.path().join("assets"),
            &[47_u8; 32],
            CreateProblem {
                account_id: "account-1".to_owned(),
                profile_id: profile_id.clone(),
                subject: if index == 11 { "物理" } else { "数学" }.to_owned(),
                note: String::new(),
                assets: vec![CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: format!("quick-question-{index}").into_bytes(),
                }],
                now_utc_ms: 300 + index,
            },
        )
        .unwrap();
        problem_ids.push(problem.id);
    }
    for problem_id in problem_ids.iter().take(11) {
        connection
            .execute(
                "UPDATE problems SET tags_json = '[\"函数\"]' WHERE id = ?1",
                [problem_id],
            )
            .unwrap();
    }
    for (problem_id, due_at) in [
        (&problem_ids[3], now - 10),
        (&problem_ids[1], now - 30),
        (&problem_ids[2], now - 20),
    ] {
        connection
            .execute(
                "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
                 VALUES(?1, ?2, 1, 5, NULL, 'fsrs-1', 'default', ?3)",
                rusqlite::params![problem_id, due_at, now - 40],
            )
            .unwrap();
    }

    let ten = start_quick_review_queue(
        &mut connection,
        StartQuickReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_id.clone(),
            preset: QuickReviewPreset::TenProblems,
            subject: Some("数学".to_owned()),
            tag: Some("函数".to_owned()),
            now_utc_ms: now,
        },
    )
    .expect("ten-problem quick review");
    assert_eq!(ten.total_count, 10);
    assert_eq!(
        ten.items
            .iter()
            .take(3)
            .map(|item| item.problem_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            problem_ids[1].as_str(),
            problem_ids[2].as_str(),
            problem_ids[3].as_str(),
        ]
    );

    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
             VALUES('quick-again', 'account-1', ?1, ?2, 'device', 'again', 500, ?3, 'fsrs-1', 'default')",
            rusqlite::params![profile_id, problem_ids[4], now - 1_000],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
             VALUES('quick-old-again', 'account-1', ?1, ?2, 'device', 'again', 500, ?3, 'fsrs-1', 'default')",
            rusqlite::params![profile_id, problem_ids[5], now - 31 * 86_400_000],
        )
        .unwrap();
    let recent = start_quick_review_queue(
        &mut connection,
        StartQuickReview {
            account_id: "account-1".to_owned(),
            profile_id,
            preset: QuickReviewPreset::RecentlyForgotten,
            subject: None,
            tag: None,
            now_utc_ms: now,
        },
    )
    .expect("recently forgotten quick review");
    assert_eq!(recent.total_count, 1);
    assert_eq!(recent.items[0].problem_id, problem_ids[4]);
}
