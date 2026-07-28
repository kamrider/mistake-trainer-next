use mistake_trainer_next_lib::{
    infrastructure::database::run_migrations,
    modules::sync_conflicts::{
        ResolveSyncConflictEntityInput, ResolveSyncConflictFieldInput, SyncConflictChoice,
        SyncConflictError, resolve_sync_conflict_entity, resolve_sync_conflict_field,
    },
};
use rusqlite::{Connection, params};
use serde_json::json;

const ACCOUNT_ID: &str = "0191365e-2f2f-7b89-b3b0-111111111111";
const PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-222222222222";
const PROBLEM_ID: &str = "0191365e-2f2f-7b89-b3b0-333333333333";
const CONFLICT_ID: &str = "0191365e-2f2f-7b89-b3b0-444444444444";

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    run_migrations(&mut connection).unwrap();
    connection
        .execute(
            "INSERT INTO learner_profiles(
               id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES(?1, ?2, '我的档案', 1, 1, 1)",
            params![PROFILE_ID, ACCOUNT_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO problems(
               id, account_id, profile_id, subject, tags_json, note, status,
               created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES(?1, ?2, ?3, '数学', '[]', 'local note', 'active', 2, 3, 3)",
            params![PROBLEM_ID, ACCOUNT_ID, PROFILE_ID],
        )
        .unwrap();
    let remote = json!({
        "id": PROBLEM_ID,
        "profileId": PROFILE_ID,
        "subject": "数学",
        "tags": [],
        "note": "remote note",
        "status": "active",
        "timeLimitSeconds": null,
        "assets": [],
        "revision": 2,
        "createdAtUtcMs": 2,
        "updatedAtUtcMs": 3
    });
    connection
        .execute(
            "INSERT INTO sync_entity_snapshots(
               account_id, profile_id, entity_type, entity_id, revision,
               payload_json, updated_at_utc_ms
             ) VALUES(?1, ?2, 'problem', ?3, 2, ?4, 3)",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID, remote.to_string()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               ?1, ?2, ?3, 'problem', ?4, 'note',
               '\"local note\"', '\"remote note\"', 1, 4
             )",
            params![CONFLICT_ID, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
}

#[test]
fn choosing_remote_converges_to_the_snapshot_without_an_extra_push() {
    let mut connection = database();
    let remaining = resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Remote,
        },
        10,
    )
    .unwrap();
    assert!(remaining.is_empty());
    assert_eq!(
        connection
            .query_row(
                "SELECT note, revision FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        ("remote note".to_owned(), 2)
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn choosing_local_creates_one_new_canonical_revision_and_keeps_the_audit() {
    let mut connection = database();
    for (id, operation, created_at) in [
        ("0191365e-2f2f-7b89-b3b0-888888888881", "delete", 7_i64),
        ("0191365e-2f2f-7b89-b3b0-888888888882", "restore", 8_i64),
    ] {
        connection
            .execute(
                "INSERT INTO sync_operations(
                   id, account_id, profile_id, entity_type, entity_id, operation,
                   payload_json, status, attempt_count, created_at_utc_ms,
                   next_attempt_at_utc_ms
                 ) VALUES(?1, ?2, ?3, 'problem', ?4, ?5, '{}', 'pending', 0, ?6, ?6)",
                params![
                    id, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID, operation, created_at
                ],
            )
            .unwrap();
    }
    resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Local,
        },
        10,
    )
    .unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT note, revision FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        ("local note".to_owned(), 4)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*), MIN(operation) FROM sync_operations
                 WHERE entity_type = 'problem' AND entity_id = ?1",
                [PROBLEM_ID],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap(),
        (1, "upsert".to_owned())
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT resolution, resolved_value_json, resolved_at_utc_ms
                 FROM sync_conflicts WHERE id = ?1",
                [CONFLICT_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap(),
        ("local".to_owned(), "\"local note\"".to_owned(), 10)
    );
}

#[test]
fn a_conflict_id_from_another_profile_is_indistinguishable_from_missing() {
    let mut connection = database();
    let error = resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        "0191365e-2f2f-7b89-b3b0-999999999999",
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Local,
        },
        10,
    )
    .unwrap_err();
    assert!(matches!(error, SyncConflictError::NotFound));
}

#[test]
fn entity_resolution_applies_every_field_in_one_transaction() {
    let mut connection = database();
    connection
        .execute(
            "UPDATE problems SET subject = '物理' WHERE id = ?1",
            [PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-555555555555', ?1, ?2, 'problem', ?3,
               'subject', '\"物理\"', '\"数学\"', 1, 5
             )",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();

    let remaining = resolve_sync_conflict_entity(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictEntityInput {
            entity_type: "problem".to_owned(),
            entity_id: PROBLEM_ID.to_owned(),
            choice: SyncConflictChoice::Remote,
        },
        10,
    )
    .unwrap();

    assert!(remaining.is_empty());
    assert_eq!(
        connection
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
            .unwrap(),
        ("数学".to_owned(), "remote note".to_owned(), 2)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts
                 WHERE entity_id = ?1 AND resolution = 'remote'",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn invalid_selected_value_rolls_back_the_resolution_audit() {
    let mut connection = database();
    connection
        .execute(
            "UPDATE sync_conflicts
             SET field_name = 'subject', local_value_json = ?1
             WHERE id = ?2",
            params![format!("\"{}\"", "太".repeat(41)), CONFLICT_ID],
        )
        .unwrap();

    let error = resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Local,
        },
        10,
    )
    .unwrap_err();

    assert!(matches!(error, SyncConflictError::InvalidValue));
    assert_eq!(
        connection
            .query_row(
                "SELECT subject, resolution, resolved_at_utc_ms
                 FROM problems, sync_conflicts
                 WHERE problems.id = ?1 AND sync_conflicts.id = ?2",
                params![PROBLEM_ID, CONFLICT_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .unwrap(),
        ("数学".to_owned(), None, None)
    );
}

#[test]
fn keeping_a_local_entity_after_remote_deletion_advances_above_the_tombstone() {
    let mut connection = database();
    connection
        .execute("DELETE FROM sync_conflicts", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO tombstones(
               id, account_id, profile_id, entity_type, entity_id,
               deleted_at_utc_ms, purge_after_utc_ms, revision
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-666666666666', ?1, ?2, 'problem', ?3,
               6, 2592000006, 7
             )",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               ?1, ?2, ?3, 'problem', ?4, '__deleted__',
               ?5, 'true', 2, 7
             )",
            params![
                CONFLICT_ID,
                ACCOUNT_ID,
                PROFILE_ID,
                PROBLEM_ID,
                json!({
                    "subject": "数学",
                    "tags": [],
                    "note": "local note",
                    "status": "active",
                    "timeLimitSeconds": null,
                    "assets": []
                })
                .to_string()
            ],
        )
        .unwrap();

    resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Local,
        },
        10,
    )
    .unwrap();

    assert_eq!(
        connection
            .query_row(
                "SELECT status, revision FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        ("active".to_owned(), 8)
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM tombstones", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT operation FROM sync_operations WHERE entity_id = ?1",
                [PROBLEM_ID],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "upsert"
    );
}

#[test]
fn accepting_remote_deletion_keeps_the_recycle_tombstone_and_clears_pushes() {
    let mut connection = database();
    connection
        .execute("DELETE FROM sync_conflicts", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO tombstones(
               id, account_id, profile_id, entity_type, entity_id,
               deleted_at_utc_ms, purge_after_utc_ms, revision
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-777777777777', ?1, ?2, 'problem', ?3,
               6, 2592000006, 7
             )",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES(
               ?1, ?2, ?3, 'problem', ?4, '__deleted__',
               '{}', 'true', 2, 7
             )",
            params![CONFLICT_ID, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();

    resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Remote,
        },
        10,
    )
    .unwrap();

    assert_eq!(
        connection
            .query_row(
                "SELECT status, revision FROM problems WHERE id = ?1",
                [PROBLEM_ID],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        ("trashed".to_owned(), 7)
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM tombstones", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn accepting_remote_deletion_resolves_sibling_field_conflicts_together() {
    let mut connection = database();
    connection
        .execute("DELETE FROM sync_conflicts", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO tombstones(
               id, account_id, profile_id, entity_type, entity_id,
               deleted_at_utc_ms, purge_after_utc_ms, revision
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-999999999991', ?1, ?2, 'problem', ?3,
               6, 2592000006, 7
             )",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES
               (?1, ?2, ?3, 'problem', ?4, '__deleted__', '{}', 'true', 2, 7),
               ('0191365e-2f2f-7b89-b3b0-999999999992', ?2, ?3, 'problem', ?4,
                'subject', '\"物理\"', '\"数学\"', 2, 8)",
            params![CONFLICT_ID, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();

    let remaining = resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Remote,
        },
        10,
    )
    .unwrap();

    assert!(remaining.is_empty());
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts
                 WHERE entity_id = ?1 AND resolved_at_utc_ms IS NULL",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts
                 WHERE entity_id = ?1 AND resolution = 'remote'",
                [PROBLEM_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
}

#[test]
fn accepting_remote_profile_deletion_cleans_child_sync_state_without_erasing_audit() {
    const REPLACEMENT_PROFILE_ID: &str = "0191365e-2f2f-7b89-b3b0-999999999981";
    const PROFILE_CONFLICT_ID: &str = "0191365e-2f2f-7b89-b3b0-999999999982";
    let mut connection = database();
    connection
        .execute("DELETE FROM sync_conflicts", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO learner_profiles(
               id, account_id, name, created_at_utc_ms, updated_at_utc_ms, revision
             ) VALUES(?1, ?2, '保留档案', 9, 9, 1)",
            params![REPLACEMENT_PROFILE_ID, ACCOUNT_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO account_preferences(account_id, active_profile_id, updated_at_utc_ms)
             VALUES(?1, ?2, 9)",
            params![ACCOUNT_ID, PROFILE_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_operations(
               id, account_id, profile_id, entity_type, entity_id, operation,
               payload_json, status, attempt_count, created_at_utc_ms,
               next_attempt_at_utc_ms
             ) VALUES(
               '0191365e-2f2f-7b89-b3b0-999999999983', ?1, ?2, 'problem', ?3,
               'upsert', '{}', 'pending', 0, 7, 7
             )",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tombstones(
               id, account_id, profile_id, entity_type, entity_id,
               deleted_at_utc_ms, purge_after_utc_ms, revision
             ) VALUES
               ('0191365e-2f2f-7b89-b3b0-999999999984', ?1, NULL,
                'learner_profile', ?2, 8, 2592000008, 7),
               ('0191365e-2f2f-7b89-b3b0-999999999985', ?1, ?2,
                'problem', ?3, 7, 2592000007, 6)",
            params![ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_conflicts(
               id, account_id, profile_id, entity_type, entity_id, field_name,
               local_value_json, remote_value_json, base_revision, created_at_utc_ms
             ) VALUES
               (?1, ?2, ?3, 'learner_profile', ?3, '__deleted__',
                '{\"name\":\"我的档案\"}', 'true', 1, 8),
               ('0191365e-2f2f-7b89-b3b0-999999999986', ?2, ?3, 'problem', ?4,
                'note', '\"本机笔记\"', '\"云端笔记\"', 2, 7),
               ('0191365e-2f2f-7b89-b3b0-999999999987', ?2, ?3, 'problem', ?4,
                'subject', '\"数学\"', '\"物理\"', 2, 6)",
            params![PROFILE_CONFLICT_ID, ACCOUNT_ID, PROFILE_ID, PROBLEM_ID],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE sync_conflicts
             SET resolution = 'local', resolved_value_json = '\"数学\"',
                 resolved_at_utc_ms = 6
             WHERE id = '0191365e-2f2f-7b89-b3b0-999999999987'",
            [],
        )
        .unwrap();

    let remaining = resolve_sync_conflict_field(
        &mut connection,
        ACCOUNT_ID,
        PROFILE_ID,
        ResolveSyncConflictFieldInput {
            conflict_id: PROFILE_CONFLICT_ID.to_owned(),
            choice: SyncConflictChoice::Remote,
        },
        10,
    )
    .unwrap();

    assert!(remaining.is_empty());
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
    for table in ["sync_operations", "sync_entity_snapshots"] {
        let count = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE profile_id = ?1"),
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(count, 0, "{table} must not retain deleted-profile state");
    }
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM tombstones
                 WHERE account_id = ?1 AND profile_id = ?2",
                params![ACCOUNT_ID, PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM tombstones
                 WHERE entity_type = 'learner_profile' AND entity_id = ?1",
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1,
        "the accepted remote profile tombstone remains authoritative"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts WHERE profile_id = ?1",
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        3,
        "both existing child audit rows and the profile decision are retained"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sync_conflicts
                 WHERE profile_id = ?1 AND resolved_at_utc_ms IS NULL",
                [PROFILE_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT resolution, resolved_value_json
                 FROM sync_conflicts
                 WHERE id = '0191365e-2f2f-7b89-b3b0-999999999986'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap(),
        ("remote".to_owned(), "null".to_owned())
    );
}
