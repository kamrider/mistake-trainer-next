use std::fs;

use mistake_trainer_next_lib::modules::legacy::{LegacyImportError, LegacyImportManager};
use tempfile::tempdir;
use uuid::Uuid;

fn legacy_root_with_question(name: &str) -> tempfile::TempDir {
    let root = tempdir().unwrap();
    let member = root.path().join("members").join(name);
    fs::create_dir_all(member.join("files")).unwrap();
    fs::write(
        member.join("files").join("question.png"),
        b"opaque-test-bytes",
    )
    .unwrap();
    fs::write(
        member.join(".metadata.json"),
        r#"{"files":{"q":{"id":"q","relativePath":"question.png","type":"mistake","subject":"数学"}}}"#,
    )
    .unwrap();
    root
}

#[test]
fn candidate_is_opaque_expires_and_never_serializes_source_details() {
    let root = legacy_root_with_question("private-student");
    let manager = LegacyImportManager::default();
    let candidate = manager.prepare(root.path(), 1_000).unwrap();
    let parsed = Uuid::parse_str(&candidate.candidate_id).unwrap();
    assert_eq!(parsed.get_version_num(), 7);
    assert_eq!(candidate.expires_at_utc_ms, 1_801_000.0);

    let serialized = serde_json::to_string(&candidate).unwrap();
    for private_value in [
        root.path().to_string_lossy().as_ref(),
        "private-student",
        "question.png",
    ] {
        assert!(!serialized.contains(private_value));
    }

    assert!(manager.plan_for(&candidate.candidate_id, 1_800_999).is_ok());
    assert!(matches!(
        manager.plan_for(&candidate.candidate_id, 1_801_000),
        Err(LegacyImportError::ImportNotFound)
    ));
}

#[test]
fn preparing_again_replaces_the_previous_candidate_and_success_can_consume_it() {
    let first_root = legacy_root_with_question("first");
    let second_root = legacy_root_with_question("second");
    let manager = LegacyImportManager::default();
    let first = manager.prepare(first_root.path(), 1_000).unwrap();
    let second = manager.prepare(second_root.path(), 2_000).unwrap();

    assert_ne!(first.candidate_id, second.candidate_id);
    assert!(matches!(
        manager.plan_for(&first.candidate_id, 2_001),
        Err(LegacyImportError::ImportNotFound)
    ));
    assert_eq!(
        manager
            .plan_for(&second.candidate_id, 2_001)
            .unwrap()
            .members[0]
            .name,
        "second"
    );

    manager.consume(&second.candidate_id);
    assert!(matches!(
        manager.plan_for(&second.candidate_id, 2_002),
        Err(LegacyImportError::ImportNotFound)
    ));
}
