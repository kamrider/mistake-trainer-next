use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        insights::{InsightsError, dashboard_overview, report_summary, settings_overview},
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

    let report = report_summary(&connection, "account-1", &profile.id, now, 0).unwrap();
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

#[test]
fn dashboard_is_profile_scoped_uses_local_days_and_real_capture_backlog() {
    const DAY_MS: i64 = 86_400_000;
    const HOUR_MS: i64 = 3_600_000;

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
            name: "另一个档案".to_owned(),
            now_utc_ms: 11,
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

    let offset_minutes = 8 * 60;
    let offset_ms = i64::from(offset_minutes) * 60_000;
    let today_bucket = 20_000_i64;
    let today_start_utc_ms = today_bucket * DAY_MS - offset_ms;
    let now_utc_ms = today_start_utc_ms + 12 * HOUR_MS;

    for (id, profile_id, rating, occurred_at_utc_ms) in [
        (
            "event-today",
            profile.id.as_str(),
            "good",
            today_start_utc_ms + HOUR_MS,
        ),
        (
            "event-yesterday",
            profile.id.as_str(),
            "again",
            today_start_utc_ms - DAY_MS + HOUR_MS,
        ),
        (
            "event-29-days-ago",
            profile.id.as_str(),
            "easy",
            today_start_utc_ms - 29 * DAY_MS + HOUR_MS,
        ),
        (
            "event-31-days-ago",
            profile.id.as_str(),
            "easy",
            today_start_utc_ms - 31 * DAY_MS + HOUR_MS,
        ),
        (
            "event-other-profile",
            other_profile.id.as_str(),
            "good",
            today_start_utc_ms + HOUR_MS,
        ),
    ] {
        connection
            .execute(
                "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
                 VALUES(?1, 'account-1', ?2, ?3, 'device-1', ?4, 800, ?5, 'fsrs-6', 'default')",
                params![id, profile_id, problem.id, rating, occurred_at_utc_ms],
            )
            .unwrap();
    }

    let asset_id: String = connection
        .query_row(
            "SELECT asset_id FROM problem_assets WHERE problem_id = ?1 LIMIT 1",
            [&problem.id],
            |row| row.get(0),
        )
        .unwrap();
    for (id, batch_profile_id, state) in [
        ("batch-collecting", profile.id.as_str(), "collecting"),
        ("batch-organizing", profile.id.as_str(), "organizing"),
        ("batch-completed", profile.id.as_str(), "completed"),
        (
            "batch-other-profile",
            other_profile.id.as_str(),
            "organizing",
        ),
    ] {
        connection
            .execute(
                "INSERT INTO capture_batches(id, account_id, profile_id, subject, state, revision, created_at_utc_ms, updated_at_utc_ms)
                 VALUES(?1, 'account-1', ?2, '数学', ?3, 1, ?4, ?4)",
                params![id, batch_profile_id, state, now_utc_ms],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO capture_items(id, batch_id, asset_id, client_upload_id, source_name, source_sequence, width, height, created_at_utc_ms)
                 VALUES(?1 || '-item', ?1, ?2, ?1 || '-upload', 'photo.jpg', 0, 100, 100, ?3)",
                params![id, asset_id, now_utc_ms],
            )
            .unwrap();
    }

    connection
        .execute(
            "INSERT INTO profile_preferences(
                 account_id, profile_id, enabled_subjects_json, custom_subjects_json,
                 capture_sound_enabled, updated_at_utc_ms, daily_review_target,
                 daily_minutes_target
             ) VALUES('account-1', ?1, '[\"数学\"]', '[]', 1, ?2, 3, 25)",
            params![profile.id, now_utc_ms],
        )
        .unwrap();

    let overview = dashboard_overview(
        &connection,
        "account-1",
        &profile.id,
        now_utc_ms,
        offset_minutes,
    )
    .unwrap();
    assert_eq!(overview.profile_name, "小树");
    assert_eq!(overview.active_problem_count, 1);
    assert_eq!(overview.due_problem_count, 1);
    assert_eq!(overview.reviewed_today_count, 1);
    assert_eq!(overview.remembered_rate_30_days, Some(2.0 / 3.0));
    assert_eq!(overview.current_streak_days, 2);
    assert_eq!(overview.pending_capture_batch_count, 2);
    assert_eq!(overview.pending_capture_item_count, 2);
    assert_eq!(overview.daily_plan.review_target, 3);
    assert_eq!(overview.daily_plan.minutes_target, 25);
    assert_eq!(overview.daily_plan.completed_reviews, 1);
    assert_eq!(overview.daily_plan.remaining_reviews, 2);
    assert_eq!(overview.daily_plan.due_reviews, 1);
    assert_eq!(overview.daily_plan.suggested_reviews, 2);
    assert_eq!(overview.daily_plan.estimated_minutes, 1);

    assert!(matches!(
        dashboard_overview(&connection, "account-1", &profile.id, now_utc_ms, 841),
        Err(InsightsError::InvalidTimezoneOffset)
    ));
}

#[test]
fn report_surfaces_evidence_based_weak_areas_and_seven_local_due_days() {
    const DAY_MS: i64 = 86_400_000;
    const HOUR_MS: i64 = 3_600_000;
    let directory = tempdir().unwrap();
    let mut connection = open_encrypted_database(
        &directory.path().join("library.db"),
        "insights-forecast-key",
    )
    .unwrap();
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
    let math = create_problem(
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
                bytes: b"weak-math".to_vec(),
            }],
            now_utc_ms: 20,
        },
    )
    .unwrap();
    let physics = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[9_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "物理".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"weak-physics".to_vec(),
            }],
            now_utc_ms: 21,
        },
    )
    .unwrap();
    let future = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[9_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "英语".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"weak-future".to_vec(),
            }],
            now_utc_ms: 22,
        },
    )
    .unwrap();
    connection
        .execute(
            "UPDATE problems SET tags_json = '[\"错因·计算失误\"]' WHERE id = ?1",
            [&math.id],
        )
        .unwrap();

    let offset_minutes = 8 * 60;
    let today_start_utc_ms = DAY_MS - 8 * HOUR_MS;
    let now_utc_ms = today_start_utc_ms + 12 * HOUR_MS;
    for (id, problem_id, rating, duration, age) in [
        ("math-1", math.id.as_str(), "again", 1_000, 5),
        ("math-2", math.id.as_str(), "again", 2_000, 4),
        ("math-3", math.id.as_str(), "good", 3_000, 3),
        ("physics-1", physics.id.as_str(), "again", 4_000, 2),
        ("physics-2", physics.id.as_str(), "good", 6_000, 1),
    ] {
        connection
            .execute(
                "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
                 VALUES(?1, 'account-1', ?2, ?3, 'device', ?4, ?5, ?6, 'fsrs-6', 'default')",
                params![id, profile.id, problem_id, rating, duration, now_utc_ms - age * HOUR_MS],
            )
            .unwrap();
    }
    for (problem_id, due_at) in [
        (math.id.as_str(), today_start_utc_ms - HOUR_MS),
        (physics.id.as_str(), today_start_utc_ms + HOUR_MS),
        (future.id.as_str(), today_start_utc_ms + DAY_MS + HOUR_MS),
    ] {
        connection
            .execute(
                "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
                 VALUES(?1, ?2, 1, 5, NULL, 'fsrs-6', 'default', ?3)",
                params![problem_id, due_at, now_utc_ms],
            )
            .unwrap();
    }
    let other = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &[9_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: other_profile.id,
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: b"weak-other".to_vec(),
            }],
            now_utc_ms: 23,
        },
    )
    .unwrap();
    connection
        .execute(
            "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
             VALUES(?1, ?2, 1, 5, NULL, 'fsrs-6', 'default', ?3)",
            params![other.id, today_start_utc_ms + HOUR_MS, now_utc_ms],
        )
        .unwrap();

    let report = report_summary(
        &connection,
        "account-1",
        &profile.id,
        now_utc_ms,
        offset_minutes,
    )
    .unwrap();

    assert_eq!(report.weak_areas.len(), 3);
    assert_eq!(report.weak_areas[0].lapse_rate, 2.0 / 3.0);
    let reason = report
        .weak_areas
        .iter()
        .find(|area| area.kind == "reason")
        .expect("reason weakness");
    assert_eq!(reason.label, "错因·计算失误");
    assert_eq!(reason.reviewed_count, 3);
    assert_eq!(reason.lapse_count, 2);
    assert_eq!(reason.average_duration_ms, 2_000.0);
    assert_eq!(report.due_forecast.len(), 7);
    assert_eq!(report.due_forecast[0].local_date, "1970-01-02");
    assert_eq!(report.due_forecast[0].due_count, 1);
    assert_eq!(report.due_forecast[0].overdue_count, 1);
    assert_eq!(report.due_forecast[1].local_date, "1970-01-03");
    assert_eq!(report.due_forecast[1].due_count, 1);
    assert!(
        report.due_forecast[1..]
            .iter()
            .all(|day| day.overdue_count == 0)
    );
}
