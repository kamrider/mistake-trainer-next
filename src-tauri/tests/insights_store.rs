use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        insights::{report_summary, settings_overview},
        problems::{AssetRole, CaptureAsset, CreateProblem, create_problem},
        profiles::{CreateProfile, create_profile},
    },
};
use rusqlite::params;
use tempfile::tempdir;

#[test]
fn report_and_settings_are_profile_scoped_and_derived_from_real_events() {
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
        &[9_u8; 32],
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
    let other_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "另一个档案".to_owned(),
            now_utc_ms: 11,
        },
    )
    .unwrap();
    let now = 1_700_000_000_000_i64;
    connection.execute(
        "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
         VALUES('event-1', 'account-1', ?1, ?2, 'device-1', 'good', 1200, ?3, 'fsrs-6', 'default')",
        params![profile.id, problem.id, now],
    ).unwrap();
    connection.execute(
        "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
         VALUES('event-corrupt-scope', 'account-1', ?1, ?2, 'device-2', 'hard', 900, ?3, 'fsrs-6', 'default')",
        params![other_profile.id, problem.id, now],
    ).unwrap();

    let report = report_summary(&connection, "account-1", &profile.id, now).unwrap();
    assert_eq!(report.active_problem_count, 1);
    assert_eq!(report.due_problem_count, 1);
    assert_eq!(report.review_count, 1);
    assert_eq!(report.remembered_rate, 1.0);
    assert_eq!(report.total_duration_ms, 1_200.0);
    assert_eq!(report.current_streak_days, 1);
    assert_eq!(report.daily_activity.len(), 14);
    assert_eq!(
        report
            .daily_activity
            .iter()
            .map(|day| day.review_count)
            .sum::<i32>(),
        1
    );
    assert_eq!(report.subject_activity[0].subject, "数学");
    assert_eq!(report.subject_activity[0].review_count, 1);

    let settings = settings_overview(&connection, "account-1", &profile.id).unwrap();
    assert_eq!(settings.active_problem_count, 1);
    assert!(settings.pending_operation_count >= 1);
    assert!(settings.local_encryption_ready);
    assert!(!settings.cloud_sync_configured);
}
