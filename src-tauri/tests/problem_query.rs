use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use mistake_trainer_next_lib::{
    application::ports::assets::{AssetDecryptionError, AssetDecryptor},
    infrastructure::{
        assets::KeyedAssetDecryptor,
        database::{open_encrypted_database, run_migrations},
    },
    modules::{
        problems::{
            AssetRole, CaptureAsset, CreateProblem, ProblemAnswerState, ProblemFilterOptionsQuery,
            ProblemListInput, ProblemListQuery, ProblemReviewState, ProblemStatusFilter,
            create_problem, list_problem_filter_options, list_problem_summaries,
            list_problem_summaries_with_previews,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

const PAGE_SIZE: usize = 40;

struct PanicAssetDecryptor;

impl AssetDecryptor for PanicAssetDecryptor {
    fn decrypt(&self, _encrypted: &[u8]) -> Result<Vec<u8>, AssetDecryptionError> {
        panic!("malformed cursors must be rejected before preview decryption")
    }
}

#[test]
fn list_pages_with_stable_keyset_order_without_duplicates_or_gaps() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-page-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let other_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "另一个档案".to_owned(),
            now_utc_ms: 11,
        },
    )
    .expect("other profile");
    let other_account_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "其他账号".to_owned(),
            now_utc_ms: 12,
        },
    )
    .expect("other account profile");
    let mut expected = Vec::new();
    for index in 0..45 {
        let updated_at = 100 + i64::from(index / 3);
        let id = create_fixture_problem(
            &mut connection,
            directory.path(),
            "account-1",
            &profile.id,
            &format!("科目{index}"),
            updated_at,
        );
        expected.push((updated_at, id));
    }
    for index in 0..6 {
        let updated_at = 100 + i64::from(index * 2);
        create_fixture_problem(
            &mut connection,
            directory.path(),
            "account-1",
            &other_profile.id,
            &format!("其他档案{index}"),
            updated_at,
        );
        create_fixture_problem(
            &mut connection,
            directory.path(),
            "account-2",
            &other_account_profile.id,
            &format!("其他账号{index}"),
            updated_at,
        );
    }
    expected.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    let first = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id.clone(),
            ProblemStatusFilter::Active,
            None,
        ),
    )
    .expect("first page");
    assert_eq!(first.items.len(), PAGE_SIZE);
    assert!(first.next_cursor.is_some());

    let mut second_query = basic_query(
        "account-1",
        profile.id.clone(),
        ProblemStatusFilter::Active,
        None,
    );
    second_query.input.cursor = first.next_cursor.clone();
    let second = list_problem_summaries(&connection, second_query).expect("second page");
    assert_eq!(second.items.len(), 5);
    assert!(second.next_cursor.is_none());

    let actual = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|problem| problem.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(_, id)| id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        actual
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        45
    );

    let filter_options = list_problem_filter_options(
        &connection,
        ProblemFilterOptionsQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            status: ProblemStatusFilter::Active,
        },
    )
    .expect("complete filter options");
    assert_eq!(filter_options.subjects.len(), 45);
    assert!(filter_options.subjects.contains(&"科目0".to_owned()));
    assert!(filter_options.subjects.contains(&"科目44".to_owned()));
    assert!(
        filter_options
            .subjects
            .iter()
            .all(|subject| !subject.starts_with("其他"))
    );
}

#[test]
fn list_rejects_a_malformed_cursor() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-cursor-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let mut query = basic_query("account-1", profile.id, ProblemStatusFilter::Active, None);
    query.input.cursor = Some("not-a-valid-cursor".to_owned());

    assert!(matches!(
        list_problem_summaries_with_previews(
            &connection,
            &directory.path().join("assets"),
            &PanicAssetDecryptor,
            query,
        ),
        Err(mistake_trainer_next_lib::modules::problems::ProblemUseCaseError::InvalidQuery)
    ));
}

#[test]
fn filter_options_are_complete_status_and_profile_scoped() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-facets-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let selected = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("selected profile");
    let other = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "其他人".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("other profile");

    let math = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &selected.id,
        "数学",
        100,
    );
    let chemistry = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &selected.id,
        "化学",
        90,
    );
    let archived = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &selected.id,
        "英语",
        80,
    );
    let foreign = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-2",
        &other.id,
        "物理",
        95,
    );
    for (id, tags_json) in [
        (&math, r#"["基础"]"#),
        (&chemistry, r#"["压轴","基础"]"#),
        (&archived, r#"["归档标签"]"#),
        (&foreign, r#"["外部标签"]"#),
    ] {
        connection
            .execute(
                "UPDATE problems SET tags_json = ?2 WHERE id = ?1",
                rusqlite::params![id, tags_json],
            )
            .unwrap();
    }
    connection
        .execute(
            "UPDATE problems SET status = 'archived' WHERE id = ?1",
            [&archived],
        )
        .unwrap();

    let active = list_problem_filter_options(
        &connection,
        ProblemFilterOptionsQuery {
            account_id: "account-1".to_owned(),
            profile_id: selected.id.clone(),
            status: ProblemStatusFilter::Active,
        },
    )
    .expect("active filter options");
    assert_eq!(active.subjects.len(), 2);
    assert!(active.subjects.contains(&"数学".to_owned()));
    assert!(active.subjects.contains(&"化学".to_owned()));
    assert_eq!(active.tags.len(), 2);
    assert!(active.tags.contains(&"基础".to_owned()));
    assert!(active.tags.contains(&"压轴".to_owned()));
    assert!(!active.subjects.contains(&"物理".to_owned()));
    assert!(!active.tags.contains(&"外部标签".to_owned()));

    let archived_options = list_problem_filter_options(
        &connection,
        ProblemFilterOptionsQuery {
            account_id: "account-1".to_owned(),
            profile_id: selected.id,
            status: ProblemStatusFilter::Archived,
        },
    )
    .expect("archived filter options");
    assert_eq!(archived_options.subjects, vec!["英语"]);
    assert_eq!(archived_options.tags, vec!["归档标签"]);
}

#[test]
fn list_returns_only_the_selected_account_profile_with_asset_counts() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-query-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let selected = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("selected profile");
    let other = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-2".to_owned(),
            name: "其他人".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("other profile");

    create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &selected.id,
        "数学",
        20,
    );
    create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-2",
        &other.id,
        "英语",
        30,
    );

    let summaries = list_problem_summaries(
        &connection,
        basic_query("account-1", selected.id, ProblemStatusFilter::Active, None),
    )
    .expect("list problems");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].subject, "数学");
    assert_eq!(summaries[0].question_asset_count, 1);
    assert_eq!(summaries[0].answer_asset_count, 1);
    assert_eq!(summaries[0].status, "active");
}

#[test]
fn list_status_filter_separates_active_and_archived_problems() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-query-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let active = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "数学",
        20,
    );
    let archived = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "物理",
        30,
    );
    connection
        .execute(
            "UPDATE problems SET status = 'archived' WHERE id = ?1",
            [&archived],
        )
        .expect("archive fixture");

    let active_rows = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id.clone(),
            ProblemStatusFilter::Active,
            None,
        ),
    )
    .expect("active list");
    let archived_rows = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id.clone(),
            ProblemStatusFilter::Archived,
            None,
        ),
    )
    .expect("archived list");

    assert_eq!(
        active_rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![active]
    );
    assert_eq!(
        archived_rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![archived]
    );
}

#[test]
fn list_search_matches_subject_or_note_without_treating_wildcards_as_patterns() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-search-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let math = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "数学",
        20,
    );
    connection
        .execute(
            "UPDATE problems SET note = '奇函数定义域', tags_json = '[\"函数\",\"粗心\"]' WHERE id = ?1",
            [&math],
        )
        .expect("note fixture");
    create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "物理_实验",
        30,
    );

    let by_note = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id.clone(),
            ProblemStatusFilter::Active,
            Some("定义域".to_owned()),
        ),
    )
    .expect("search by note");
    let literal_wildcard = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id.clone(),
            ProblemStatusFilter::Active,
            Some("_".to_owned()),
        ),
    )
    .expect("literal wildcard search");
    let by_tag = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id,
            ProblemStatusFilter::Active,
            Some("粗心".to_owned()),
        ),
    )
    .expect("search by tag");

    assert_eq!(
        by_note
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![math.clone()]
    );
    assert_eq!(literal_wildcard.len(), 1);
    assert_eq!(literal_wildcard[0].subject, "物理_实验");
    assert_eq!(by_tag.len(), 1);
    assert_eq!(by_tag[0].id, math);
    assert_eq!(by_tag[0].tags, vec!["函数", "粗心"]);
}

#[test]
fn advanced_filters_compose_without_crossing_profile_or_review_boundaries() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-filter-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let other_profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "另一档案".to_owned(),
            now_utc_ms: 11,
        },
    )
    .expect("other profile");
    let math = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "数学",
        20,
    );
    let physics = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "物理",
        30,
    );
    let english = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &profile.id,
        "英语",
        40,
    );
    let other_profile_problem = create_fixture_problem(
        &mut connection,
        directory.path(),
        "account-1",
        &other_profile.id,
        "数学",
        50,
    );
    connection
        .execute(
            "UPDATE problems SET tags_json = '[\"函数\",\"错因·计算失误\"]' WHERE id = ?1",
            [&math],
        )
        .expect("tag math");
    connection
        .execute(
            "UPDATE problems SET tags_json = '[\"错因·计算失误\"]' WHERE id = ?1",
            [&other_profile_problem],
        )
        .expect("tag other profile");
    connection
        .execute(
            "DELETE FROM problem_assets WHERE problem_id = ?1 AND role = 'answer'",
            [&physics],
        )
        .expect("remove physics answer");
    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
             VALUES('review-math', 'account-1', ?1, ?2, 'device', 'again', 1200, 900, 'fsrs-1', 'default')",
            rusqlite::params![profile.id, math],
        )
        .expect("review math");
    connection
        .execute(
            "INSERT INTO review_events(id, account_id, profile_id, problem_id, device_id, rating, duration_ms, occurred_at_utc_ms, algorithm_version, parameter_version)
             VALUES('review-other', 'account-1', ?1, ?2, 'device', 'again', 900, 999, 'fsrs-1', 'default')",
            rusqlite::params![other_profile.id, other_profile_problem],
        )
        .expect("review other profile");
    connection
        .execute(
            "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
             VALUES(?1, 950, 1, 5, 900, 'fsrs-1', 'default', 900)",
            [&math],
        )
        .expect("due math");
    connection
        .execute(
            "INSERT INTO schedule_states(problem_id, due_at_utc_ms, stability, difficulty, last_reviewed_at_utc_ms, algorithm_version, parameter_version, rebuilt_at_utc_ms)
             VALUES(?1, 2000, 1, 5, NULL, 'fsrs-1', 'default', 900)",
            [&english],
        )
        .expect("future english");

    let by_reason = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            now_utc_ms: 1_000,
            input: ProblemListInput {
                status: ProblemStatusFilter::Active,
                search: None,
                subjects: vec![],
                tags: vec!["错因·计算失误".to_owned()],
                review_state: ProblemReviewState::RecentlyForgotten,
                answer_state: ProblemAnswerState::Any,
                cursor: None,
            },
        },
    )
    .expect("reason filter");
    let missing_never_reviewed_physics = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            now_utc_ms: 1_000,
            input: ProblemListInput {
                status: ProblemStatusFilter::Active,
                search: None,
                subjects: vec!["物理".to_owned()],
                tags: vec![],
                review_state: ProblemReviewState::NeverReviewed,
                answer_state: ProblemAnswerState::MissingAnswer,
                cursor: None,
            },
        },
    )
    .expect("missing answer filter");
    let due = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            now_utc_ms: 1_000,
            input: ProblemListInput {
                status: ProblemStatusFilter::Active,
                search: None,
                subjects: vec![],
                tags: vec![],
                review_state: ProblemReviewState::Due,
                answer_state: ProblemAnswerState::Any,
                cursor: None,
            },
        },
    )
    .expect("due filter");
    let subject_or = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            now_utc_ms: 1_000,
            input: ProblemListInput {
                status: ProblemStatusFilter::Active,
                search: None,
                subjects: vec!["数学".to_owned(), "物理".to_owned()],
                tags: vec![],
                review_state: ProblemReviewState::Any,
                answer_state: ProblemAnswerState::Any,
                cursor: None,
            },
        },
    )
    .expect("subject OR filter");
    let has_answer = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            now_utc_ms: 1_000,
            input: ProblemListInput {
                status: ProblemStatusFilter::Active,
                search: None,
                subjects: vec![],
                tags: vec![],
                review_state: ProblemReviewState::Any,
                answer_state: ProblemAnswerState::HasAnswer,
                cursor: None,
            },
        },
    )
    .expect("has answer filter");

    assert_eq!(
        by_reason
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![math.as_str()]
    );
    assert_eq!(
        missing_never_reviewed_physics
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![physics.as_str()]
    );
    assert_eq!(
        due.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec![math.as_str()]
    );
    assert_eq!(
        subject_or
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![physics.as_str(), math.as_str()]
    );
    assert_eq!(
        has_answer
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec![english.as_str(), math.as_str()]
    );
}

#[test]
fn list_rejects_filters_that_exceed_contract_limits() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-limit-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");

    let result = list_problem_summaries(
        &connection,
        basic_query(
            "account-1",
            profile.id,
            ProblemStatusFilter::Active,
            Some("筛".repeat(101)),
        ),
    );

    assert!(matches!(
        result,
        Err(mistake_trainer_next_lib::modules::problems::ProblemUseCaseError::InvalidQuery)
    ));
}

#[test]
fn list_with_previews_returns_a_small_question_thumbnail() {
    let directory = tempdir().expect("tempdir");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-preview-key")
            .expect("open database");
    run_migrations(&mut connection).expect("migrate database");
    let asset_key = [61_u8; 32];
    let asset_decryptor = KeyedAssetDecryptor::new(&asset_key);
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .expect("profile");
    let mut image_bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(ImageBuffer::from_pixel(800, 600, Rgba([185, 88, 63, 255])))
        .write_to(&mut image_bytes, ImageFormat::Png)
        .expect("encode fixture image");
    create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &asset_key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![
                CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: image_bytes.get_ref().clone(),
                },
                CaptureAsset {
                    role: AssetRole::Answer,
                    media_type: "image/png".to_owned(),
                    bytes: image_bytes.into_inner(),
                },
            ],
            now_utc_ms: 20,
        },
    )
    .expect("create problem");

    let summaries = list_problem_summaries_with_previews(
        &connection,
        &directory.path().join("assets"),
        &asset_decryptor,
        basic_query("account-1", profile.id, ProblemStatusFilter::Active, None),
    )
    .expect("list previews");

    let preview = summaries[0]
        .question_preview_data_url
        .as_deref()
        .expect("question preview");
    assert!(preview.starts_with("data:image/png;base64,"));
}

fn basic_query(
    account_id: &str,
    profile_id: String,
    status: ProblemStatusFilter,
    search: Option<String>,
) -> ProblemListQuery {
    ProblemListQuery {
        account_id: account_id.to_owned(),
        profile_id,
        now_utc_ms: 0,
        input: ProblemListInput {
            status,
            search,
            subjects: vec![],
            tags: vec![],
            review_state: ProblemReviewState::Any,
            answer_state: ProblemAnswerState::Any,
            cursor: None,
        },
    }
}

fn create_fixture_problem(
    connection: &mut rusqlite::Connection,
    root: &std::path::Path,
    account_id: &str,
    profile_id: &str,
    subject: &str,
    now_utc_ms: i64,
) -> String {
    create_problem(
        connection,
        &root.join("assets"),
        &[61_u8; 32],
        CreateProblem {
            account_id: account_id.to_owned(),
            profile_id: profile_id.to_owned(),
            subject: subject.to_owned(),
            note: String::new(),
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
    .expect("create fixture problem")
    .id
}
