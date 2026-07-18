use std::{fs, sync::Mutex};

use mistake_trainer_next_lib::{
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        database::{open_encrypted_database, open_encrypted_database_read_only, run_migrations},
    },
    modules::backup::{BackupError, create_backup, validate_backup},
};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

const DATABASE_KEY: &str = "backup-database-key";
const ASSET_KEY: [u8; 32] = [7_u8; 32];
const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";

struct Fixture {
    root: TempDir,
    destination: TempDir,
    connection: Mutex<Connection>,
    blob_root: std::path::PathBuf,
    encrypted_asset: Vec<u8>,
}

fn fixture() -> Fixture {
    let root = tempdir().expect("library tempdir");
    let destination = tempdir().expect("destination tempdir");
    let database_path = root.path().join("library.db");
    let mut connection =
        open_encrypted_database(&database_path, DATABASE_KEY).expect("encrypted database");
    run_migrations(&mut connection).expect("migrations");
    connection
        .execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
             VALUES(?1, ?2, '本机档案', 1, 1)",
            params![PROFILE_ID, ACCOUNT_ID],
        )
        .expect("profile");

    let blob_root = root.path().join("assets");
    fs::create_dir_all(blob_root.join("aa")).expect("blob directory");
    let plaintext = b"question image must never appear in the backup manifest";
    let encrypted_asset = encrypt_asset(plaintext, &ASSET_KEY).expect("encrypt fixture asset");
    fs::write(blob_root.join("aa/question.enc"), &encrypted_asset).expect("encrypted asset");
    connection
        .execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES('asset-1', ?1, ?2, 'aa/question.enc', ?3, 'image/png', 1)",
            params![ACCOUNT_ID, plaintext_sha256(plaintext), plaintext.len() as i64],
        )
        .expect("asset row");

    Fixture {
        root,
        destination,
        connection: Mutex::new(connection),
        blob_root,
        encrypted_asset,
    }
}

fn created_package(
    fixture: &Fixture,
) -> (
    mistake_trainer_next_lib::modules::backup::BackupSummary,
    std::path::PathBuf,
) {
    let summary = create_backup(
        &fixture.connection,
        &fixture.blob_root,
        DATABASE_KEY,
        ACCOUNT_ID,
        fixture.destination.path(),
        1_725_000_000_000,
    )
    .expect("backup succeeds");
    let package = fixture.destination.path().join(&summary.label);
    (summary, package)
}

fn refresh_database_manifest(package: &std::path::Path, schema_version: i64) {
    let database = fs::read(package.join("library.db")).unwrap();
    let manifest_path = package.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["schemaVersion"] = serde_json::json!(schema_version);
    manifest["database"]["encryptedBytes"] = serde_json::json!(database.len());
    manifest["database"]["ciphertextSha256"] =
        serde_json::json!(format!("{:x}", Sha256::digest(&database)));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn encrypted_backup_round_trips_without_leaking_identity_or_plaintext() {
    let fixture = fixture();
    {
        let connection = fixture.connection.lock().unwrap();
        connection
            .execute(
                "INSERT INTO capture_batches(
                   id, account_id, profile_id, subject, state, revision,
                   created_at_utc_ms, updated_at_utc_ms
                 ) VALUES('capture-batch-1', ?1, ?2, '数学', 'organizing', 1, 1, 1)",
                params![ACCOUNT_ID, PROFILE_ID],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO capture_drafts(
                   id, batch_id, position, subject_override, tags_json, note,
                   created_at_utc_ms, updated_at_utc_ms
                 ) VALUES('capture-draft-1', 'capture-batch-1', 0, NULL, '[]', '', 1, 1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO capture_items(
                   id, batch_id, asset_id, client_upload_id, source_sequence,
                   width, height, source_name, created_at_utc_ms
                 ) VALUES('capture-item-1', 'capture-batch-1', 'asset-1', 'client-1', 0, 100, 100, 'question.png', 1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
                 VALUES('capture-draft-1', 'capture-item-1', 'question', 0)",
                [],
            )
            .unwrap();
    }
    let source_asset_before = fs::read(fixture.blob_root.join("aa/question.enc")).unwrap();

    let (created, package) = created_package(&fixture);

    assert_eq!(created.asset_count, 1);
    assert!(!created.ready_for_restore);
    assert!(package.join("library.db").is_file());
    assert_eq!(
        fs::read(package.join("assets/aa/question.enc")).unwrap(),
        fixture.encrypted_asset
    );
    let manifest = fs::read(package.join("manifest.json")).unwrap();
    assert!(
        !manifest
            .windows(ACCOUNT_ID.len())
            .any(|value| value == ACCOUNT_ID.as_bytes())
    );
    assert!(!manifest.windows(14).any(|value| value == b"question image"));
    assert!(
        !String::from_utf8_lossy(&manifest)
            .contains(fixture.root.path().to_string_lossy().as_ref())
    );

    let plain = Connection::open(package.join("library.db")).expect("sqlite opens file handle");
    assert!(
        plain
            .query_row("SELECT count(*) FROM learner_profiles", [], |row| row
                .get::<_, i64>(0))
            .is_err(),
        "backup database must remain unreadable without the SQLCipher key"
    );
    let encrypted =
        open_encrypted_database_read_only(&package.join("library.db"), DATABASE_KEY).unwrap();
    let captured_draft_count: i64 = encrypted
        .query_row("SELECT COUNT(*) FROM capture_drafts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        captured_draft_count, 1,
        "unfinished capture drafts are backed up"
    );
    drop(encrypted);
    let validated =
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).expect("backup validates");
    assert_eq!(validated.asset_count, 1);
    assert!(validated.ready_for_restore);
    assert_eq!(
        fs::read(fixture.blob_root.join("aa/question.enc")).unwrap(),
        source_asset_before
    );
    assert!(!package.join("library.db-wal").exists());
    assert!(!package.join("library.db-shm").exists());
}

#[test]
fn validation_rejects_forged_ciphertext_manifest_and_another_account() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    let asset = package.join("assets/aa/question.enc");
    let replacement = encrypt_asset(b"forged image", &[8_u8; 32]).unwrap();
    fs::write(&asset, &replacement).unwrap();
    let manifest_path = package.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["assets"][0]["encryptedBytes"] = serde_json::json!(replacement.len());
    manifest["assets"][0]["ciphertextSha256"] =
        serde_json::json!(format!("{:x}", Sha256::digest(&replacement)));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));
    assert!(matches!(
        validate_backup(
            &package,
            DATABASE_KEY,
            &ASSET_KEY,
            "0191365e-2f2f-7b89-b3b0-999999999999"
        ),
        Err(BackupError::AccountMismatch)
    ));
}

#[test]
fn validation_rejects_manifest_paths_that_escape_the_package() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    let manifest_path = package.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let malicious = manifest.replace("assets/aa/question.enc", "../question.enc");
    fs::write(&manifest_path, malicious).unwrap();

    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::InvalidPackage)
    ));
}

#[test]
fn validation_rejects_sqlite_sidecars_and_windows_alias_paths() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    fs::write(package.join("library.db-wal"), b"unmanifested state").unwrap();
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::InvalidPackage)
    ));

    fs::remove_file(package.join("library.db-wal")).unwrap();
    let manifest_path = package.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        &manifest_path,
        manifest.replace("assets/aa/question.enc", "assets/aa/question.enc:stream"),
    )
    .unwrap();
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::InvalidPackage)
    ));
}

#[test]
fn validation_requires_review_sessions_exactly_when_the_schema_requires_it() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database.execute("DROP TABLE review_sessions", []).unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 2);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP TABLE account_preferences", [])
            .unwrap();
        database
            .execute("DROP TABLE profile_preferences", [])
            .unwrap();
        for table in [
            "capture_draft_items",
            "capture_items",
            "capture_drafts",
            "capture_batches",
        ] {
            database
                .execute(&format!("DROP TABLE {table}"), [])
                .unwrap();
        }
        database.pragma_update(None, "user_version", 1).unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 1);
    assert!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).is_ok(),
        "schema v1 remains supported when the v2-only table is absent"
    );
}

#[test]
fn validation_requires_all_capture_tables_for_schema_v3() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP TABLE capture_draft_items", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 3);

    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));
}

#[test]
fn validation_requires_account_preferences_for_schema_v6_and_rejects_foreign_rows() {
    let missing_table_fixture = fixture();
    let (_, package) = created_package(&missing_table_fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP TABLE account_preferences", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 6);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    let fixture = fixture();
    {
        let connection = fixture.connection.lock().unwrap();
        connection.execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
             VALUES('foreign-profile', 'foreign-account', 'foreign', 1, 1)",
            [],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
             VALUES('foreign-account', 'foreign-profile', 1)",
                [],
            )
            .unwrap();
    }
    assert!(matches!(
        create_backup(
            &fixture.connection,
            &fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            fixture.destination.path(),
            1_725_000_000_000,
        ),
        Err(BackupError::ForeignAccountData)
    ));
}

#[test]
fn creation_rejects_application_storage_and_mixed_account_libraries() {
    let fixture = fixture();
    assert!(matches!(
        create_backup(
            &fixture.connection,
            &fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            fixture.root.path(),
            10,
        ),
        Err(BackupError::InvalidDestination)
    ));

    fixture
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO review_sessions(
               id, account_id, profile_id, mode, problem_ids_json,
               current_index, status, created_at_utc_ms, updated_at_utc_ms
             ) VALUES('foreign-session', 'foreign-account', ?1, 'due', '[]', 0, 'active', 1, 1)",
            [PROFILE_ID],
        )
        .unwrap();
    assert!(matches!(
        create_backup(
            &fixture.connection,
            &fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            fixture.destination.path(),
            10,
        ),
        Err(BackupError::ForeignAccountData)
    ));
    fixture
        .connection
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM review_sessions WHERE id = 'foreign-session'",
            [],
        )
        .unwrap();

    fixture
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms)
             VALUES('foreign-profile', 'foreign-account', '其他账户', 1, 1)",
            [],
        )
        .unwrap();
    assert!(matches!(
        create_backup(
            &fixture.connection,
            &fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            fixture.destination.path(),
            10,
        ),
        Err(BackupError::ForeignAccountData)
    ));
    assert_eq!(fs::read_dir(fixture.destination.path()).unwrap().count(), 0);
}

#[test]
fn failed_backup_removes_only_its_new_temporary_directory() {
    let fixture = fixture();
    fixture
        .connection
        .lock()
        .unwrap()
        .execute(
            "UPDATE assets SET encrypted_path = '../library.db' WHERE id = 'asset-1'",
            [],
        )
        .unwrap();
    let sentinel = fixture.destination.path().join("keep.txt");
    fs::write(&sentinel, b"keep").unwrap();

    assert!(matches!(
        create_backup(
            &fixture.connection,
            &fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            fixture.destination.path(),
            10,
        ),
        Err(BackupError::InvalidPackage)
    ));
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
    let entries = fs::read_dir(fixture.destination.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec!["keep.txt"]);
}
