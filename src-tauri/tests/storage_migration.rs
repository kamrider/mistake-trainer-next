use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use mistake_trainer_next_lib::{
    application::result::AppResult,
    application::startup::{
        LibraryStartup, StartupAccessUnavailable, initialize_application_library,
        initialize_configured_application_library_if_accessible,
    },
    commands::storage::{StorageLocationKind, storage_status_for},
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        runtime::{LibraryRuntime, SecretStore, initialize_local_library},
        storage_location::{
            STORAGE_PENDING_FILE, STORAGE_POINTER_FILE, resolve_storage, write_storage_pointer,
        },
    },
    modules::{
        backup::{create_backup, prepare_backup_restore, schedule_backup_restore, validate_backup},
        storage_migration::{
            StorageMigrationOutcome, apply_pending_storage_migration,
            read_storage_migration_receipt, stage_storage_migration,
        },
    },
};
use rusqlite::params;
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

const DATABASE_KEY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const ASSET_KEY: [u8; 32] = [7_u8; 32];
const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const DEVICE_ID: &str = "0191365e-2f2f-7b89-b3b0-333333333333";
const PROBLEM_ID: &str = "0191365e-2f2f-7b89-b3b0-444444444444";

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

struct MigrationFixture {
    _control: TempDir,
    control_root: PathBuf,
    source_root: PathBuf,
    runtime: LibraryRuntime,
}

fn fixture() -> MigrationFixture {
    let control = tempdir().unwrap();
    let control_root = control.path().to_path_buf();
    let source_root = control_root.join("library");
    let runtime = initialize_local_library(&source_root, &FixedSecrets, 100).unwrap();
    let profile_id = runtime.active_profile().id;
    fs::create_dir_all(runtime.blob_root.join("aa")).unwrap();

    let question = b"encrypted migration question";
    let answer = b"encrypted migration answer";
    fs::write(
        runtime.blob_root.join("aa/question.enc"),
        encrypt_asset(question, &ASSET_KEY).unwrap(),
    )
    .unwrap();
    fs::write(
        runtime.blob_root.join("aa/answer.enc"),
        encrypt_asset(answer, &ASSET_KEY).unwrap(),
    )
    .unwrap();
    fs::create_dir_all(runtime.blob_root.join("cache")).unwrap();
    fs::write(
        runtime.blob_root.join("cache/unreferenced.enc"),
        b"must not migrate",
    )
    .unwrap();

    let connection = runtime.connection.lock().unwrap();
    connection
        .execute(
            "INSERT INTO problems(
                id, account_id, profile_id, subject, tags_json, note, status,
                created_at_utc_ms, updated_at_utc_ms
             ) VALUES(?1, ?2, ?3, '数学', '[]', '', 'active', 100, 100)",
            params![PROBLEM_ID, ACCOUNT_ID, profile_id],
        )
        .unwrap();
    for (id, plaintext, encrypted_path) in [
        ("asset-question", question.as_slice(), "aa/question.enc"),
        ("asset-answer", answer.as_slice(), "aa/answer.enc"),
    ] {
        connection
            .execute(
                "INSERT INTO assets(
                    id, account_id, plaintext_sha256, encrypted_path,
                    byte_length, media_type, created_at_utc_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, 'image/jpeg', 100)",
                params![
                    id,
                    ACCOUNT_ID,
                    plaintext_sha256(plaintext),
                    encrypted_path,
                    plaintext.len() as i64
                ],
            )
            .unwrap();
    }
    connection
        .execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position)
             VALUES(?1, 'asset-question', 'question', 0)",
            [PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position)
             VALUES(?1, 'asset-answer', 'answer', 0)",
            [PROBLEM_ID],
        )
        .unwrap();
    drop(connection);

    MigrationFixture {
        _control: control,
        control_root,
        source_root,
        runtime,
    }
}

fn product_root(parent: &Path) -> PathBuf {
    parent.join("Mistake Trainer Next Data")
}

fn destination_library(parent: &Path) -> PathBuf {
    product_root(parent).join("library")
}

fn referenced_paths(runtime: &LibraryRuntime) -> HashSet<String> {
    let connection = runtime.connection.lock().unwrap();
    let mut statement = connection
        .prepare("SELECT encrypted_path FROM assets ORDER BY encrypted_path")
        .unwrap();
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<HashSet<_>, _>>()
        .unwrap()
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn storage_status_reports_bounded_sizes_and_pending_state_without_a_path() {
    let fixture = fixture();
    let expected_asset_bytes = fs::metadata(fixture.runtime.blob_root.join("aa/question.enc"))
        .unwrap()
        .len()
        + fs::metadata(fixture.runtime.blob_root.join("aa/answer.enc"))
            .unwrap()
            .len();

    let AppResult::Success { data, .. } =
        storage_status_for(&fixture.runtime, &fixture.control_root)
    else {
        panic!("default storage status should load")
    };
    assert_eq!(data.kind, StorageLocationKind::Default);
    assert_eq!(data.location_label, "默认位置 · Windows 应用数据");
    assert!(data.database_bytes > 0.0);
    assert_eq!(data.asset_bytes, expected_asset_bytes as f64);
    assert!(!data.migration_pending);
    let serialized = serde_json::to_string(&data).unwrap();
    assert!(!serialized.contains(&fixture.control_root.to_string_lossy().to_string()));

    let destination = tempdir().unwrap();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    let AppResult::Success { data, .. } =
        storage_status_for(&fixture.runtime, &fixture.control_root)
    else {
        panic!("status should remain available while restart is pending")
    };
    assert!(data.migration_pending);
}

#[test]
fn rejects_current_nested_control_and_unowned_nonempty_destinations() {
    let fixture = fixture();

    assert!(
        stage_storage_migration(
            &fixture.runtime,
            &fixture.control_root,
            &fixture.source_root,
            200
        )
        .is_err()
    );

    let nonempty = tempdir().unwrap();
    fs::write(nonempty.path().join("foreign.txt"), b"not ours").unwrap();
    assert!(
        stage_storage_migration(
            &fixture.runtime,
            &fixture.control_root,
            nonempty.path(),
            201
        )
        .is_err()
    );
    assert!(nonempty.path().join("foreign.txt").is_file());
    assert!(!product_root(nonempty.path()).exists());
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
}

#[test]
fn rejects_a_source_asset_reparse_point_before_copying() {
    let fixture = fixture();
    let external = tempdir().unwrap();
    fs::write(
        external.path().join("question.enc"),
        fs::read(fixture.runtime.blob_root.join("aa/question.enc")).unwrap(),
    )
    .unwrap();
    let linked = fixture.runtime.blob_root.join("linked");
    create_directory_link(&linked, external.path());
    fixture
        .runtime
        .connection
        .lock()
        .unwrap()
        .execute(
            "UPDATE assets SET encrypted_path = 'linked/question.enc'
             WHERE id = 'asset-question'",
            [],
        )
        .unwrap();
    let destination = tempdir().unwrap();

    let result = stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    );

    assert!(result.is_err());
    assert!(!product_root(destination.path()).exists());
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
    remove_directory_link(&linked);
}

#[test]
fn snapshot_copies_only_database_referenced_encrypted_assets() {
    let fixture = fixture();
    let destination = tempdir().unwrap();

    let receipt = stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();

    assert_eq!(receipt.outcome, StorageMigrationOutcome::Scheduled);
    assert_eq!(receipt.copied_asset_count, 2);
    let destination_root = destination_library(destination.path());
    assert!(destination_root.join("library.db").is_file());
    for relative in referenced_paths(&fixture.runtime) {
        assert!(destination_root.join("assets").join(relative).is_file());
    }
    assert!(
        !destination_root
            .join("assets/cache/unreferenced.enc")
            .exists()
    );
    assert!(fixture.source_root.join("library.db").is_file());
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());
    assert!(fixture.control_root.join(STORAGE_PENDING_FILE).is_file());
}

#[test]
fn copy_failure_leaves_source_and_pointer_unchanged() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    let database_before = sha256(&fixture.source_root.join("library.db"));
    let first_asset_before = sha256(&fixture.runtime.blob_root.join("aa/answer.enc"));
    fs::remove_file(fixture.runtime.blob_root.join("aa/question.enc")).unwrap();

    let result = stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    );

    assert!(result.is_err());
    assert_eq!(
        sha256(&fixture.source_root.join("library.db")),
        database_before
    );
    assert_eq!(
        sha256(&fixture.runtime.blob_root.join("aa/answer.enc")),
        first_asset_before
    );
    assert!(!product_root(destination.path()).exists());
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
}

#[test]
fn rejects_a_snapshot_containing_foreign_account_rows() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    fixture
        .runtime
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO learner_profiles(
                id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES('foreign-profile', 'foreign-account', '不应迁移', 100, 100, 1)",
            [],
        )
        .unwrap();

    let result = stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    );

    assert!(result.is_err());
    assert!(fixture.source_root.join("library.db").is_file());
    assert!(!product_root(destination.path()).exists());
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
}

#[test]
fn expired_pending_migration_fails_closed_without_touching_either_library() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    drop(fixture.runtime);

    let startup = initialize_configured_application_library_if_accessible(
        &fixture.control_root,
        &FixedSecrets,
        200 + 24 * 60 * 60 * 1_000 + 1,
    )
    .unwrap();

    assert!(matches!(
        startup,
        LibraryStartup::AccessUnavailable(StartupAccessUnavailable::StorageMigration(_))
    ));
    assert!(fixture.source_root.join("library.db").is_file());
    assert!(
        destination_library(destination.path())
            .join("library.db")
            .is_file()
    );
    assert!(fixture.control_root.join(STORAGE_PENDING_FILE).is_file());
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());
}

#[test]
fn startup_recovers_when_pointer_commit_precedes_journal_removal() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    let expected_profile = fixture.runtime.active_profile();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    drop(fixture.runtime);
    let destination_root = destination_library(destination.path())
        .canonicalize()
        .unwrap();
    write_storage_pointer(&fixture.control_root, &destination_root).unwrap();
    assert!(fixture.control_root.join(STORAGE_PENDING_FILE).is_file());

    let startup = initialize_configured_application_library_if_accessible(
        &fixture.control_root,
        &FixedSecrets,
        201,
    )
    .unwrap();
    let LibraryStartup::Ready(runtime) = startup else {
        panic!("committed pointer plus pending journal must resume idempotently")
    };

    assert_eq!(runtime.active_profile(), expected_profile);
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
    assert!(!fixture.source_root.exists());
    assert_eq!(
        resolve_storage(&fixture.control_root)
            .unwrap()
            .library_root()
            .canonicalize()
            .unwrap(),
        destination_root
    );
}

#[test]
fn restart_commits_pointer_only_after_destination_opens() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    let original_profile = fixture.runtime.active_profile();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    drop(fixture.runtime);
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());

    let moved = apply_pending_storage_migration(&fixture.control_root, &FixedSecrets, 201)
        .unwrap()
        .expect("pending migration should produce the active runtime");

    assert_eq!(moved.active_profile(), original_profile);
    let problem_count: i64 = moved
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM problems WHERE id = ?1 AND account_id = ?2",
            params![PROBLEM_ID, ACCOUNT_ID],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(problem_count, 1);
    assert_eq!(referenced_paths(&moved).len(), 2);
    assert_eq!(
        resolve_storage(&fixture.control_root)
            .unwrap()
            .library_root()
            .canonicalize()
            .unwrap(),
        destination_library(destination.path())
            .canonicalize()
            .unwrap()
    );
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
    assert!(!fixture.source_root.exists());
    assert_eq!(
        read_storage_migration_receipt(&fixture.control_root)
            .unwrap()
            .unwrap()
            .outcome,
        StorageMigrationOutcome::Moved
    );
}

#[test]
fn tampered_destination_rolls_back_to_source_and_records_receipt() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    let original_profile = fixture.runtime.active_profile();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    drop(fixture.runtime);
    fs::write(
        destination_library(destination.path()).join("assets/aa/question.enc"),
        b"tampered",
    )
    .unwrap();

    let reopened = apply_pending_storage_migration(&fixture.control_root, &FixedSecrets, 201)
        .unwrap()
        .expect("rollback should reopen the untouched source");

    assert_eq!(reopened.active_profile(), original_profile);
    assert_eq!(
        reopened
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert!(!fixture.control_root.join(STORAGE_POINTER_FILE).exists());
    assert!(!fixture.control_root.join(STORAGE_PENDING_FILE).exists());
    assert!(!product_root(destination.path()).exists());
    assert!(fixture.source_root.join("library.db").is_file());
    assert_eq!(
        read_storage_migration_receipt(&fixture.control_root)
            .unwrap()
            .unwrap()
            .outcome,
        StorageMigrationOutcome::RolledBack
    );
}

#[test]
fn committed_custom_storage_keeps_backup_restore_controls_beside_that_library() {
    let fixture = fixture();
    let destination = tempdir().unwrap();
    let backup_destination = tempdir().unwrap();
    stage_storage_migration(
        &fixture.runtime,
        &fixture.control_root,
        destination.path(),
        200,
    )
    .unwrap();
    drop(fixture.runtime);
    let runtime = apply_pending_storage_migration(&fixture.control_root, &FixedSecrets, 201)
        .unwrap()
        .unwrap();
    let custom_library_root = runtime.blob_root.parent().unwrap().to_path_buf();
    let custom_product_root = custom_library_root.parent().unwrap().to_path_buf();

    let summary = create_backup(
        &runtime.connection,
        &runtime.blob_root,
        runtime.database_key(),
        runtime.account_id(),
        backup_destination.path(),
        300,
    )
    .unwrap();
    let package = backup_destination.path().join(&summary.label);
    assert!(
        validate_backup(
            &package,
            runtime.database_key(),
            &runtime.asset_key,
            runtime.account_id()
        )
        .is_ok()
    );
    let candidate = prepare_backup_restore(
        &package,
        &custom_product_root,
        runtime.database_key(),
        &runtime.asset_key,
        runtime.account_id(),
        301,
    )
    .unwrap();
    schedule_backup_restore(
        &custom_product_root,
        &candidate.id,
        runtime.database_key(),
        &runtime.asset_key,
        runtime.account_id(),
        302,
    )
    .unwrap();

    assert!(custom_product_root.join("restore-pending.json").is_file());
    assert!(!fixture.control_root.join("restore-pending.json").exists());
    drop(runtime);
    let restored =
        initialize_application_library(&custom_library_root, &FixedSecrets, 303).unwrap();

    assert_eq!(
        restored
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert!(custom_product_root.join("restore-receipt.json").is_file());
    assert!(!custom_product_root.join("restore-pending.json").exists());
    assert_eq!(
        resolve_storage(&fixture.control_root)
            .unwrap()
            .library_root()
            .canonicalize()
            .unwrap(),
        custom_library_root.canonicalize().unwrap()
    );
}

#[cfg(windows)]
fn create_directory_link(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "test junction could not be created");
}

#[cfg(windows)]
fn remove_directory_link(link: &Path) {
    fs::remove_dir(link).unwrap();
}

#[cfg(unix)]
fn create_directory_link(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(unix)]
fn remove_directory_link(link: &Path) {
    fs::remove_file(link).unwrap();
}
