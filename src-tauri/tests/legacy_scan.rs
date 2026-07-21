use std::fs;

use mistake_trainer_next_lib::modules::legacy::{build_legacy_import_plan, scan_legacy_storage};
use tempfile::tempdir;

#[test]
fn scan_reports_missing_pairs_and_preserves_the_legacy_tree() {
    let directory = tempdir().expect("tempdir");
    let member = directory.path().join("members").join("小树");
    let files = member.join("files");
    fs::create_dir_all(&files).expect("fixture directories");
    fs::write(files.join("question.png"), b"question-image").expect("fixture image");
    fs::write(
        member.join(".metadata.json"),
        r#"{
          "version":"1.1",
          "files":{
            "question-id":{
              "id":"question-id","relativePath":"question.png","originalFileName":"q.png",
              "type":"mistake","pairId":"missing-answer","isPaired":true,"isFrozen":true,
              "trainingRecords":[{"date":"2025-01-01T00:00:00.000Z","result":"success"}]
            },
            "missing-answer":{
              "id":"missing-answer","relativePath":"answer.png","originalFileName":"a.png",
              "type":"answer","pairId":"question-id","isPaired":true
            }
          }
        }"#,
    )
    .expect("fixture metadata");
    let before = tree_fingerprint(directory.path());

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds");

    assert_eq!(report.members, 1);
    assert_eq!(report.metadata_records, 2);
    assert_eq!(report.existing_assets, 1);
    assert_eq!(report.training_records, 1);
    assert_eq!(report.frozen_records, 1);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_asset")
    );
    let absolute_root = directory.path().to_string_lossy();
    assert!(
        report
            .issues
            .iter()
            .all(|issue| !issue.detail.contains(absolute_root.as_ref())),
        "preflight issues must not expose the selected absolute path"
    );
    assert_eq!(tree_fingerprint(directory.path()), before);
}

#[test]
fn scan_reports_corrupt_metadata_instead_of_mutating_or_aborting() {
    let directory = tempdir().expect("tempdir");
    let member = directory.path().join("members").join("broken");
    fs::create_dir_all(&member).expect("fixture directories");
    fs::write(member.join(".metadata.json"), "{not-json").expect("fixture metadata");

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds with report");

    assert_eq!(report.members, 1);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "invalid_metadata")
    );
    assert_eq!(
        fs::read_to_string(member.join(".metadata.json")).unwrap(),
        "{not-json"
    );
}

#[test]
fn scan_rejects_relative_paths_that_escape_a_member_files_directory() {
    let directory = tempdir().expect("tempdir");
    let member = directory.path().join("members").join("unsafe");
    fs::create_dir_all(member.join("files")).expect("fixture directories");
    fs::write(
        member.join(".metadata.json"),
        r#"{"version":"1.1","files":{"x":{"id":"C:\\Users\\private","relativePath":"../../secret.png","originalFileName":"x.png"}}}"#,
    )
    .expect("fixture metadata");

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds with report");

    assert_eq!(report.existing_assets, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "unsafe_relative_path")
    );
    assert!(
        report
            .issues
            .iter()
            .all(|issue| !issue.detail.contains("secret.png")),
        "untrusted relative paths must not be echoed into the report"
    );
    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.record_id.as_deref() != Some("C:\\Users\\private")),
        "path-like record identifiers must be redacted"
    );
}

#[test]
fn scan_rejects_oversized_metadata_before_reading_or_parsing_it() {
    let directory = tempdir().expect("tempdir");
    let member = directory.path().join("members").join("oversized");
    fs::create_dir_all(member.join("files")).expect("fixture directories");
    let metadata = fs::File::create(member.join(".metadata.json")).expect("fixture metadata");
    metadata
        .set_len(16 * 1024 * 1024 + 1)
        .expect("sparse oversized metadata");

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds with report");

    assert_eq!(report.metadata_records, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "metadata_too_large")
    );
}

#[test]
fn scan_rejects_an_oversized_asset_without_counting_it_as_readable() {
    let directory = tempdir().expect("tempdir");
    let member = directory.path().join("members").join("large-asset");
    let files = member.join("files");
    fs::create_dir_all(&files).expect("fixture directories");
    let asset = fs::File::create(files.join("large.png")).expect("fixture asset");
    asset
        .set_len(64 * 1024 * 1024 + 1)
        .expect("sparse oversized asset");
    fs::write(
        member.join(".metadata.json"),
        r#"{"files":{"large":{"id":"large","relativePath":"large.png"}}}"#,
    )
    .expect("fixture metadata");

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds with report");

    assert_eq!(report.existing_assets, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "asset_too_large")
    );
}

#[cfg(windows)]
#[test]
fn scan_rejects_a_member_junction_that_resolves_outside_the_selected_root() {
    use std::process::{Command, Stdio};

    let directory = tempdir().expect("tempdir");
    let outside = tempdir().expect("outside tempdir");
    let members = directory.path().join("members");
    fs::create_dir_all(&members).expect("members directory");
    fs::write(outside.path().join(".metadata.json"), r#"{"files":{}}"#).expect("outside metadata");
    let junction = members.join("escaped");
    let status = Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(&junction)
        .arg(outside.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("create junction fixture");
    assert!(status.success(), "junction fixture must be created");

    let report = scan_legacy_storage(directory.path()).expect("scan succeeds with report");

    assert_eq!(report.members, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "unsafe_member_path")
    );
}

#[cfg(windows)]
#[test]
fn candidate_build_rejects_an_internal_windows_junction_before_recursing() {
    use std::process::Command;

    let directory = tempdir().unwrap();
    let target = directory.path().join("real-member");
    fs::create_dir_all(target.join("files")).unwrap();
    fs::write(target.join(".metadata.json"), r#"{"files":{}}"#).unwrap();
    let junction = directory.path().join("members").join("linked-member");
    fs::create_dir_all(junction.parent().unwrap()).unwrap();
    let status = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .status()
        .expect("create internal junction fixture");
    assert!(status.success(), "junction fixture must be created");

    assert!(build_legacy_import_plan(directory.path()).is_err());
}

fn tree_fingerprint(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn collect(
        root: &std::path::Path,
        current: &std::path::Path,
        output: &mut Vec<(String, Vec<u8>)>,
    ) {
        let mut entries = fs::read_dir(current)
            .expect("read fixture directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture entries");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, output);
            } else {
                output.push((
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).expect("read fixture file"),
                ));
            }
        }
    }

    let mut output = Vec::new();
    collect(root, root, &mut output);
    output
}
