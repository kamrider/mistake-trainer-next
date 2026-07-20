use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::preferences::{
        DEFAULT_SUBJECTS, PreferencesError, ReviewFocusPolicy, SaveReviewPreferences,
        SaveSubjectPreferences, load_review_preferences, load_subject_preferences,
        save_review_preferences, save_subject_preferences,
    },
};
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, rusqlite::Connection) {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("library.db");
    let mut connection =
        open_encrypted_database(&path, "preferences-test-key").expect("open database");
    run_migrations(&mut connection).expect("run migrations");
    for (account, profile, name) in [
        ("account-1", "profile-1", "第一档案"),
        ("account-1", "profile-2", "第二档案"),
        ("account-2", "profile-3", "其他账户"),
    ] {
        connection.execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
             VALUES(?1, ?2, ?3, 1, 1, 1)",
            (profile, account, name),
        ).expect("insert profile");
    }
    (directory, connection)
}

#[test]
fn defaults_to_the_nine_builtin_subjects_without_writing_a_row() {
    let (_directory, connection) = setup();

    let preferences =
        load_subject_preferences(&connection, "account-1", "profile-1").expect("load defaults");

    assert_eq!(preferences.enabled_subjects, DEFAULT_SUBJECTS);
    assert!(preferences.custom_subjects.is_empty());
    assert!(preferences.capture_sound_enabled);
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM profile_preferences", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn saves_normalized_unique_values_and_isolates_profiles() {
    let (_directory, connection) = setup();

    let saved = save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec![" 数学 ".into(), "数学".into(), "编程".into()],
            custom_subjects: vec![" 编程 ".into(), "编程".into(), "竞赛数学".into()],
            capture_sound_enabled: false,
        },
        100,
    )
    .expect("save preferences");

    assert_eq!(saved.enabled_subjects, vec!["数学", "编程"]);
    assert_eq!(saved.custom_subjects, vec!["编程", "竞赛数学"]);
    assert!(!saved.capture_sound_enabled);
    assert_eq!(
        load_subject_preferences(&connection, "account-1", "profile-1").unwrap(),
        saved,
    );
    assert_eq!(
        load_subject_preferences(&connection, "account-1", "profile-2")
            .unwrap()
            .enabled_subjects,
        DEFAULT_SUBJECTS,
    );
    assert_eq!(
        load_subject_preferences(&connection, "account-2", "profile-3")
            .unwrap()
            .enabled_subjects,
        DEFAULT_SUBJECTS,
    );
}

#[test]
fn rejects_empty_unknown_and_oversized_preferences() {
    let (_directory, connection) = setup();

    let empty = save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec![],
            custom_subjects: vec![],
            capture_sound_enabled: true,
        },
        1,
    );
    assert!(matches!(empty, Err(PreferencesError::InvalidInput)));

    let unknown = save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec!["不存在".into()],
            custom_subjects: vec![],
            capture_sound_enabled: true,
        },
        1,
    );
    assert!(matches!(unknown, Err(PreferencesError::InvalidInput)));

    let too_many = save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec!["数学".into()],
            custom_subjects: (0..21).map(|index| format!("自定义{index}")).collect(),
            capture_sound_enabled: true,
        },
        1,
    );
    assert!(matches!(too_many, Err(PreferencesError::InvalidInput)));

    let too_long = save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec!["数学".into()],
            custom_subjects: vec!["长".repeat(41)],
            capture_sound_enabled: true,
        },
        1,
    );
    assert!(matches!(too_long, Err(PreferencesError::InvalidInput)));
}

#[test]
fn review_focus_defaults_off_without_writing_a_row() {
    let (_directory, connection) = setup();

    let preferences =
        load_review_preferences(&connection, "account-1", "profile-1").expect("load defaults");

    assert_eq!(preferences.focus_policy, ReviewFocusPolicy::Off);
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM profile_preferences", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn review_focus_round_trips_all_policies_and_isolates_profiles() {
    let (_directory, connection) = setup();

    for (index, policy) in [
        ReviewFocusPolicy::SessionStart,
        ReviewFocusPolicy::EveryTen,
        ReviewFocusPolicy::Off,
    ]
    .into_iter()
    .enumerate()
    {
        let saved = save_review_preferences(
            &connection,
            "account-1",
            "profile-1",
            SaveReviewPreferences {
                focus_policy: policy,
            },
            100 + index as i64,
        )
        .expect("save focus preference");
        assert_eq!(saved.focus_policy, policy);
        assert_eq!(
            load_review_preferences(&connection, "account-1", "profile-1")
                .unwrap()
                .focus_policy,
            policy,
        );
    }

    assert_eq!(
        load_review_preferences(&connection, "account-1", "profile-2")
            .unwrap()
            .focus_policy,
        ReviewFocusPolicy::Off,
    );
    assert_eq!(
        load_review_preferences(&connection, "account-2", "profile-3")
            .unwrap()
            .focus_policy,
        ReviewFocusPolicy::Off,
    );
}

#[test]
fn subject_and_review_preferences_never_overwrite_each_other() {
    let (_directory, connection) = setup();

    save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec!["数学".into(), "编程".into()],
            custom_subjects: vec!["编程".into()],
            capture_sound_enabled: false,
        },
        10,
    )
    .unwrap();
    save_review_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveReviewPreferences {
            focus_policy: ReviewFocusPolicy::EveryTen,
        },
        11,
    )
    .unwrap();

    let subjects = load_subject_preferences(&connection, "account-1", "profile-1").unwrap();
    assert_eq!(subjects.enabled_subjects, vec!["数学", "编程"]);
    assert_eq!(subjects.custom_subjects, vec!["编程"]);
    assert!(!subjects.capture_sound_enabled);

    save_subject_preferences(
        &connection,
        "account-1",
        "profile-1",
        SaveSubjectPreferences {
            enabled_subjects: vec!["语文".into()],
            custom_subjects: vec![],
            capture_sound_enabled: true,
        },
        12,
    )
    .unwrap();
    assert_eq!(
        load_review_preferences(&connection, "account-1", "profile-1")
            .unwrap()
            .focus_policy,
        ReviewFocusPolicy::EveryTen,
    );
}

#[test]
fn review_preferences_reject_missing_or_cross_account_profiles() {
    let (_directory, connection) = setup();

    for result in [
        load_review_preferences(&connection, "account-1", "missing"),
        load_review_preferences(&connection, "account-2", "profile-1"),
    ] {
        assert!(matches!(result, Err(PreferencesError::ProfileNotFound)));
    }
    assert!(matches!(
        save_review_preferences(
            &connection,
            "account-2",
            "profile-1",
            SaveReviewPreferences {
                focus_policy: ReviewFocusPolicy::SessionStart,
            },
            1,
        ),
        Err(PreferencesError::ProfileNotFound)
    ));
}
