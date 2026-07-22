use mistake_trainer_next_lib::{
    domain::review::FsrsRating,
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::review_history::{
        ReviewHistoryDetailQuery, ReviewHistoryError, ReviewHistoryQuery, ReviewHistoryRange,
        get_review_history_detail, list_review_history,
    },
};
use rusqlite::{Connection, params};
use tempfile::tempdir;

const ACCOUNT: &str = "account-a";
const PROFILE: &str = "profile-a";
const NOW: i64 = 2_000_000_000_000;
const DAY: i64 = 86_400_000;

fn fixture() -> (tempfile::TempDir, Connection) {
    let directory = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "history-key").unwrap();
    run_migrations(&mut connection).unwrap();
    for (id, account) in [
        (PROFILE, ACCOUNT),
        ("profile-b", ACCOUNT),
        ("profile-x", "account-x"),
    ] {
        connection.execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, ?2, ?1, 1, 1, 1)",
            params![id, account],
        ).unwrap();
    }
    (directory, connection)
}

fn problem(
    connection: &Connection,
    id: &str,
    account: &str,
    profile: &str,
    subject: &str,
    note: &str,
    status: &str,
) {
    connection.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, status, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 1, 1)",
        params![id, account, profile, subject, note, status],
    ).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn event(
    connection: &Connection,
    id: &str,
    account: &str,
    profile: &str,
    problem_id: &str,
    device: &str,
    rating: &str,
    occurred: i64,
    algorithm: &str,
    parameters: &str,
) {
    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating,
             duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1250, ?7, ?8, ?9)",
            params![
                id, account, profile, problem_id, device, rating, occurred, algorithm, parameters
            ],
        )
        .unwrap();
}

fn query() -> ReviewHistoryQuery {
    ReviewHistoryQuery {
        account_id: ACCOUNT.to_owned(),
        profile_id: PROFILE.to_owned(),
        range: ReviewHistoryRange::All,
        rating: None,
        subject: None,
        search: String::new(),
        cursor: None,
        limit: 2,
        now_utc_ms: NOW,
    }
}

#[test]
fn list_is_deterministic_paginated_filtered_and_profile_scoped() {
    let (_directory, connection) = fixture();
    problem(
        &connection,
        "p-a",
        ACCOUNT,
        PROFILE,
        "数学",
        "百分比 100%_完成",
        "active",
    );
    problem(
        &connection,
        "p-b",
        ACCOUNT,
        PROFILE,
        "英语",
        "archived history stays visible",
        "archived",
    );
    problem(
        &connection,
        "p-c",
        ACCOUNT,
        "profile-b",
        "数学",
        "other profile",
        "active",
    );
    problem(
        &connection,
        "p-x",
        "account-x",
        "profile-x",
        "数学",
        "other account",
        "active",
    );
    event(
        &connection,
        "event-a",
        ACCOUNT,
        PROFILE,
        "p-a",
        "device-a",
        "good",
        NOW - DAY,
        "fsrs-6.6.1",
        "default-6.6.1",
    );
    event(
        &connection,
        "event-b",
        ACCOUNT,
        PROFILE,
        "p-b",
        "device-b",
        "again",
        NOW - DAY,
        "fsrs-5",
        "legacy",
    );
    event(
        &connection,
        "event-c",
        ACCOUNT,
        PROFILE,
        "p-a",
        "device-a",
        "hard",
        NOW - 10 * DAY,
        "fsrs-6.6.1",
        "default-6.6.1",
    );
    event(
        &connection,
        "foreign-profile",
        ACCOUNT,
        "profile-b",
        "p-c",
        "device-a",
        "easy",
        NOW,
        "fsrs-6.6.1",
        "default-6.6.1",
    );
    event(
        &connection,
        "foreign-account",
        "account-x",
        "profile-x",
        "p-x",
        "device-a",
        "easy",
        NOW,
        "fsrs-6.6.1",
        "default-6.6.1",
    );

    let first = list_review_history(&connection, query()).unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.event_id.as_str())
            .collect::<Vec<_>>(),
        ["event-b", "event-a"]
    );
    assert_eq!(first.total_count, 3);
    assert_eq!(first.available_subjects, ["数学", "英语"]);
    assert!(first.items[0].problem_status == "archived");
    let second = list_review_history(
        &connection,
        ReviewHistoryQuery {
            cursor: first.next_cursor,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| item.event_id.as_str())
            .collect::<Vec<_>>(),
        ["event-c"]
    );
    assert!(second.next_cursor.is_none());

    let seven_days = list_review_history(
        &connection,
        ReviewHistoryQuery {
            range: ReviewHistoryRange::SevenDays,
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(seven_days.total_count, 2);
    let thirty_days = list_review_history(
        &connection,
        ReviewHistoryQuery {
            range: ReviewHistoryRange::ThirtyDays,
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(thirty_days.total_count, 3);
    let rating = list_review_history(
        &connection,
        ReviewHistoryQuery {
            rating: Some(FsrsRating::Again),
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(
        rating
            .items
            .iter()
            .map(|item| item.event_id.as_str())
            .collect::<Vec<_>>(),
        ["event-b"]
    );
    let subject = list_review_history(
        &connection,
        ReviewHistoryQuery {
            subject: Some(" 数学 ".to_owned()),
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(subject.total_count, 2);
    let literal = list_review_history(
        &connection,
        ReviewHistoryQuery {
            search: "%_".to_owned(),
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert_eq!(
        literal.total_count, 2,
        "both reviews of only the literal-wildcard problem match"
    );
}

#[test]
fn list_rejects_unbounded_or_malformed_inputs() {
    let (_directory, connection) = fixture();
    for invalid in [
        ReviewHistoryQuery {
            limit: 0,
            ..query()
        },
        ReviewHistoryQuery {
            limit: 51,
            ..query()
        },
        ReviewHistoryQuery {
            search: "字".repeat(81),
            ..query()
        },
        ReviewHistoryQuery {
            subject: Some(String::new()),
            ..query()
        },
        ReviewHistoryQuery {
            subject: Some("字".repeat(41)),
            ..query()
        },
    ] {
        assert!(matches!(
            list_review_history(&connection, invalid),
            Err(ReviewHistoryError::InvalidQuery)
        ));
    }
    assert!(matches!(
        list_review_history(
            &connection,
            ReviewHistoryQuery {
                cursor: Some("not-a-cursor".to_owned()),
                ..query()
            }
        ),
        Err(ReviewHistoryError::InvalidCursor)
    ));
    assert!(matches!(
        list_review_history(
            &connection,
            ReviewHistoryQuery {
                cursor: Some("a".repeat(513)),
                ..query()
            }
        ),
        Err(ReviewHistoryError::InvalidCursor)
    ));

    let padded_search = list_review_history(
        &connection,
        ReviewHistoryQuery {
            search: format!(" {} ", "x".repeat(80)),
            ..query()
        },
    );
    assert!(
        padded_search.is_ok(),
        "search length is bounded after trimming"
    );
}

#[test]
fn detail_is_scoped_and_exposes_audit_facts_without_raw_device_id() {
    let (_directory, connection) = fixture();
    problem(
        &connection,
        "p-a",
        ACCOUNT,
        PROFILE,
        "物理",
        "完整笔记",
        "archived",
    );
    problem(
        &connection,
        "p-b",
        ACCOUNT,
        "profile-b",
        "物理",
        "foreign",
        "active",
    );
    event(
        &connection,
        "event-1",
        ACCOUNT,
        PROFILE,
        "p-a",
        "device-current",
        "again",
        NOW - 2,
        "fsrs-5",
        "legacy",
    );
    event(
        &connection,
        "event-2",
        ACCOUNT,
        PROFILE,
        "p-a",
        "device-other",
        "good",
        NOW - 1,
        "fsrs-6.6.1",
        "default-6.6.1",
    );
    event(
        &connection,
        "event-foreign",
        ACCOUNT,
        "profile-b",
        "p-b",
        "device-current",
        "easy",
        NOW,
        "fsrs-6.6.1",
        "default-6.6.1",
    );
    connection
        .execute(
            "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty,
             last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
         VALUES('p-a', ?1, 4.5, 3.25, ?2, 'fsrs-6.6.1', 'default-6.6.1', ?2)",
            params![NOW + DAY, NOW - 1],
        )
        .unwrap();

    let detail = get_review_history_detail(
        &connection,
        ReviewHistoryDetailQuery {
            account_id: ACCOUNT.to_owned(),
            profile_id: PROFILE.to_owned(),
            event_id: "event-1".to_owned(),
            current_device_id: "device-current".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(detail.note, "完整笔记");
    assert_eq!(detail.review_ordinal, 1);
    assert_eq!(detail.problem_review_count, 2);
    assert!(detail.is_current_device);
    assert!(!detail.algorithm_is_current);
    assert!(!detail.parameters_are_current);
    let schedule = detail.current_schedule.unwrap();
    assert_eq!(schedule.due_at_utc_ms, (NOW + DAY) as f64);
    assert_eq!(schedule.stability, 4.5);
    assert_eq!(schedule.difficulty, 3.25);

    let current_detail = get_review_history_detail(
        &connection,
        ReviewHistoryDetailQuery {
            account_id: ACCOUNT.to_owned(),
            profile_id: PROFILE.to_owned(),
            event_id: "event-2".to_owned(),
            current_device_id: "device-current".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(current_detail.review_ordinal, 2);
    assert!(!current_detail.is_current_device);
    assert!(current_detail.algorithm_is_current);
    assert!(current_detail.parameters_are_current);

    for event_id in ["event-foreign", "missing"] {
        assert!(matches!(
            get_review_history_detail(
                &connection,
                ReviewHistoryDetailQuery {
                    account_id: ACCOUNT.to_owned(),
                    profile_id: PROFILE.to_owned(),
                    event_id: event_id.to_owned(),
                    current_device_id: "device-current".to_owned(),
                }
            ),
            Err(ReviewHistoryError::NotFound)
        ));
    }
    assert!(matches!(
        get_review_history_detail(
            &connection,
            ReviewHistoryDetailQuery {
                account_id: ACCOUNT.to_owned(),
                profile_id: PROFILE.to_owned(),
                event_id: String::new(),
                current_device_id: "device-current".to_owned(),
            }
        ),
        Err(ReviewHistoryError::InvalidQuery)
    ));
}

#[test]
fn corrupt_cross_profile_problem_links_are_never_exposed() {
    let (_directory, connection) = fixture();
    problem(
        &connection,
        "p-foreign",
        "account-x",
        "profile-x",
        "private subject",
        "private note",
        "active",
    );
    event(
        &connection,
        "event-cross-link",
        ACCOUNT,
        PROFILE,
        "p-foreign",
        "device-current",
        "good",
        NOW,
        "fsrs-6.6.1",
        "default-6.6.1",
    );

    let page = list_review_history(
        &connection,
        ReviewHistoryQuery {
            limit: 20,
            ..query()
        },
    )
    .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.total_count, 0);
    assert!(page.available_subjects.is_empty());

    assert!(matches!(
        get_review_history_detail(
            &connection,
            ReviewHistoryDetailQuery {
                account_id: ACCOUNT.to_owned(),
                profile_id: PROFILE.to_owned(),
                event_id: "event-cross-link".to_owned(),
                current_device_id: "device-current".to_owned(),
            }
        ),
        Err(ReviewHistoryError::NotFound)
    ));
}
