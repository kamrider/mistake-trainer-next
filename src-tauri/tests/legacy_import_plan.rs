use std::fs;

use mistake_trainer_next_lib::modules::legacy::{
    LegacyRating, build_legacy_import_plan, legacy_tree_fingerprint,
};
use tempfile::tempdir;

#[test]
fn real_legacy_fields_form_ordered_problem_groups_and_safe_public_report() {
    let directory = tempdir().unwrap();
    let member = directory.path().join("members").join("alice");
    let files = member.join("files");
    fs::create_dir_all(&files).unwrap();
    for (name, bytes) in [
        ("question.png", b"question".as_slice()),
        ("answer-1.png", b"answer one".as_slice()),
        ("answer-2.png", b"answer two".as_slice()),
        ("solo.png", b"solo question".as_slice()),
        ("orphan.png", b"orphan answer".as_slice()),
    ] {
        fs::write(files.join(name), bytes).unwrap();
    }
    fs::write(
        member.join(".metadata.json"),
        r#"{
          "version":"1.1",
          "files":{
            "q":{"id":"q","relativePath":"question.png","type":"mistake","pairId":"pair-1","subject":"数学","tags":["圆锥曲线","期中"],"notes":"检查离心率","answerTimeLimit":180,"proficiency":40,"trainingInterval":10,"nextTrainingDate":"2026-07-25T00:00:00.000Z","isFrozen":true,"trainingRecords":[{"date":"2026-07-10T00:00:00.000Z","result":"success","answerTime":42000},{"date":"bad-date","result":"fail"}]},
            "a1":{"id":"a1","relativePath":"answer-1.png","type":"answer","pairId":"pair-1"},
            "a2":{"id":"a2","relativePath":"answer-2.png","type":"answer","pairId":"pair-1"},
            "solo":{"id":"solo","relativePath":"solo.png","type":"mistake","subject":"物理"},
            "orphan":{"id":"orphan","relativePath":"orphan.png","type":"answer","pairId":"missing-question"}
          }
        }"#,
    )
    .unwrap();

    let plan = build_legacy_import_plan(directory.path()).unwrap();

    assert_eq!(plan.members.len(), 1);
    assert_eq!(plan.members[0].name, "alice");
    assert_eq!(plan.members[0].problems.len(), 2);
    let paired = plan.members[0]
        .problems
        .iter()
        .find(|problem| problem.source_problem_key == "pair-1")
        .unwrap();
    assert_eq!(paired.question_assets.len(), 1);
    assert_eq!(paired.answer_assets.len(), 2);
    assert_eq!(paired.subject, "数学");
    assert_eq!(paired.tags, ["圆锥曲线", "期中"]);
    assert_eq!(paired.note, "检查离心率");
    assert_eq!(paired.time_limit_seconds, Some(180));
    assert!(paired.frozen);
    assert_eq!(paired.reviews.len(), 1, "invalid review dates are skipped");
    assert_eq!(paired.reviews[0].rating, LegacyRating::Good);
    assert_eq!(paired.reviews[0].duration_ms, 42_000);
    assert_eq!(paired.due_at_utc_ms, Some(1_784_937_600_000));
    assert_eq!(paired.stability_days, 10.0);
    assert!((paired.difficulty - 6.4).abs() < f64::EPSILON);
    assert!(
        plan.report
            .issues
            .iter()
            .any(|issue| issue.code == "orphan_answer")
    );
    assert!(
        plan.report
            .issues
            .iter()
            .any(|issue| issue.code == "invalid_training_date")
    );
    assert!(
        plan.report
            .issues
            .iter()
            .filter(|issue| matches!(issue.record_id.as_deref(), Some("q" | "a1" | "a2")))
            .all(|issue| issue.code != "missing_pair"),
        "a shared pairId with both question and answers is a complete pair"
    );
    let public = serde_json::to_string(&plan.public_report()).unwrap();
    assert!(!public.contains(directory.path().to_string_lossy().as_ref()));
    assert!(!public.contains("question.png"));
}

#[test]
fn source_fingerprint_changes_with_contents_without_modifying_the_tree() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join(".metadata.json"), r#"{"files":{}}"#).unwrap();
    let before_bytes = fs::read(directory.path().join(".metadata.json")).unwrap();
    let first = legacy_tree_fingerprint(directory.path()).unwrap();
    assert_eq!(
        fs::read(directory.path().join(".metadata.json")).unwrap(),
        before_bytes
    );
    fs::write(directory.path().join("extra.bin"), b"changed").unwrap();
    let second = legacy_tree_fingerprint(directory.path()).unwrap();
    assert_ne!(first, second);
}

#[test]
fn fingerprint_rejects_excessive_directory_depth() {
    let directory = tempdir().unwrap();
    let mut nested = directory.path().to_path_buf();
    for _ in 0..33 {
        nested.push("d");
        fs::create_dir(&nested).unwrap();
    }
    fs::write(nested.join("asset.bin"), b"content").unwrap();

    let error = legacy_tree_fingerprint(directory.path())
        .expect_err("directory nesting beyond the scan budget must fail");
    assert!(error.to_string().contains("too deeply nested"));
}
