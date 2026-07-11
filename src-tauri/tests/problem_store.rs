use mistake_trainer_next_lib::{
    infrastructure::{
        assets::decrypt_asset,
        database::{open_encrypted_database, run_migrations},
    },
    modules::{
        problems::{AssetRole, CaptureAsset, CreateProblem, ProblemUseCaseError, create_problem},
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

fn capture(role: AssetRole, bytes: &[u8]) -> CaptureAsset {
    CaptureAsset {
        role,
        media_type: "image/png".to_owned(),
        bytes: bytes.to_vec(),
    }
}

#[test]
fn problem_assets_and_outbox_are_committed_as_one_aggregate() {
    let directory = tempdir().expect("temp directory");
    let database_path = directory.path().join("library.db");
    let blob_root = directory.path().join("assets");
    let key = [31_u8; 32];
    let mut connection = open_encrypted_database(&database_path, "problem-key").unwrap();
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
        &blob_root,
        &key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id,
            subject: "数学".to_owned(),
            note: "奇函数".to_owned(),
            assets: vec![
                capture(AssetRole::Question, b"question-image"),
                capture(AssetRole::Answer, b"answer-image"),
            ],
            now_utc_ms: 200,
        },
    )
    .expect("create problem");

    let counts: (i64, i64, i64) = connection.query_row(
        "SELECT (SELECT count(*) FROM problems), (SELECT count(*) FROM assets), (SELECT count(*) FROM problem_assets)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(counts, (1, 2, 2));
    assert_eq!(problem.asset_ids.len(), 2);

    for asset_id in &problem.asset_ids {
        let relative: String = connection
            .query_row(
                "SELECT encrypted_path FROM assets WHERE id = ?1",
                [asset_id],
                |row| row.get(0),
            )
            .unwrap();
        let encrypted = std::fs::read(blob_root.join(relative)).expect("read encrypted blob");
        let plaintext = decrypt_asset(&encrypted, &key).expect("decrypt stored blob");
        assert!(plaintext == b"question-image" || plaintext == b"answer-image");
    }

    let problem_outbox: i64 = connection
        .query_row(
            "SELECT count(*) FROM sync_operations WHERE entity_type = 'problem' AND entity_id = ?1",
            [&problem.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(problem_outbox, 1);
}

#[test]
fn duplicate_plaintext_reuses_one_asset_row_and_blob() {
    let directory = tempdir().unwrap();
    let blob_root = directory.path().join("assets");
    let key = [37_u8; 32];
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-key").unwrap();
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

    for now in [200, 300] {
        create_problem(
            &mut connection,
            &blob_root,
            &key,
            CreateProblem {
                account_id: "account-1".to_owned(),
                profile_id: profile.id.clone(),
                subject: "数学".to_owned(),
                note: String::new(),
                assets: vec![capture(AssetRole::Question, b"shared-image")],
                now_utc_ms: now,
            },
        )
        .unwrap();
    }

    let assets: i64 = connection
        .query_row("SELECT count(*) FROM assets", [], |row| row.get(0))
        .unwrap();
    let links: i64 = connection
        .query_row("SELECT count(*) FROM problem_assets", [], |row| row.get(0))
        .unwrap();
    assert_eq!((assets, links), (1, 2));
}

#[test]
fn unknown_profile_leaves_database_and_blob_root_unchanged() {
    let directory = tempdir().unwrap();
    let blob_root = directory.path().join("assets");
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "problem-key").unwrap();
    run_migrations(&mut connection).unwrap();

    let error = create_problem(
        &mut connection,
        &blob_root,
        &[41_u8; 32],
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: "missing-profile".to_owned(),
            subject: "数学".to_owned(),
            note: String::new(),
            assets: vec![capture(AssetRole::Question, b"must-not-write")],
            now_utc_ms: 200,
        },
    )
    .expect_err("unknown profile must fail");

    assert!(matches!(error, ProblemUseCaseError::ProfileNotFound));
    let problems: i64 = connection
        .query_row("SELECT count(*) FROM problems", [], |row| row.get(0))
        .unwrap();
    assert_eq!(problems, 0);
    assert!(!blob_root.exists());
}
