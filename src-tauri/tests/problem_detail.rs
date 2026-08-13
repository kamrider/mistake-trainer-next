use mistake_trainer_next_lib::{
    infrastructure::{
        assets::KeyedAssetDecryptor,
        database::{open_encrypted_database, run_migrations},
    },
    modules::{
        problems::{
            AssetRole, CaptureAsset, CreateProblem, ProblemDetailQuery, ProblemUseCaseError,
            create_problem, get_problem_detail,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

const VALID_PNG: &[u8] = include_bytes!("../icons/32x32.png");

#[test]
fn detail_returns_ordered_safe_image_previews_for_the_selected_profile() {
    let directory = tempdir().expect("tempdir");
    let blob_root = directory.path().join("assets");
    let key = [73_u8; 32];
    let asset_decryptor = KeyedAssetDecryptor::new(&key);
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-detail-key")
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
    let problem = create_problem(
        &mut connection,
        &blob_root,
        &key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: "先看定义域".to_owned(),
            assets: vec![
                CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: VALID_PNG.to_vec(),
                },
                CaptureAsset {
                    role: AssetRole::Answer,
                    media_type: "image/png".to_owned(),
                    bytes: VALID_PNG.to_vec(),
                },
            ],
            now_utc_ms: 20,
        },
    )
    .expect("problem");
    connection
        .execute(
            "UPDATE problems SET time_limit_seconds = 90, tags_json = '[\"函数\",\"粗心\"]' WHERE id = ?1",
            [&problem.id],
        )
        .expect("set time limit");

    let detail = get_problem_detail(
        &connection,
        &blob_root,
        &asset_decryptor,
        ProblemDetailQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            problem_id: problem.id.clone(),
        },
    )
    .expect("detail");

    assert_eq!(detail.id, problem.id);
    assert_eq!(detail.subject, "数学");
    assert_eq!(detail.tags, vec!["函数", "粗心"]);
    assert_eq!(detail.time_limit_seconds, Some(90));
    assert_eq!(detail.assets.len(), 2);
    assert_eq!(detail.assets[0].role, "question");
    assert_eq!(detail.assets[1].role, "answer");
    assert!(
        detail.assets[0]
            .data_url
            .starts_with("data:image/png;base64,")
    );

    let error = get_problem_detail(
        &connection,
        &blob_root,
        &asset_decryptor,
        ProblemDetailQuery {
            account_id: "other-account".to_owned(),
            profile_id: profile.id,
            problem_id: problem.id,
        },
    )
    .expect_err("cross-account detail must fail");
    assert!(matches!(error, ProblemUseCaseError::ProblemNotFound));
}

#[test]
fn detail_rejects_a_tampered_asset_path_before_reading_it() {
    let directory = tempdir().expect("tempdir");
    let blob_root = directory.path().join("assets");
    let key = [79_u8; 32];
    let asset_decryptor = KeyedAssetDecryptor::new(&key);
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-path-key")
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
    let problem = create_problem(
        &mut connection,
        &blob_root,
        &key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![CaptureAsset {
                role: AssetRole::Question,
                media_type: "image/png".to_owned(),
                bytes: VALID_PNG.to_vec(),
            }],
            now_utc_ms: 20,
        },
    )
    .expect("problem");
    connection
        .execute(
            "UPDATE assets SET encrypted_path = '../library.db' WHERE id = ?1",
            [&problem.asset_ids[0]],
        )
        .expect("tamper path");

    let error = get_problem_detail(
        &connection,
        &blob_root,
        &asset_decryptor,
        ProblemDetailQuery {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            problem_id: problem.id,
        },
    )
    .expect_err("path traversal must fail");
    assert!(matches!(error, ProblemUseCaseError::InvalidAssetPath));
}
