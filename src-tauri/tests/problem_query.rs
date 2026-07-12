use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        problems::{
            AssetRole, CaptureAsset, CreateProblem, ProblemListQuery, ProblemStatusFilter,
            create_problem, list_problem_summaries,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

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
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: selected.id,
            status: ProblemStatusFilter::Active,
        },
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
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            status: ProblemStatusFilter::Active,
        },
    )
    .expect("active list");
    let archived_rows = list_problem_summaries(
        &connection,
        ProblemListQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            status: ProblemStatusFilter::Archived,
        },
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
