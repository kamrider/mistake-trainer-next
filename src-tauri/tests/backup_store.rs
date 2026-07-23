use std::{fs, sync::Mutex};

use mistake_trainer_next_lib::{
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        database::{open_encrypted_database, open_encrypted_database_read_only, run_migrations},
    },
    modules::backup::{
        BackupError, create_backup, prepare_backup_restore, validate_backup,
        validate_restore_candidate,
    },
};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};
use uuid::Uuid;

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

fn remove_v11_cloud_schema(database: &Connection) {
    database
        .execute_batch(
            "DROP INDEX sync_operations_lease_idx;
         DROP TABLE cloud_asset_transfers;
         DROP TABLE cloud_sync_state;
         ALTER TABLE sync_operations DROP COLUMN lease_id;
         ALTER TABLE sync_operations DROP COLUMN lease_expires_at_utc_ms;
         ALTER TABLE sync_operations DROP COLUMN last_error_code;",
        )
        .unwrap();
}

fn remove_v12_derivation_schema(database: &Connection) {
    database
        .execute_batch(
            "DROP INDEX capture_items_active_sequence_idx;
         DROP TABLE asset_derivations;
         DROP TABLE capture_source_retention;
         ALTER TABLE capture_items DROP COLUMN superseded_by_derivation_id;",
        )
        .unwrap();
}

fn remove_v13_sync_merge_schema(database: &Connection) {
    database
        .execute_batch(
            "DROP INDEX sync_conflicts_open_field_idx;
         DROP INDEX sync_entity_snapshots_profile_idx;
         DROP TABLE sync_entity_snapshots;
         ALTER TABLE sync_conflicts DROP COLUMN resolved_value_json;
         ALTER TABLE sync_conflicts DROP COLUMN resolution;",
        )
        .unwrap();
}

#[test]
fn schema_v13_backup_requires_merge_state_and_rejects_foreign_snapshots() {
    let missing_index_fixture = fixture();
    let (_, package) = created_package(&missing_index_fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP INDEX sync_conflicts_open_field_idx", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 13);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    let foreign_snapshot_fixture = fixture();
    foreign_snapshot_fixture
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO sync_entity_snapshots(
               account_id, profile_id, entity_type, entity_id, revision,
               payload_json, updated_at_utc_ms
             ) VALUES(
               'foreign-account', NULL, 'learner_profile', 'foreign-profile',
               1, '{\"name\":\"foreign\"}', 1
             )",
            [],
        )
        .unwrap();
    assert!(matches!(
        create_backup(
            &foreign_snapshot_fixture.connection,
            &foreign_snapshot_fixture.blob_root,
            DATABASE_KEY,
            ACCOUNT_ID,
            foreign_snapshot_fixture.destination.path(),
            1_725_000_000_000,
        ),
        Err(BackupError::ForeignAccountData)
    ));
}

#[test]
fn schema_v11_backup_preserves_cloud_progress_and_requires_the_complete_shape() {
    let fixture = fixture();
    {
        let connection = fixture.connection.lock().unwrap();
        connection
            .execute(
                "INSERT INTO cloud_sync_state(
                 account_id, pull_cursor, last_attempt_at_utc_ms, last_success_at_utc_ms,
                 last_error_code, remote_user_fingerprint
             ) VALUES(?1, 42, 100, 90, 'network', 'fingerprint')",
                [ACCOUNT_ID],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cloud_asset_transfers(
                 asset_id, upload_url, confirmed_offset, expires_at_utc_ms, updated_at_utc_ms
             ) VALUES('asset-1', 'https://example.invalid/upload/opaque', 6, 200, 110)",
                [],
            )
            .unwrap();
    }

    let (_, package) = created_package(&fixture);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(package.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 13);
    validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).unwrap();
    {
        let database =
            open_encrypted_database_read_only(&package.join("library.db"), DATABASE_KEY).unwrap();
        let state: (i64, Option<String>) = database
            .query_row(
                "SELECT pull_cursor, last_error_code FROM cloud_sync_state WHERE account_id = ?1",
                [ACCOUNT_ID],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, (42, Some("network".to_owned())));
        assert_eq!(
            database
                .query_row(
                    "SELECT confirmed_offset FROM cloud_asset_transfers WHERE asset_id = 'asset-1'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            6
        );
        let token_columns: i64 = database
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('cloud_sync_state')
             WHERE lower(name) LIKE '%access_token%' OR lower(name) LIKE '%refresh_token%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(token_columns, 0);
    }

    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP INDEX sync_operations_lease_idx", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 11);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));
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
fn prepare_restore_copies_a_verified_opaque_candidate_and_revalidates_it() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    let application_root = tempdir().expect("application root");

    let candidate = prepare_backup_restore(
        &package,
        application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        1_725_000_000_000,
    )
    .expect("prepare restore candidate");

    Uuid::parse_str(&candidate.id).expect("opaque UUID candidate id");
    assert_eq!(candidate.summary.asset_count, 1);
    assert!(candidate.summary.ready_for_restore);
    assert_eq!(candidate.expires_at_utc_ms, 1_725_086_400_000_f64);
    let serialized = serde_json::to_string(&candidate).unwrap();
    assert!(!serialized.contains(package.to_string_lossy().as_ref()));
    assert!(!serialized.contains(application_root.path().to_string_lossy().as_ref()));

    let revalidated = validate_restore_candidate(
        application_root.path(),
        &candidate.id,
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        1_725_000_000_001,
    )
    .expect("revalidate staged candidate");
    assert_eq!(revalidated, candidate.summary);
}

#[test]
fn prepare_restore_rejects_tampering_expiry_and_forged_ids_without_touching_live_data() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    let application_root = tempdir().expect("application root");
    let live_database_before = fs::read(fixture.root.path().join("library.db")).unwrap();
    let live_asset_before = fs::read(fixture.blob_root.join("aa/question.enc")).unwrap();

    let candidate = prepare_backup_restore(
        &package,
        application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        1_725_000_000_000,
    )
    .expect("prepare restore candidate");
    let staged_asset = application_root
        .path()
        .join(format!(".mistake-trainer-restore-{}", candidate.id))
        .join("assets/aa/question.enc");
    fs::write(&staged_asset, b"tampered").unwrap();

    assert!(matches!(
        validate_restore_candidate(
            application_root.path(),
            &candidate.id,
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            1_725_000_000_001,
        ),
        Err(BackupError::Integrity)
    ));
    assert!(matches!(
        validate_restore_candidate(
            application_root.path(),
            &Uuid::now_v7().to_string(),
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            1_725_000_000_001,
        ),
        Err(BackupError::InvalidPackage)
    ));

    let fresh = prepare_backup_restore(
        &package,
        application_root.path(),
        DATABASE_KEY,
        &ASSET_KEY,
        ACCOUNT_ID,
        1_725_000_000_000,
    )
    .expect("fresh candidate");
    assert!(matches!(
        validate_restore_candidate(
            application_root.path(),
            &fresh.id,
            DATABASE_KEY,
            &ASSET_KEY,
            ACCOUNT_ID,
            1_725_086_400_001,
        ),
        Err(BackupError::ExpiredCandidate)
    ));

    assert_eq!(
        fs::read(fixture.root.path().join("library.db")).unwrap(),
        live_database_before
    );
    assert_eq!(
        fs::read(fixture.blob_root.join("aa/question.enc")).unwrap(),
        live_asset_before
    );
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
        remove_v13_sync_merge_schema(&database);
        remove_v12_derivation_schema(&database);
        remove_v11_cloud_schema(&database);
        database
            .execute("DROP TABLE legacy_import_entities", [])
            .unwrap();
        database.execute("DROP TABLE legacy_imports", []).unwrap();
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
fn validation_requires_focus_columns_for_schema_v8() {
    let preference_fixture = fixture();
    let (_, package) = created_package(&preference_fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute(
                "ALTER TABLE profile_preferences DROP COLUMN review_focus_policy",
                [],
            )
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 8);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    let session_fixture = fixture();
    let (_, package) = created_package(&session_fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP TRIGGER review_sessions_focus_state_insert_guard", [])
            .unwrap();
        database
            .execute("DROP TRIGGER review_sessions_focus_state_update_guard", [])
            .unwrap();
        database
            .execute(
                "ALTER TABLE review_sessions DROP COLUMN focus_order_json",
                [],
            )
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 8);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));
}

#[test]
fn validation_requires_review_history_index_only_for_schema_v9() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP INDEX review_events_profile_time_idx", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 9);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        remove_v13_sync_merge_schema(&database);
        remove_v12_derivation_schema(&database);
        remove_v11_cloud_schema(&database);
        database
            .execute("DROP TABLE legacy_import_entities", [])
            .unwrap();
        database.execute("DROP TABLE legacy_imports", []).unwrap();
        database.pragma_update(None, "user_version", 8).unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 8);
    assert!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).is_ok(),
        "schema v8 remains valid before the history index existed"
    );
}

#[test]
fn validation_requires_legacy_import_ledger_only_for_schema_v10() {
    let fixture = fixture();
    let (_, package) = created_package(&fixture);
    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        database
            .execute("DROP INDEX legacy_import_entities_import_idx", [])
            .unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 10);
    assert!(matches!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID),
        Err(BackupError::Integrity)
    ));

    {
        let database = open_encrypted_database(&package.join("library.db"), DATABASE_KEY).unwrap();
        remove_v13_sync_merge_schema(&database);
        remove_v12_derivation_schema(&database);
        remove_v11_cloud_schema(&database);
        database
            .execute("DROP TABLE legacy_import_entities", [])
            .unwrap();
        database.execute("DROP TABLE legacy_imports", []).unwrap();
        database.pragma_update(None, "user_version", 9).unwrap();
        database
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
    }
    refresh_database_manifest(&package, 9);
    assert!(
        validate_backup(&package, DATABASE_KEY, &ASSET_KEY, ACCOUNT_ID).is_ok(),
        "schema v9 remains valid before the import ledger existed"
    );
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
