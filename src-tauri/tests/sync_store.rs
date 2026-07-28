#![allow(clippy::too_many_arguments)]

use mistake_trainer_next_lib::{
    infrastructure::database::run_migrations,
    modules::sync_store::{
        SyncStoreError, WireEntity, acknowledge_push_batch, discard_expired_asset_transfers,
        fail_push_batch, lease_push_batch, pull_cursor, record_pull_success,
    },
};
use rusqlite::{Connection, params};
use uuid::Uuid;

const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const OTHER_ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-999999999999";
const REMOTE_USER_ID: &str = "33333333-3333-4333-8333-333333333333";
const PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";
const PROBLEM_ID: &str = "0191365e-2f2f-7b89-b3b0-333333333333";
const QUESTION_ASSET_ID: &str = "0191365e-2f2f-7b89-b3b0-444444444444";
const ANSWER_ASSET_ID: &str = "0191365e-2f2f-7b89-b3b0-555555555555";

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    run_migrations(&mut connection).unwrap();
    connection
}

fn insert_profile(connection: &Connection, id: &str, account_id: &str, name: &str) {
    connection.execute(
        "INSERT INTO learner_profiles(id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision)
         VALUES(?1, ?2, ?3, 10, 20, 2)",
        params![id, account_id, name],
    ).unwrap();
}

fn insert_operation(
    connection: &Connection,
    id: &str,
    account_id: &str,
    profile_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    operation: &str,
    created_at: i64,
) {
    connection
        .execute(
            "INSERT INTO sync_operations(
             id, account_id, profile_id, entity_type, entity_id, operation, payload_json,
             status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, '{\"stale\":true}', 'pending', 0, ?7, 0)",
            params![
                id,
                account_id,
                profile_id,
                entity_type,
                entity_id,
                operation,
                created_at
            ],
        )
        .unwrap();
}

#[test]
fn leasing_projects_canonical_entities_in_dependency_order_and_ignores_stale_payloads() {
    let mut connection = database();
    insert_profile(&connection, PROFILE_ID, ACCOUNT_ID, "真实档案名");
    for (id, hash, path, created) in [
        (QUESTION_ASSET_ID, "a".repeat(64), "aa/question.enc", 30_i64),
        (ANSWER_ASSET_ID, "b".repeat(64), "bb/answer.enc", 31_i64),
    ] {
        connection.execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
             VALUES(?1, ?2, ?3, ?4, 321, 'image/png', ?5)",
            params![id, ACCOUNT_ID, hash, path, created],
        ).unwrap();
    }
    connection.execute(
        "INSERT INTO problems(
             id, account_id, profile_id, subject, tags_json, note, status, time_limit_seconds,
             created_at_utc_ms, updated_at_utc_ms, revision
         ) VALUES(?1, ?2, ?3, '数学', '[\"圆锥曲线\"]', '规范化表中的笔记', 'active', 90, 40, 50, 3)",
        params![PROBLEM_ID, ACCOUNT_ID, PROFILE_ID],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position)
         VALUES(?1, ?2, 'answer', 0), (?1, ?3, 'question', 0)",
            params![PROBLEM_ID, ANSWER_ASSET_ID, QUESTION_ASSET_ID],
        )
        .unwrap();

    let problem_operation = Uuid::now_v7().to_string();
    let profile_operation = Uuid::now_v7().to_string();
    let answer_operation = Uuid::now_v7().to_string();
    let question_operation = Uuid::now_v7().to_string();
    insert_operation(
        &connection,
        &problem_operation,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "problem",
        PROBLEM_ID,
        "upsert",
        1,
    );
    insert_operation(
        &connection,
        &profile_operation,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "learner_profile",
        PROFILE_ID,
        "upsert",
        99,
    );
    insert_operation(
        &connection,
        &answer_operation,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "asset",
        ANSWER_ASSET_ID,
        "upsert",
        3,
    );
    insert_operation(
        &connection,
        &question_operation,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "asset",
        QUESTION_ASSET_ID,
        "upsert",
        4,
    );

    let batch = lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 1_000, 100).unwrap();

    assert_eq!(
        batch
            .operations
            .iter()
            .map(|operation| operation.entity_type.as_str())
            .collect::<Vec<_>>(),
        ["learner_profile", "asset", "asset", "problem"]
    );
    let WireEntity::LearnerProfile(profile) = &batch.operations[0].payload else {
        panic!("profile payload")
    };
    assert_eq!(profile.name, "真实档案名");
    let WireEntity::Problem(problem) = &batch.operations[3].payload else {
        panic!("problem payload")
    };
    assert_eq!(problem.note, "规范化表中的笔记");
    assert_eq!(problem.tags, ["圆锥曲线"]);
    assert_eq!(problem.assets[0].asset_id, QUESTION_ASSET_ID);
    assert_eq!(problem.assets[1].asset_id, ANSWER_ASSET_ID);
    assert_eq!(batch.assets.len(), 2);
    assert!(
        batch
            .assets
            .iter()
            .all(|asset| asset.storage_object.starts_with(REMOTE_USER_ID))
    );
    let json = serde_json::to_string(&batch.operations).unwrap();
    assert!(!json.contains("stale"));
    assert!(!json.contains("captureBatch"));
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM sync_operations WHERE status = 'processing' AND lease_id = ?1",
            [&batch.lease_id],
            |row| row.get::<_, i64>(0),
        ).unwrap(),
        4
    );
}

#[test]
fn leasing_caps_batches_and_recovers_expired_leases_without_wrong_lease_acknowledgement() {
    let mut connection = database();
    let mut operation_ids = Vec::new();
    for index in 0..105 {
        let profile_id = Uuid::now_v7().to_string();
        let operation_id = Uuid::now_v7().to_string();
        insert_profile(
            &connection,
            &profile_id,
            ACCOUNT_ID,
            &format!("profile-{index}"),
        );
        insert_operation(
            &connection,
            &operation_id,
            ACCOUNT_ID,
            Some(&profile_id),
            "learner_profile",
            &profile_id,
            "upsert",
            index,
        );
        operation_ids.push(operation_id);
    }

    let first = lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 10_000, 500).unwrap();
    assert_eq!(first.operations.len(), 100);
    let wrong_lease = Uuid::now_v7().to_string();
    assert_eq!(
        acknowledge_push_batch(&mut connection, &wrong_lease, &operation_ids[..100]).unwrap(),
        0
    );
    assert_eq!(
        acknowledge_push_batch(
            &mut connection,
            &first.lease_id,
            &first
                .operations
                .iter()
                .map(|operation| operation.operation_id.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
        100
    );

    let second =
        lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 10_001, 100).unwrap();
    assert_eq!(second.operations.len(), 5);
    let while_leased =
        lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 10_002, 100).unwrap();
    assert!(while_leased.operations.is_empty());
    let recovered = lease_push_batch(
        &mut connection,
        ACCOUNT_ID,
        REMOTE_USER_ID,
        second.lease_expires_at_utc_ms,
        100,
    )
    .unwrap();
    assert_eq!(recovered.operations.len(), 5);
    assert_ne!(recovered.lease_id, second.lease_id);
}

#[test]
fn failed_lease_returns_to_pending_with_bounded_backoff() {
    let mut connection = database();
    insert_profile(&connection, PROFILE_ID, ACCOUNT_ID, "profile");
    let operation_id = Uuid::now_v7().to_string();
    insert_operation(
        &connection,
        &operation_id,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "learner_profile",
        PROFILE_ID,
        "upsert",
        1,
    );
    let batch = lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 50_000, 100).unwrap();

    assert_eq!(
        fail_push_batch(&mut connection, &batch.lease_id, "network", 50_100).unwrap(),
        1
    );
    let state: (String, i64, i64, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT status, attempt_count, next_attempt_at_utc_ms, lease_id, last_error_code
         FROM sync_operations WHERE id = ?1",
            [&operation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(state.0, "pending");
    assert_eq!(state.1, 1);
    assert!((60_100..=61_100).contains(&state.2));
    assert_eq!(state.3, None);
    assert_eq!(state.4.as_deref(), Some("network"));
    assert!(
        lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 60_099, 100)
            .unwrap()
            .operations
            .is_empty()
    );
    assert_eq!(
        lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, state.2, 100)
            .unwrap()
            .operations
            .len(),
        1
    );
}

#[test]
fn forged_cross_account_operation_rolls_back_without_leasing_anything() {
    let mut connection = database();
    insert_profile(&connection, PROFILE_ID, OTHER_ACCOUNT_ID, "foreign");
    let operation_id = Uuid::now_v7().to_string();
    insert_operation(
        &connection,
        &operation_id,
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "learner_profile",
        PROFILE_ID,
        "upsert",
        1,
    );

    assert!(matches!(
        lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 1_000, 100),
        Err(SyncStoreError::MissingCanonicalEntity)
    ));
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM sync_operations WHERE id = ?1",
                [&operation_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "pending"
    );
}

#[test]
fn review_export_and_delete_operations_are_rebuilt_from_canonical_rows() {
    let mut connection = database();
    insert_profile(&connection, PROFILE_ID, ACCOUNT_ID, "profile");
    connection
        .execute(
            "INSERT INTO problems(
             id, account_id, profile_id, subject, tags_json, note, status,
             created_at_utc_ms, updated_at_utc_ms, revision
         ) VALUES(?1, ?2, ?3, '物理', '[]', '', 'trashed', 1, 2, 4)",
            params![PROBLEM_ID, ACCOUNT_ID, PROFILE_ID],
        )
        .unwrap();
    let event_id = Uuid::now_v7().to_string();
    let device_id = Uuid::now_v7().to_string();
    connection
        .execute(
            "INSERT INTO review_events(
             id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
             occurred_at_utc_ms, algorithm_version, parameter_version
         ) VALUES(?1, ?2, ?3, ?4, ?5, 'good', 4321, 77, 'fsrs-6', 'params-1')",
            params![event_id, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID, device_id],
        )
        .unwrap();
    let export_id = Uuid::now_v7().to_string();
    connection
        .execute(
            "INSERT INTO export_snapshots(
             id, account_id, profile_id, title, problem_ids_json, configuration_json,
             created_at_utc_ms, revision
         ) VALUES(?1, ?2, ?3, '错题周报', ?4, '{\"layout\":\"alternating\"}', 88, 2)",
            params![
                export_id,
                ACCOUNT_ID,
                PROFILE_ID,
                serde_json::to_string(&[PROBLEM_ID]).unwrap()
            ],
        )
        .unwrap();
    let tombstone_id = Uuid::now_v7().to_string();
    connection
        .execute(
            "INSERT INTO tombstones(
             id, account_id, profile_id, entity_type, entity_id, deleted_at_utc_ms,
             purge_after_utc_ms, revision
         ) VALUES(?1, ?2, ?3, 'problem', ?4, 90, 999999, 4)",
            params![tombstone_id, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    insert_operation(
        &connection,
        &Uuid::now_v7().to_string(),
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "review_event",
        &event_id,
        "upsert",
        1,
    );
    insert_operation(
        &connection,
        &Uuid::now_v7().to_string(),
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "export_snapshot",
        &export_id,
        "upsert",
        2,
    );
    insert_operation(
        &connection,
        &Uuid::now_v7().to_string(),
        ACCOUNT_ID,
        Some(PROFILE_ID),
        "problem",
        PROBLEM_ID,
        "delete",
        3,
    );

    let batch = lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 1_000, 100).unwrap();
    assert_eq!(
        batch
            .operations
            .iter()
            .map(|item| item.entity_type.as_str())
            .collect::<Vec<_>>(),
        ["review_event", "export_snapshot", "problem"]
    );
    let WireEntity::ReviewEvent(event) = &batch.operations[0].payload else {
        panic!("review event")
    };
    assert_eq!(event.duration_ms, 4321);
    assert_eq!(event.device_id, device_id);
    let WireEntity::ExportSnapshot(snapshot) = &batch.operations[1].payload else {
        panic!("export")
    };
    assert_eq!(snapshot.problem_ids, [PROBLEM_ID]);
    assert_eq!(snapshot.configuration["layout"], "alternating");
    let WireEntity::Tombstone(tombstone) = &batch.operations[2].payload else {
        panic!("tombstone")
    };
    assert_eq!(tombstone.tombstone_id, tombstone_id);
    assert_eq!(tombstone.deleted_revision, 4);
}

#[test]
fn remote_binding_is_permanent_cursor_is_monotonic_and_expired_transfers_are_removed() {
    let mut connection = database();
    assert_eq!(pull_cursor(&connection, ACCOUNT_ID).unwrap(), 0);
    record_pull_success(&connection, ACCOUNT_ID, 12, 100).unwrap();
    record_pull_success(&connection, ACCOUNT_ID, 7, 110).unwrap();
    assert_eq!(pull_cursor(&connection, ACCOUNT_ID).unwrap(), 12);

    lease_push_batch(&mut connection, ACCOUNT_ID, REMOTE_USER_ID, 120, 100).unwrap();
    assert!(matches!(
        lease_push_batch(
            &mut connection,
            ACCOUNT_ID,
            "44444444-4444-4444-8444-444444444444",
            130,
            100,
        ),
        Err(SyncStoreError::RemoteBindingMismatch)
    ));
    let fingerprint: String = connection
        .query_row(
            "SELECT remote_user_fingerprint FROM cloud_sync_state WHERE account_id = ?1",
            [ACCOUNT_ID],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fingerprint.len(), 64);
    assert!(!fingerprint.contains(REMOTE_USER_ID));

    connection.execute(
        "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms)
         VALUES(?1, ?2, ?3, 'aa/question.enc', 10, 'image/png', 1)",
        params![QUESTION_ASSET_ID, ACCOUNT_ID, "c".repeat(64)],
    ).unwrap();
    connection.execute(
        "INSERT INTO cloud_asset_transfers(asset_id, upload_url, confirmed_offset, expires_at_utc_ms, updated_at_utc_ms)
         VALUES(?1, 'https://example.invalid/expired', 0, 199, 1)",
        [QUESTION_ASSET_ID],
    ).unwrap();
    assert_eq!(
        discard_expired_asset_transfers(&connection, 200).unwrap(),
        1
    );
}
