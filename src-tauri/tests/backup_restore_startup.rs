use std::{fs, sync::Mutex};

use mistake_trainer_next_lib::{
    application::startup::initialize_application_library,
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        database::{open_encrypted_database, run_migrations},
        runtime::SecretStore,
    },
    modules::backup::{
        BackupError, begin_pending_restore, create_backup, prepare_backup_restore,
        schedule_backup_restore, take_restore_receipt, validate_backup,
    },
};
use rusqlite::params;
use serde_json::Value;
use tempfile::{TempDir, tempdir};

const DATABASE_KEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const ASSET_KEY: [u8; 32] = [7_u8; 32];
const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";
const DEVICE_ID: &str = "0191365e-2f2f-7b89-b3b0-333333333333";

struct FixedSecrets;

impl SecretStore for FixedSecrets {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(match name {
            "database-key" => Some(DATABASE_KEY.to_owned()),
            "asset-key" => Some("07".repeat(32)),
            "account-id" => Some(ACCOUNT_ID.to_owned()),
            "device-id" => Some(DEVICE_ID.to_owned()),
            _ => None,
        })
    }

    fn set(&self, _name: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }
}

struct RestoreFixture {
    _source: TempDir,
    _destination: TempDir,
    application_root: TempDir,
    candidate_id: String,
}

fn fixture() -> RestoreFixture {
    let source = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let application_root = tempdir().unwrap();
    let source_database = source.path().join("library.db");
    let mut connection = open_encrypted_database(&source_database, DATABASE_KEY).unwrap();
    run_migrations(&mut connection).unwrap();
    connection
        .execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
             VALUES(?1, ?2, '恢复后的档案', 1, 1)",
            params![PROFILE_ID, ACCOUNT_ID],
        )
        .unwrap();
    let blob_root = source.path().join("assets");
    fs::create_dir_all(blob_root.join("aa")).unwrap();
    let plaintext = b"restored question";
    let encrypted = encrypt_asset(plaintext, &ASSET_KEY).unwrap();
    fs::write(blob_root.join("aa/question.enc"), encrypted).unwrap();
    connection
        .execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES('asset-1', ?1, ?2, 'aa/question.enc', ?3, 'image/png', 1)",
            params![ACCOUNT_ID, plaintext_sha256(plaintext), plaintext.len() as i64],
        )
        .unwrap();
    let connection = Mutex::new(connection);
    let summary = create_backup(
        &connection,
        &blob_root,
        DATABASE_KEY,
        ACCOUNT_ID,
        destination.path(),
        100,
    )
    .unwrap();
    let package = destination.path().join(summary.label);
    let candidate = prepare_backup_restore(
        &package,
        application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        200,
    )
    .unwrap();
    let live = application_root.path().join("library");
    fs::create_dir(&live).unwrap();
    fs::write(live.join("old-library.txt"), b"keep old library").unwrap();
    schedule_backup_restore(
        application_root.path(),
        &candidate.id,
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        201,
    )
    .unwrap();
    RestoreFixture {
        _source: source,
        _destination: destination,
        application_root,
        candidate_id: candidate.id,
    }
}

fn control_paths(fixture: &RestoreFixture) -> (std::path::PathBuf, std::path::PathBuf) {
    let marker: Value = serde_json::from_slice(
        &fs::read(fixture.application_root.path().join("restore-pending.json")).unwrap(),
    )
    .unwrap();
    let rollback_id = marker["rollbackId"].as_str().unwrap();
    (
        fixture
            .application_root
            .path()
            .join(format!(".mistake-trainer-restore-{}", fixture.candidate_id)),
        fixture
            .application_root
            .path()
            .join(format!(".mistake-trainer-rollback-{rollback_id}")),
    )
}

#[test]
fn normal_startup_swap_commits_only_after_the_restored_package_is_ready() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");
    let swap = begin_pending_restore(
        fixture.application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        202,
    )
    .unwrap()
    .expect("pending swap");

    assert!(!live.join("old-library.txt").exists());
    assert!(live.join("library.db").is_file());
    assert!(validate_backup(&live, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).is_ok());
    let receipt = swap.commit(203).unwrap();
    assert_eq!(receipt.status, "succeeded");
    assert!(
        !fixture
            .application_root
            .path()
            .join("restore-pending.json")
            .exists()
    );
    assert_eq!(
        take_restore_receipt(fixture.application_root.path())
            .unwrap()
            .unwrap(),
        receipt
    );
}

#[test]
fn application_startup_applies_the_pending_restore_before_opening_sqlcipher() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");

    let runtime = initialize_application_library(&live, &FixedSecrets, 202).unwrap();

    assert_eq!(runtime.active_profile().id, PROFILE_ID);
    assert_eq!(runtime.active_profile().name, "恢复后的档案");
    assert!(
        !fixture
            .application_root
            .path()
            .join("restore-pending.json")
            .exists()
    );
    let receipt = take_restore_receipt(fixture.application_root.path())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, "succeeded");
}

#[test]
fn failed_restored_runtime_can_roll_back_the_exact_old_library() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");
    let swap = begin_pending_restore(
        fixture.application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        202,
    )
    .unwrap()
    .unwrap();

    let receipt = swap.rollback(203).unwrap();
    assert_eq!(receipt.status, "rolled_back");
    assert_eq!(
        fs::read(live.join("old-library.txt")).unwrap(),
        b"keep old library"
    );
    assert!(!live.join("manifest.json").exists());
}

#[test]
fn startup_recovers_after_each_directory_rename_boundary() {
    for post_swap in [false, true] {
        let fixture = fixture();
        let live = fixture.application_root.path().join("library");
        let (stage, rollback) = control_paths(&fixture);
        fs::rename(&live, &rollback).unwrap();
        if post_swap {
            fs::rename(&stage, &live).unwrap();
        }

        let swap = begin_pending_restore(
            fixture.application_root.path(),
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            202,
        )
        .unwrap()
        .expect("recovered swap");
        assert!(live.join("library.db").is_file());
        swap.rollback(203).unwrap();
        assert!(live.join("old-library.txt").is_file());
    }
}

#[test]
fn tampered_stage_never_moves_the_current_library() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");
    let (stage, _) = control_paths(&fixture);
    fs::write(stage.join("assets/aa/question.enc"), b"tampered").unwrap();

    assert!(matches!(
        begin_pending_restore(
            fixture.application_root.path(),
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            202,
        ),
        Err(BackupError::Integrity)
    ));
    assert_eq!(
        fs::read(live.join("old-library.txt")).unwrap(),
        b"keep old library"
    );
}

#[test]
fn forged_pending_label_is_rejected_before_the_first_directory_move() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");
    let marker_path = fixture.application_root.path().join("restore-pending.json");
    let mut marker: Value = serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["label"] = Value::String("forged-label".to_owned());
    fs::write(&marker_path, serde_json::to_vec_pretty(&marker).unwrap()).unwrap();

    assert!(matches!(
        begin_pending_restore(
            fixture.application_root.path(),
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            202,
        ),
        Err(BackupError::Integrity)
    ));
    assert_eq!(
        fs::read(live.join("old-library.txt")).unwrap(),
        b"keep old library"
    );
    let (stage, rollback) = control_paths(&fixture);
    assert!(stage.is_dir());
    assert!(!rollback.exists());
}

#[test]
fn corrupted_restored_library_after_a_crash_is_rolled_back_before_opening() {
    let fixture = fixture();
    let live = fixture.application_root.path().join("library");
    let (stage, rollback) = control_paths(&fixture);
    fs::rename(&live, &rollback).unwrap();
    fs::rename(&stage, &live).unwrap();
    fs::write(live.join("assets/aa/question.enc"), b"corrupted after swap").unwrap();

    let result = begin_pending_restore(
        fixture.application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        202,
    )
    .unwrap();

    assert!(result.is_none());
    assert_eq!(
        fs::read(live.join("old-library.txt")).unwrap(),
        b"keep old library"
    );
    let receipt = take_restore_receipt(fixture.application_root.path())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, "rolled_back");
}
