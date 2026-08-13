use std::io::Cursor;

use image::{DynamicImage, GrayImage, ImageFormat, Luma};
use mistake_trainer_next_lib::{
    infrastructure::{
        assets::KeyedAssetDecryptor,
        database::{open_encrypted_database, run_migrations},
    },
    modules::{
        capture_inbox::{
            CaptureBatchState, CaptureInboxError, CreateCaptureBatch, IngestCaptureItem,
            create_capture_batch, ingest_capture_item,
        },
        capture_quality::check_capture_quality,
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::{TempDir, tempdir};

const ACCOUNT: &str = "capture-quality-account";
const DATABASE_KEY: &str = "capture-quality-database-key";
const ASSET_KEY: [u8; 32] = [91; 32];

struct TestLibrary {
    directory: TempDir,
    profile_id: String,
}

impl TestLibrary {
    fn new() -> Self {
        let directory = tempdir().expect("temp directory");
        let mut connection =
            open_encrypted_database(&directory.path().join("library.db"), DATABASE_KEY)
                .expect("open database");
        run_migrations(&mut connection).expect("migrate database");
        let profile = create_profile(
            &mut connection,
            CreateProfile {
                account_id: ACCOUNT.to_owned(),
                name: "student".to_owned(),
                now_utc_ms: 1,
            },
        )
        .expect("create profile");
        Self {
            directory,
            profile_id: profile.id,
        }
    }

    fn open(&self) -> rusqlite::Connection {
        let mut connection =
            open_encrypted_database(&self.directory.path().join("library.db"), DATABASE_KEY)
                .expect("reopen database");
        run_migrations(&mut connection).expect("migrate database");
        connection
    }

    fn blob_root(&self) -> std::path::PathBuf {
        self.directory.path().join("assets")
    }
}

fn document_png() -> Vec<u8> {
    let mut image = GrayImage::from_pixel(320, 220, Luma([238]));
    for row in 0..6 {
        let y = 30 + row * 28;
        for py in y..y + 4 {
            for x in 40..280 {
                image.put_pixel(x, py, Luma([28]));
            }
        }
    }
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(image)
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode PNG");
    output.into_inner()
}

fn batch_and_item(
    library: &TestLibrary,
    connection: &mut rusqlite::Connection,
    suffix: &str,
) -> (String, String) {
    let batch = create_capture_batch(
        connection,
        CreateCaptureBatch {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            subject: "数学".to_owned(),
            state: CaptureBatchState::Organizing,
            now_utc_ms: 10,
        },
    )
    .expect("create batch");
    let item = ingest_capture_item(
        connection,
        &library.blob_root(),
        &ASSET_KEY,
        IngestCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            client_upload_id: format!("quality-{suffix}"),
            source_name: format!("quality-{suffix}.png"),
            source_sequence: None,
            bytes: document_png(),
            now_utc_ms: 20,
        },
    )
    .expect("ingest item");
    (batch.id, item.id)
}

#[test]
fn quality_check_is_owned_read_only_and_item_scoped() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let (batch_id, item_id) = batch_and_item(&library, &mut connection, "first");
    let (other_batch_id, other_item_id) = batch_and_item(&library, &mut connection, "second");
    let revision_before: i64 = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get(0),
        )
        .expect("batch revision");
    let outbox_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox count");
    let decryptor = KeyedAssetDecryptor::new(&ASSET_KEY);

    let report = check_capture_quality(
        &connection,
        &library.blob_root(),
        &decryptor,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        &item_id,
    )
    .expect("check owned item");
    assert_eq!(report.item_id, item_id);

    let wrong_item = check_capture_quality(
        &connection,
        &library.blob_root(),
        &decryptor,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        &other_item_id,
    )
    .expect_err("an item from another batch must fail closed");
    assert!(matches!(wrong_item, CaptureInboxError::ItemNotFound));

    for (account_id, profile_id, expected) in [
        ("foreign-account", library.profile_id.as_str(), "account"),
        (ACCOUNT, "foreign-profile", "profile"),
    ] {
        let error = check_capture_quality(
            &connection,
            &library.blob_root(),
            &decryptor,
            account_id,
            profile_id,
            &other_batch_id,
            &other_item_id,
        )
        .expect_err(expected);
        assert!(matches!(error, CaptureInboxError::BatchNotFound));
    }

    let revision_after: i64 = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get(0),
        )
        .expect("batch revision");
    let outbox_after: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .expect("outbox count");
    assert_eq!(revision_after, revision_before);
    assert_eq!(outbox_after, outbox_before);
}

#[test]
fn corrupt_encrypted_asset_fails_closed() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let (batch_id, item_id) = batch_and_item(&library, &mut connection, "corrupt");
    let encrypted_path: String = connection
        .query_row(
            "SELECT a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id WHERE i.id = ?1",
            [&item_id],
            |row| row.get(0),
        )
        .expect("asset path");
    std::fs::write(
        library.blob_root().join(encrypted_path),
        b"not encrypted image data",
    )
    .expect("corrupt encrypted asset");

    let error = check_capture_quality(
        &connection,
        &library.blob_root(),
        &KeyedAssetDecryptor::new(&ASSET_KEY),
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        &item_id,
    )
    .expect_err("corrupt ciphertext must fail closed");
    assert!(matches!(error, CaptureInboxError::Crypto));
}
