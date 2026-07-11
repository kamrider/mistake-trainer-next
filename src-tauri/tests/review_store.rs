use mistake_trainer_next_lib::{
    domain::review::SimpleRating,
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        problems::{AssetRole, CaptureAsset, CreateProblem, create_problem},
        profiles::{CreateProfile, create_profile},
        review::{ReviewUseCaseError, SubmitReview, submit_review},
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
fn remembered_review_commits_event_schedule_and_outbox_together() {
    let (_directory, mut connection, profile_id, problem_id) = create_fixture();
    let now = 1_700_000_000_000_i64;

    let result = submit_review(
        &mut connection,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id,
            problem_id: problem_id.clone(),
            device_id: "device-1".to_owned(),
            rating: SimpleRating::Remembered,
            duration_ms: 2_400,
            occurred_at_utc_ms: now,
        },
    )
    .expect("submit review");

    assert_eq!(result.rating, "good");
    assert!(result.due_at_utc_ms > now);
    assert_eq!(result.algorithm_version, "fsrs-6.6.1");
    let counts: (i64, i64, i64) = connection.query_row(
        "SELECT (SELECT count(*) FROM review_events WHERE problem_id = ?1), (SELECT count(*) FROM schedule_states WHERE problem_id = ?1), (SELECT count(*) FROM sync_operations WHERE entity_type = 'review_event' AND entity_id = ?2)",
        [&problem_id, &result.event_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(counts, (1, 1, 1));
}

#[test]
fn identical_event_history_produces_identical_schedule_state() {
    let (_directory_a, mut a, profile_a, problem_a) = create_fixture();
    let (_directory_b, mut b, profile_b, problem_b) = create_fixture();
    let now = 1_700_000_000_000_i64;

    let first = submit_review(
        &mut a,
        SubmitReview {
            account_id: "account-1".to_owned(),
            profile_id: profile_a,
            problem_id: problem_a,
            device_id: "device-a".to_owned(),
            rating: SimpleRating::Remembered,
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
            rating: SimpleRating::Remembered,
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
            rating: SimpleRating::Forgot,
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
