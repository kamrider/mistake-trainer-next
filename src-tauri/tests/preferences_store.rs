use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::preferences::{
        DEFAULT_SUBJECTS, PreferencesError, SaveSubjectPreferences, load_subject_preferences,
        save_subject_preferences,
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
