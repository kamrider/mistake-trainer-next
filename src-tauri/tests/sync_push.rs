#![allow(clippy::manual_async_fn)]

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use mistake_trainer_next_lib::{
    infrastructure::{
        assets::{encrypt_asset, plaintext_sha256},
        database::run_migrations,
        supabase::{
            CloudError, CloudPushTransport, ObjectUploadResult, PushAcknowledgement,
            RemoteObjectMetadata,
        },
    },
    modules::sync_push::{SyncPushError, push_once},
};
use rusqlite::{Connection, params};
use tempfile::TempDir;
use uuid::Uuid;

const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const REMOTE_USER_ID: &str = "33333333-3333-4333-8333-333333333333";
const ACCESS_TOKEN: &str = "test-access-token";
const ASSET_KEY: [u8; 32] = [41; 32];
const NOW: i64 = 1_000_000;
const TUS_CHUNK_BYTES: usize = 6 * 1024 * 1024;

#[derive(Clone, Copy)]
enum AcknowledgementMode {
    Valid,
    Empty,
    NetworkFailure,
}

struct TransportState {
    object_exists: bool,
    expected_length: i64,
    expected_media_type: String,
    small_uploads: Vec<Vec<u8>>,
    created_resumable: usize,
    resumable_offset: i64,
    resumable_chunks: Vec<(i64, usize)>,
    pushed_operation_ids: Vec<Vec<String>>,
    acknowledgement_mode: AcknowledgementMode,
}

#[derive(Clone)]
struct MockTransport {
    state: Arc<Mutex<TransportState>>,
}

impl MockTransport {
    fn new(expected_length: usize, acknowledgement_mode: AcknowledgementMode) -> Self {
        Self {
            state: Arc::new(Mutex::new(TransportState {
                object_exists: false,
                expected_length: expected_length as i64,
                expected_media_type: "image/jpeg".to_owned(),
                small_uploads: Vec::new(),
                created_resumable: 0,
                resumable_offset: 0,
                resumable_chunks: Vec::new(),
                pushed_operation_ids: Vec::new(),
                acknowledgement_mode,
            })),
        }
    }
}

impl CloudPushTransport for MockTransport {
    fn object_metadata<'a>(
        &'a self,
        _access_token: &'a str,
        _storage_object: &'a str,
    ) -> impl Future<Output = Result<Option<RemoteObjectMetadata>, CloudError>> + Send + 'a {
        async move {
            let state = self.state.lock().unwrap();
            Ok(state.object_exists.then(|| RemoteObjectMetadata {
                byte_length: state.expected_length,
                media_type: state.expected_media_type.clone(),
            }))
        }
    }

    fn upload_small_object<'a>(
        &'a self,
        _access_token: &'a str,
        _storage_object: &'a str,
        _media_type: &'a str,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<ObjectUploadResult, CloudError>> + Send + 'a {
        async move {
            let mut state = self.state.lock().unwrap();
            state.small_uploads.push(bytes.to_vec());
            state.object_exists = true;
            Ok(ObjectUploadResult::Created)
        }
    }

    fn create_resumable_upload<'a>(
        &'a self,
        _access_token: &'a str,
        _storage_object: &'a str,
        _media_type: &'a str,
        _byte_length: i64,
    ) -> impl Future<Output = Result<String, CloudError>> + Send + 'a {
        async move {
            let mut state = self.state.lock().unwrap();
            state.created_resumable += 1;
            state.resumable_offset = 0;
            Ok("https://project.storage.supabase.co/storage/v1/upload/resumable/test".to_owned())
        }
    }

    fn resumable_offset<'a>(
        &'a self,
        _access_token: &'a str,
        _upload_url: &'a str,
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a {
        async move { Ok(Some(self.state.lock().unwrap().resumable_offset)) }
    }

    fn upload_resumable_chunk<'a>(
        &'a self,
        _access_token: &'a str,
        _upload_url: &'a str,
        offset: i64,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a {
        async move {
            let mut state = self.state.lock().unwrap();
            assert_eq!(offset, state.resumable_offset);
            state.resumable_chunks.push((offset, bytes.len()));
            state.resumable_offset += bytes.len() as i64;
            if state.resumable_offset == state.expected_length {
                state.object_exists = true;
            }
            Ok(Some(state.resumable_offset))
        }
    }

    fn push_operations<'a>(
        &'a self,
        _access_token: &'a str,
        operations: &'a serde_json::Value,
    ) -> impl Future<Output = Result<Vec<PushAcknowledgement>, CloudError>> + Send + 'a {
        async move {
            let operations = operations.as_array().ok_or(CloudError::InvalidResponse)?;
            let ids = operations
                .iter()
                .map(|operation| {
                    operation["operationId"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or(CloudError::InvalidResponse)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut state = self.state.lock().unwrap();
            state.pushed_operation_ids.push(ids);
            match state.acknowledgement_mode {
                AcknowledgementMode::Empty => Ok(Vec::new()),
                AcknowledgementMode::NetworkFailure => Err(CloudError::Network),
                AcknowledgementMode::Valid => operations
                    .iter()
                    .map(|operation| {
                        Ok(PushAcknowledgement {
                            operation_id: operation["operationId"]
                                .as_str()
                                .ok_or(CloudError::InvalidResponse)?
                                .to_owned(),
                            entity_type: operation["entityType"]
                                .as_str()
                                .ok_or(CloudError::InvalidResponse)?
                                .to_owned(),
                            entity_id: operation["entityId"]
                                .as_str()
                                .ok_or(CloudError::InvalidResponse)?
                                .to_owned(),
                            change_seq: 1,
                        })
                    })
                    .collect(),
            }
        }
    }
}

struct Fixture {
    connection: Connection,
    root: TempDir,
    asset_id: String,
    operation_id: String,
    plaintext: Vec<u8>,
}

fn fixture(plaintext: Vec<u8>) -> Fixture {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    run_migrations(&mut connection).unwrap();

    let root = tempfile::tempdir().unwrap();
    let relative_path = "aa/asset.enc";
    std::fs::create_dir_all(root.path().join("aa")).unwrap();
    std::fs::write(
        root.path().join(relative_path),
        encrypt_asset(&plaintext, &ASSET_KEY).unwrap(),
    )
    .unwrap();

    let asset_id = Uuid::now_v7().to_string();
    let operation_id = Uuid::now_v7().to_string();
    connection
        .execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES(?1, ?2, ?3, ?4, ?5, 'image/jpeg', ?6)",
            params![
                asset_id,
                ACCOUNT_ID,
                plaintext_sha256(&plaintext),
                relative_path,
                plaintext.len() as i64,
                NOW
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_operations(
                 id, account_id, profile_id, entity_type, entity_id, operation, payload_json,
                 status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
             ) VALUES(?1, ?2, NULL, 'asset', ?3, 'upsert', '{}', 'pending', 0, ?4, 0)",
            params![operation_id, ACCOUNT_ID, asset_id, NOW],
        )
        .unwrap();
    Fixture {
        connection,
        root,
        asset_id,
        operation_id,
        plaintext,
    }
}

fn operation_state(connection: &Connection, operation_id: &str) -> Option<(String, i64, String)> {
    connection
        .query_row(
            "SELECT status, attempt_count, COALESCE(last_error_code, '')
             FROM sync_operations WHERE id = ?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
}

#[test]
fn small_asset_is_verified_uploaded_then_acknowledged_atomically() {
    let mut fixture = fixture(b"small encrypted question image".to_vec());
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Valid);

    let report = tokio::runtime::Runtime::new().unwrap().block_on(push_once(
        &mut fixture.connection,
        &transport,
        ACCOUNT_ID,
        REMOTE_USER_ID,
        ACCESS_TOKEN,
        fixture.root.path(),
        &ASSET_KEY,
        NOW,
    ));

    let report = report.unwrap();
    assert_eq!(
        report.acknowledged_operation_ids,
        [fixture.operation_id.clone()]
    );
    assert_eq!(report.uploaded_asset_ids.len(), 1);
    assert!(operation_state(&fixture.connection, &fixture.operation_id).is_none());
    let state = transport.state.lock().unwrap();
    assert_eq!(state.small_uploads, [fixture.plaintext]);
    assert_eq!(state.pushed_operation_ids.len(), 1);
}

#[test]
fn missing_acknowledgement_keeps_the_outbox_operation_for_retry() {
    let mut fixture = fixture(b"answer image".to_vec());
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Empty);

    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap_err();

    assert!(matches!(error, SyncPushError::InvalidAcknowledgement));
    assert_eq!(
        operation_state(&fixture.connection, &fixture.operation_id),
        Some(("pending".to_owned(), 1, "cloud_ack_invalid".to_owned()))
    );
}

#[test]
fn corrupt_local_blob_never_reaches_storage_or_metadata_rpc() {
    let mut fixture = fixture(b"private image".to_vec());
    std::fs::write(fixture.root.path().join("aa/asset.enc"), b"corrupt").unwrap();
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Valid);

    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap_err();

    assert!(matches!(error, SyncPushError::InvalidLocalAsset));
    let state = transport.state.lock().unwrap();
    assert!(state.small_uploads.is_empty());
    assert!(state.resumable_chunks.is_empty());
    assert!(state.pushed_operation_ids.is_empty());
    assert_eq!(
        operation_state(&fixture.connection, &fixture.operation_id),
        Some(("pending".to_owned(), 1, "local_asset_invalid".to_owned()))
    );
}

#[test]
fn large_asset_uses_exact_six_mebibyte_tus_chunks_before_metadata_push() {
    let plaintext = vec![0x5a; TUS_CHUNK_BYTES + 137];
    let mut fixture = fixture(plaintext);
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Valid);

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap();

    let state = transport.state.lock().unwrap();
    assert_eq!(state.created_resumable, 1);
    assert_eq!(
        state.resumable_chunks,
        [(0, TUS_CHUNK_BYTES), (TUS_CHUNK_BYTES as i64, 137)]
    );
    assert_eq!(state.pushed_operation_ids.len(), 1);
    assert_eq!(
        fixture
            .connection
            .query_row("SELECT COUNT(*) FROM cloud_asset_transfers", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn resumable_upload_uses_the_server_offset_and_sends_only_the_remainder() {
    let plaintext = vec![0x36; TUS_CHUNK_BYTES + 211];
    let mut fixture = fixture(plaintext);
    let upload_url = "https://project.storage.supabase.co/storage/v1/upload/resumable/resume";
    fixture
        .connection
        .execute(
            "INSERT INTO cloud_asset_transfers(
                 asset_id, upload_url, confirmed_offset, expires_at_utc_ms, updated_at_utc_ms
             ) VALUES(?1, ?2, 3, ?3, ?4)",
            params![fixture.asset_id, upload_url, NOW + 60_000, NOW - 1],
        )
        .unwrap();
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Valid);
    transport.state.lock().unwrap().resumable_offset = TUS_CHUNK_BYTES as i64;

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap();

    let state = transport.state.lock().unwrap();
    assert_eq!(state.created_resumable, 0);
    assert_eq!(state.resumable_chunks, [(TUS_CHUNK_BYTES as i64, 211)]);
}

#[test]
fn expired_resumable_upload_is_replaced_before_any_resume_request() {
    let plaintext = vec![0x78; TUS_CHUNK_BYTES + 19];
    let mut fixture = fixture(plaintext);
    fixture
        .connection
        .execute(
            "INSERT INTO cloud_asset_transfers(
                 asset_id, upload_url, confirmed_offset, expires_at_utc_ms, updated_at_utc_ms
             ) VALUES(?1, 'https://expired.invalid/private-token', 7, ?2, ?2)",
            params![fixture.asset_id, NOW],
        )
        .unwrap();
    let transport = MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::Valid);

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap();

    let state = transport.state.lock().unwrap();
    assert_eq!(state.created_resumable, 1);
    assert_eq!(state.resumable_chunks[0], (0, TUS_CHUNK_BYTES));
}

#[test]
fn rpc_network_failure_preserves_the_same_operation_id_for_replay() {
    let mut fixture = fixture(b"retryable image".to_vec());
    let transport =
        MockTransport::new(fixture.plaintext.len(), AcknowledgementMode::NetworkFailure);

    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW,
        ))
        .unwrap_err();
    assert!(matches!(error, SyncPushError::Cloud(CloudError::Network)));
    assert_eq!(
        operation_state(&fixture.connection, &fixture.operation_id),
        Some(("pending".to_owned(), 1, "cloud_network".to_owned()))
    );
    assert_eq!(
        transport.state.lock().unwrap().pushed_operation_ids[0],
        [fixture.operation_id.clone()]
    );

    transport.state.lock().unwrap().acknowledgement_mode = AcknowledgementMode::Valid;
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(push_once(
            &mut fixture.connection,
            &transport,
            ACCOUNT_ID,
            REMOTE_USER_ID,
            ACCESS_TOKEN,
            fixture.root.path(),
            &ASSET_KEY,
            NOW + 1_000_000,
        ))
        .unwrap();
    assert!(operation_state(&fixture.connection, &fixture.operation_id).is_none());
    let state = transport.state.lock().unwrap();
    assert_eq!(state.small_uploads.len(), 1);
    assert_eq!(
        state.pushed_operation_ids,
        [
            vec![fixture.operation_id.clone()],
            vec![fixture.operation_id.clone()]
        ]
    );
}
