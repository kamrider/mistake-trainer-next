use std::{fs, io::Cursor};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use mistake_trainer_next_lib::{
    infrastructure::{
        assets::{decrypt_asset, encrypt_asset, plaintext_sha256},
        database::{open_encrypted_database, run_migrations},
    },
    modules::legacy::{
        LegacyImportError, build_legacy_import_plan, import_legacy_plan, legacy_tree_fingerprint,
        rollback_legacy_import,
    },
};
use rusqlite::params;
use tempfile::{TempDir, tempdir};

const ACCOUNT: &str = "0191365e-2f2f-7b89-b3b0-aaaaaaaaaaaa";
const KEY: [u8; 32] = [41; 32];

struct Fixture {
    root: TempDir,
    source: TempDir,
    blob_root: std::path::PathBuf,
    connection: rusqlite::Connection,
    question: Vec<u8>,
}

fn png(seed: u8) -> Vec<u8> {
    let image = RgbaImage::from_pixel(3, 2, Rgba([seed, seed / 2, 240, 255]));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn fixture() -> Fixture {
    let root = tempdir().unwrap();
    let source = tempdir().unwrap();
    let blob_root = root.path().join("assets");
    fs::create_dir_all(&blob_root).unwrap();
    let mut connection =
        open_encrypted_database(&root.path().join("library.db"), "legacy-import-test").unwrap();
    run_migrations(&mut connection).unwrap();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
         VALUES('existing-profile', ?1, 'alice', 1, 1)",
        [ACCOUNT],
    ).unwrap();

    let member = source.path().join("members").join("alice");
    let files = member.join("files");
    fs::create_dir_all(&files).unwrap();
    let question = png(20);
    fs::write(files.join("question.png"), &question).unwrap();
    fs::write(files.join("answer.png"), png(80)).unwrap();
    fs::write(files.join("solo.png"), png(140)).unwrap();
    fs::write(
        member.join(".metadata.json"),
        r#"{
          "version":"1.1",
          "files":{
            "q":{"id":"q","relativePath":"question.png","type":"mistake","pairId":"pair-1","subject":"数学","tags":["函数"],"notes":"保留步骤","answerTimeLimit":120,"proficiency":50,"trainingInterval":8,"nextTrainingDate":"2026-07-25T00:00:00.000Z","isFrozen":true,"trainingRecords":[{"date":"2026-07-10T00:00:00.000Z","result":"success","answerTime":35000}]},
            "a":{"id":"a","relativePath":"answer.png","type":"answer","pairId":"pair-1"},
            "solo":{"id":"solo","relativePath":"solo.png","type":"mistake","subject":"物理"}
          }
        }"#,
    ).unwrap();

    let existing_asset_id = "existing-question-asset";
    let relative = "blobs/ex/existing-question-asset.mtb";
    fs::create_dir_all(blob_root.join("blobs/ex")).unwrap();
    fs::write(
        blob_root.join(relative),
        encrypt_asset(&question, &KEY).unwrap(),
    )
    .unwrap();
    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES(?1, ?2, ?3, ?4, ?5, 'image/png', 1)",
        params![existing_asset_id, ACCOUNT, plaintext_sha256(&question), relative, question.len() as i64],
    ).unwrap();

    Fixture {
        root,
        source,
        blob_root,
        connection,
        question,
    }
}

#[test]
fn import_is_atomic_encrypted_deduplicated_auditable_and_reversible() {
    let mut fixture = fixture();
    let before = legacy_tree_fingerprint(fixture.source.path()).unwrap();
    let plan = build_legacy_import_plan(fixture.source.path()).unwrap();
    let mut progress = Vec::new();

    let receipt = import_legacy_plan(
        &mut fixture.connection,
        &fixture.blob_root,
        &KEY,
        ACCOUNT,
        "candidate-happy",
        plan,
        1_800_000_000_000,
        |event| progress.push(event),
    )
    .unwrap();

    assert_eq!(
        legacy_tree_fingerprint(fixture.source.path()).unwrap(),
        before
    );
    assert_eq!(receipt.member_count, 1);
    assert_eq!(receipt.problem_count, 2);
    assert_eq!(receipt.asset_count, 3);
    assert_eq!(receipt.review_count, 1);
    assert_eq!(receipt.frozen_problem_count, 1);
    assert!(!progress.is_empty());
    assert_eq!(
        fixture
            .connection
            .query_row(
                "SELECT name FROM learner_profiles WHERE id <> 'existing-profile'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "alice (2)"
    );
    assert_eq!(count(&fixture.connection, "problems"), 2);
    assert_eq!(
        count(&fixture.connection, "assets"),
        3,
        "one plaintext duplicate is reused"
    );
    assert_eq!(count(&fixture.connection, "review_events"), 1);
    assert_eq!(count(&fixture.connection, "schedule_states"), 2);
    assert_eq!(count(&fixture.connection, "export_snapshots"), 1);
    assert!(count(&fixture.connection, "sync_operations") >= 7);
    let schedule: (i64, f64, f64) = fixture
        .connection
        .query_row(
            "SELECT s.due_at_utc_ms, s.stability, s.difficulty
         FROM schedule_states s JOIN problems p ON p.id = s.problem_id
         WHERE p.subject = '数学'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(schedule.0, 1_784_937_600_000);
    assert_eq!(schedule.1, 8.0);
    assert!((schedule.2 - 5.5).abs() < f64::EPSILON);
    let reused: i64 = fixture
        .connection
        .query_row(
            "SELECT created_by_import FROM legacy_import_entities
         WHERE import_id = ?1 AND entity_type = 'asset' AND entity_id = 'existing-question-asset'",
            [&receipt.import_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reused, 0);
    let imported_blob = fixture
        .connection
        .query_row(
            "SELECT encrypted_path FROM assets WHERE id <> 'existing-question-asset' LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    let encrypted = fs::read(fixture.blob_root.join(imported_blob)).unwrap();
    assert!(decrypt_asset(&encrypted, &KEY).is_ok());
    assert!(
        !encrypted
            .windows(fixture.question.len())
            .any(|window| window == fixture.question)
    );

    let duplicate = import_legacy_plan(
        &mut fixture.connection,
        &fixture.blob_root,
        &KEY,
        ACCOUNT,
        "candidate-duplicate",
        build_legacy_import_plan(fixture.source.path()).unwrap(),
        1_800_000_050_000,
        |_| {},
    );
    assert!(matches!(duplicate, Err(LegacyImportError::AlreadyImported)));

    let rollback = rollback_legacy_import(
        &mut fixture.connection,
        &fixture.blob_root,
        ACCOUNT,
        &receipt.import_id,
        1_800_000_100_000,
    )
    .unwrap();
    assert_eq!(rollback.removed_problem_count, 2);
    assert_eq!(rollback.removed_profile_count, 1);
    assert_eq!(rollback.removed_asset_count, 2);
    assert_eq!(count(&fixture.connection, "problems"), 0);
    assert_eq!(count(&fixture.connection, "assets"), 1);
    assert_eq!(count(&fixture.connection, "review_events"), 0);
    assert_eq!(
        fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sync_operations WHERE operation = 'delete'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        7,
        "every removed cloud-visible entity receives an idempotent delete"
    );
    assert_eq!(
        count(&fixture.connection, "tombstones"),
        7,
        "rollback deletions remain protected from stale remote upserts for 30 days"
    );
    assert_eq!(
        fixture
            .connection
            .query_row(
                "SELECT status FROM legacy_imports WHERE id = ?1",
                [&receipt.import_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "rolled_back"
    );
}

#[test]
fn rollback_preserves_a_changed_problem_with_its_imported_and_new_review_history() {
    let mut fixture = fixture();
    let receipt = import_legacy_plan(
        &mut fixture.connection,
        &fixture.blob_root,
        &KEY,
        ACCOUNT,
        "candidate-preserve",
        build_legacy_import_plan(fixture.source.path()).unwrap(),
        1_800_000_000_000,
        |_| {},
    )
    .unwrap();
    let (problem_id, profile_id): (String, String) = fixture
        .connection
        .query_row(
            "SELECT id, profile_id FROM problems WHERE subject = '数学'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    fixture
        .connection
        .execute(
            "UPDATE problems SET note = '迁移后补充', revision = 2 WHERE id = ?1",
            [&problem_id],
        )
        .unwrap();
    fixture
        .connection
        .execute(
            "INSERT INTO review_events(
               id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
               occurred_at_utc_ms, algorithm_version, parameter_version
             ) VALUES('new-review', ?1, ?2, ?3, 'device-local', 'good', 12000,
                      1800000050000, 'fsrs-5', 'default-0.90')",
            params![ACCOUNT, profile_id, problem_id],
        )
        .unwrap();

    let rollback = rollback_legacy_import(
        &mut fixture.connection,
        &fixture.blob_root,
        ACCOUNT,
        &receipt.import_id,
        1_800_000_100_000,
    )
    .unwrap();

    assert_eq!(rollback.removed_problem_count, 1);
    assert!(rollback.preserved_entity_count >= 4);
    assert_eq!(
        fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM review_events WHERE problem_id = ?1",
                [&problem_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2,
        "preserving a changed problem also preserves its coherent review history"
    );
    assert_eq!(
        fixture
            .connection
            .query_row(
                "SELECT note FROM problems WHERE id = ?1",
                [&problem_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "迁移后补充"
    );
}

#[test]
fn failed_final_blob_move_leaves_no_database_rows_or_temporary_assets() {
    let mut fixture = fixture();
    fs::remove_dir_all(fixture.blob_root.join("blobs")).unwrap();
    fs::write(fixture.blob_root.join("blobs"), b"blocks final directories").unwrap();
    let plan = build_legacy_import_plan(fixture.source.path()).unwrap();

    let result = import_legacy_plan(
        &mut fixture.connection,
        &fixture.blob_root,
        &KEY,
        ACCOUNT,
        "candidate-failure",
        plan,
        1_800_000_000_000,
        |_| {},
    );

    assert!(result.is_err());
    assert_eq!(count(&fixture.connection, "problems"), 0);
    assert_eq!(count(&fixture.connection, "review_events"), 0);
    assert_eq!(count(&fixture.connection, "legacy_imports"), 0);
    assert!(!fixture.blob_root.join(".legacy-import").exists());
}

#[test]
fn corrupt_image_aborts_without_importing_rows_or_leaving_staging_files() {
    let mut fixture = fixture();
    fs::write(
        fixture
            .source
            .path()
            .join("members/alice/files/answer.png"),
        b"not a decodable png",
    )
    .unwrap();
    let plan = build_legacy_import_plan(fixture.source.path()).unwrap();

    let result = import_legacy_plan(
        &mut fixture.connection,
        &fixture.blob_root,
        &KEY,
        ACCOUNT,
        "candidate-corrupt-image",
        plan,
        1_800_000_000_000,
        |_| {},
    );

    assert!(matches!(result, Err(LegacyImportError::InvalidImage)));
    assert_eq!(count(&fixture.connection, "problems"), 0);
    assert_eq!(count(&fixture.connection, "review_events"), 0);
    assert_eq!(count(&fixture.connection, "legacy_imports"), 0);
    assert!(!fixture.blob_root.join(".legacy-import").exists());
}

fn count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}
