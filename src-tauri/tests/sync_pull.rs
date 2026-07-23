use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use mistake_trainer_next_lib::{
    infrastructure::{
        database::run_migrations,
        supabase::{CloudError, CloudPullTransport, DownloadedRemoteAsset, RemotePullChange},
    },
    modules::sync_pull::pull_until_current,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const REMOTE_USER_ID: &str = "33333333-3333-4333-8333-333333333333";
const PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";
const PROBLEM_ID: &str = "0191365e-2f2f-7b89-b3b0-333333333333";
const ASSET_ID: &str = "0191365e-2f2f-7b89-b3b0-444444444444";
const REVIEW_ID: &str = "0191365e-2f2f-7b89-b3b0-555555555555";
const DEVICE_ID: &str = "0191365e-2f2f-7b89-b3b0-666666666666";
const ASSET_KEY: [u8; 32] = [41; 32];

#[derive(Clone)]
struct MockPull {
    changes: Arc<Mutex<Vec<RemotePullChange>>>,
    objects: Arc<HashMap<String, DownloadedRemoteAsset>>,
}

impl CloudPullTransport for MockPull {
    fn pull_changes<'a>(
        &'a self,
        _access_token: &'a str,
        after: i64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<RemotePullChange>, CloudError>> + Send + 'a {
        async move {
            let changes = self.changes.lock().unwrap();
            Ok(changes
                .iter()
                .filter(|change| change.change_seq > after)
                .take(limit)
                .cloned()
                .collect())
        }
    }

    fn download_object<'a>(
        &'a self,
        _access_token: &'a str,
        storage_object: &'a str,
    ) -> impl Future<Output = Result<DownloadedRemoteAsset, CloudError>> + Send + 'a {
        async move {
            self.objects
                .get(storage_object)
                .cloned()
                .ok_or(CloudError::InvalidResponse)
        }
    }
}

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    run_migrations(&mut connection).unwrap();
    connection
}

fn png_bytes() -> Vec<u8> {
    let image = RgbaImage::from_pixel(4, 3, Rgba([190, 88, 63, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn change(
    sequence: i64,
    entity_type: &str,
    entity_id: &str,
    payload: serde_json::Value,
) -> RemotePullChange {
    RemotePullChange {
        change_seq: sequence,
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        operation: "upsert".to_owned(),
        payload,
    }
}

fn delete_change(
    sequence: i64,
    entity_type: &str,
    entity_id: &str,
    payload: serde_json::Value,
) -> RemotePullChange {
    RemotePullChange {
        change_seq: sequence,
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        operation: "delete".to_owned(),
        payload,
    }
}

fn fixture() -> (MockPull, Vec<u8>) {
    let bytes = png_bytes();
    let hash = mistake_trainer_next_lib::infrastructure::assets::plaintext_sha256(&bytes);
    let storage_object = format!("{REMOTE_USER_ID}/{hash}");
    let account = json!({"accountId": REMOTE_USER_ID});
    let mut profile = account.clone();
    profile["id"] = json!(PROFILE_ID);
    profile["name"] = json!("同步档案");
    profile["revision"] = json!(1);
    profile["createdAtUtcMs"] = json!(10);
    profile["updatedAtUtcMs"] = json!(20);
    let mut asset = account.clone();
    asset["id"] = json!(ASSET_ID);
    asset["plaintextSha256"] = json!(hash);
    asset["storageObject"] = json!(storage_object.clone());
    asset["byteLength"] = json!(bytes.len() as i64);
    asset["mediaType"] = json!("image/png");
    asset["revision"] = json!(1);
    asset["createdAtUtcMs"] = json!(30);
    let mut problem = account.clone();
    problem["id"] = json!(PROBLEM_ID);
    problem["profileId"] = json!(PROFILE_ID);
    problem["subject"] = json!("数学");
    problem["tags"] = json!(["同步"]);
    problem["note"] = json!("");
    problem["status"] = json!("active");
    problem["timeLimitSeconds"] = serde_json::Value::Null;
    problem["assets"] = json!([{"assetId": ASSET_ID, "role": "question", "position": 0}]);
    problem["revision"] = json!(1);
    problem["createdAtUtcMs"] = json!(40);
    problem["updatedAtUtcMs"] = json!(50);
    let mut review = account;
    review["id"] = json!(REVIEW_ID);
    review["profileId"] = json!(PROFILE_ID);
    review["problemId"] = json!(PROBLEM_ID);
    review["deviceId"] = json!(DEVICE_ID);
    review["rating"] = json!("good");
    review["durationMs"] = json!(900);
    review["occurredAtUtcMs"] = json!(1000);
    review["algorithmVersion"] = json!("fsrs-6.6.1");
    review["parameterVersion"] = json!("default-6.6.1");
    let changes = vec![
        change(1, "learner_profile", PROFILE_ID, profile),
        change(2, "asset", ASSET_ID, asset),
        change(3, "problem", PROBLEM_ID, problem),
        change(4, "review_event", REVIEW_ID, review),
    ];
    let mut objects = HashMap::new();
    objects.insert(
        storage_object,
        DownloadedRemoteAsset {
            bytes: bytes.clone(),
            media_type: "image/png".to_owned(),
        },
    );
    (
        MockPull {
            changes: Arc::new(Mutex::new(changes)),
            objects: Arc::new(objects),
        },
        bytes,
    )
}

#[test]
fn valid_page_applies_assets_library_and_deterministic_schedule() {
    let (transport, bytes) = fixture();
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let report = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap();

    assert_eq!(report.applied_count, 4);
    assert_eq!(report.downloaded_asset_count, 1);
    assert_eq!(report.final_cursor, 4);
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM problems", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM problem_assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let schedule = connection
        .query_row(
            "SELECT due_at_utc_ms, algorithm_version FROM schedule_states WHERE problem_id = ?1",
            [PROBLEM_ID],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert!(schedule.0 > 1_000);
    assert_eq!(schedule.1, "fsrs-6.6.1");
    assert_eq!(
        std::fs::read(
            root.path()
                .join("blobs/01/0191365e-2f2f-7b89-b3b0-444444444444.mtb")
        )
        .unwrap()
        .len(),
        bytes.len() + 32
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT pull_cursor FROM cloud_sync_state WHERE account_id = ?1",
                [ACCOUNT_ID],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
}

#[test]
fn invalid_account_is_rejected_without_advancing_cursor_or_writing_data() {
    let (transport, _) = fixture();
    transport.changes.lock().unwrap()[0].payload["accountId"] =
        json!("33333333-3333-4333-8333-999999999999");
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap_err();
    assert_eq!(error.stable_code(), "cloud_change_invalid");
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM learner_profiles", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let cursor: Option<i64> = connection
        .query_row(
            "SELECT pull_cursor FROM cloud_sync_state WHERE account_id = ?1",
            [ACCOUNT_ID],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    assert_eq!(cursor.unwrap_or(0), 0);
}

#[test]
fn mismatched_download_is_rejected_and_does_not_leave_a_blob() {
    let (transport, _) = fixture();
    let object = transport.objects.values().next().unwrap().clone();
    let mut broken = object;
    broken.bytes = vec![1, 2, 3];
    let mut objects = (*transport.objects).clone();
    let key = objects.keys().next().unwrap().to_owned();
    objects.insert(key, broken);
    let transport = MockPull {
        objects: Arc::new(objects),
        ..transport
    };
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap_err();
    assert_eq!(error.stable_code(), "cloud_asset_mismatch");
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        !root
            .path()
            .join("blobs/01/0191365e-2f2f-7b89-b3b0-444444444444.mtb")
            .exists()
    );
}

#[test]
fn profile_and_orphan_asset_tombstones_delete_locally_and_select_a_replacement() {
    const REPLACEMENT_PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-777777777777";
    const PROFILE_TOMBSTONE_ID: &str = "0191365e-2f2f-7b89-b3b0-888888888888";
    const ASSET_TOMBSTONE_ID: &str = "0191365e-2f2f-7b89-b3b0-999999999999";
    let mut connection = database();
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, '待删除', 10, 10, 99), (?3, ?2, '保留档案', 20, 20, 1)",
        rusqlite::params![PROFILE_ID, ACCOUNT_ID, REPLACEMENT_PROFILE_ID],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO sync_entity_snapshots(
               account_id, profile_id, entity_type, entity_id, revision,
               payload_json, updated_at_utc_ms
             ) VALUES(?1, ?2, 'learner_profile', ?2, 99, ?3, 10)",
            rusqlite::params![
                ACCOUNT_ID,
                PROFILE_ID,
                json!({
                    "id": PROFILE_ID,
                    "name": "待删除",
                    "revision": 99,
                    "createdAtUtcMs": 10,
                    "updatedAtUtcMs": 10
                })
                .to_string()
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_operations(
           id, account_id, profile_id, entity_type, entity_id, operation,
           payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
         ) VALUES(
           '0191365e-2f2f-7b89-b3b0-666666666666', ?1, ?2, 'learner_profile', ?2,
           'upsert', '{}', 'pending', 0, 90, 90
         )",
            rusqlite::params![ACCOUNT_ID, PROFILE_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
         VALUES(?1, ?2, 10)",
            rusqlite::params![ACCOUNT_ID, PROFILE_ID],
        )
        .unwrap();
    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES(?1, ?2, ?3, 'blobs/01/orphan.mtb', 64, 'image/png', 10)",
        rusqlite::params![ASSET_ID, ACCOUNT_ID, "a".repeat(64)],
    ).unwrap();
    connection.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, '数学', 10, 10, 1)",
        rusqlite::params![PROBLEM_ID, ACCOUNT_ID, PROFILE_ID],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position)
         VALUES(?1, ?2, 'question', 0)",
            rusqlite::params![PROBLEM_ID, ASSET_ID],
        )
        .unwrap();
    let profile_delete = delete_change(
        1,
        "learner_profile",
        PROFILE_ID,
        json!({
            "accountId": REMOTE_USER_ID,
            "tombstoneId": PROFILE_TOMBSTONE_ID,
            "profileId": null,
            "entityType": "learner_profile",
            "entityId": PROFILE_ID,
            "deletedAtUtcMs": 100,
            "purgeAfterUtcMs": 2_592_000_100_i64,
            "deletedRevision": 2
        }),
    );
    let asset_delete = delete_change(
        2,
        "asset",
        ASSET_ID,
        json!({
            "accountId": REMOTE_USER_ID,
            "tombstoneId": ASSET_TOMBSTONE_ID,
            "profileId": null,
            "entityType": "asset",
            "entityId": ASSET_ID,
            "deletedAtUtcMs": 101,
            "purgeAfterUtcMs": 2_592_000_101_i64,
            "deletedRevision": 2
        }),
    );
    let transport = MockPull {
        changes: Arc::new(Mutex::new(vec![profile_delete, asset_delete])),
        objects: Arc::new(HashMap::new()),
    };
    let root = tempfile::tempdir().unwrap();
    let blob = root.path().join("blobs/01/orphan.mtb");
    std::fs::create_dir_all(blob.parent().unwrap()).unwrap();
    std::fs::write(&blob, b"encrypted").unwrap();

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            1_000,
        ))
        .unwrap();

    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM learner_profiles WHERE id = ?1",
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_operations WHERE entity_type = 'learner_profile' AND entity_id = ?1",
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0,
        "an authoritative profile tombstone must discard a stale high-revision upsert"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT active_profile_id FROM account_preferences WHERE account_id = ?1",
                [ACCOUNT_ID],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        REPLACEMENT_PROFILE_ID
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM assets WHERE id = ?1",
                [ASSET_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert!(!blob.exists());
}

#[test]
fn concurrent_problem_edits_on_different_fields_auto_merge_and_reenqueue() {
    let (transport, _) = fixture();
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap();
    connection
        .execute(
            "UPDATE problems
             SET subject = 'physics', revision = 2, updated_at_utc_ms = 60
             WHERE id = ?1",
            [PROBLEM_ID],
        )
        .unwrap();

    let mut remote = transport.changes.lock().unwrap()[2].payload.clone();
    remote["note"] = json!("cloud note");
    remote["revision"] = json!(2);
    remote["updatedAtUtcMs"] = json!(70);
    transport
        .changes
        .lock()
        .unwrap()
        .push(change(5, "problem", PROBLEM_ID, remote));

    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            20_000,
        ))
        .unwrap();

    let merged = connection
        .query_row(
            "SELECT subject, note, revision FROM problems WHERE id = ?1",
            [PROBLEM_ID],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(merged, ("physics".to_owned(), "cloud note".to_owned(), 3));
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts WHERE entity_id = ?1 AND resolved_at_utc_ms IS NULL",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_operations
                 WHERE entity_type = 'problem' AND entity_id = ?1 AND operation = 'upsert'",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn concurrent_problem_edits_on_the_same_field_create_one_true_conflict() {
    let (transport, _) = fixture();
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap();
    connection
        .execute(
            "UPDATE problems
             SET note = 'local note', revision = 2, updated_at_utc_ms = 60
             WHERE id = ?1",
            [PROBLEM_ID],
        )
        .unwrap();

    let mut remote = transport.changes.lock().unwrap()[2].payload.clone();
    remote["note"] = json!("cloud note");
    remote["revision"] = json!(2);
    remote["updatedAtUtcMs"] = json!(70);
    transport
        .changes
        .lock()
        .unwrap()
        .push(change(5, "problem", PROBLEM_ID, remote));

    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            20_000,
        ))
        .unwrap();

    assert_eq!(
        connection
            .query_row(
                "SELECT note FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "local note"
    );
    let conflict = connection
        .query_row(
            "SELECT field_name, local_value_json, remote_value_json
             FROM sync_conflicts
             WHERE entity_id = ?1 AND resolved_at_utc_ms IS NULL",
            [PROBLEM_ID],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        conflict,
        (
            "note".to_owned(),
            "\"local note\"".to_owned(),
            "\"cloud note\"".to_owned(),
        )
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_operations
                 WHERE entity_type = 'problem' AND entity_id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

#[test]
fn remote_problem_delete_conflicts_with_a_local_edit_instead_of_erasing_it() {
    const TOMBSTONE_ID: &str = "0191365e-2f2f-7b89-b3b0-777777777777";
    let (transport, _) = fixture();
    let mut connection = database();
    let root = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            10_000,
        ))
        .unwrap();
    connection
        .execute(
            "UPDATE problems
             SET note = 'keep this local work', revision = 2, updated_at_utc_ms = 60
             WHERE id = ?1",
            [PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_operations(
               id, account_id, profile_id, entity_type, entity_id, operation,
               payload_json, status, attempt_count, created_at_utc_ms,
               next_attempt_at_utc_ms
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-888888888888', ?1, ?2, 'problem', ?3,
               'upsert', '{}', 'pending', 0, 60, 60
             )",
            rusqlite::params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    transport.changes.lock().unwrap().push(delete_change(
        5,
        "problem",
        PROBLEM_ID,
        json!({
            "accountId": REMOTE_USER_ID,
            "tombstoneId": TOMBSTONE_ID,
            "profileId": PROFILE_ID,
            "entityType": "problem",
            "entityId": PROBLEM_ID,
            "deletedAtUtcMs": 100,
            "purgeAfterUtcMs": 2_592_000_100_i64,
            "deletedRevision": 2
        }),
    ));

    runtime
        .block_on(pull_until_current(
            &mut connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            "access-token",
            root.path(),
            &ASSET_KEY,
            20_000,
        ))
        .unwrap();

    let local = connection
        .query_row(
            "SELECT note, status FROM problems WHERE id = ?1",
            [PROBLEM_ID],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(
        local,
        ("keep this local work".to_owned(), "active".to_owned())
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT field_name FROM sync_conflicts
                 WHERE entity_id = ?1 AND resolved_at_utc_ms IS NULL",
                [PROBLEM_ID],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "__deleted__"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_operations
                 WHERE entity_type = 'problem' AND entity_id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}
