use std::fs;

use mistake_trainer_next_lib::modules::legacy::scan_legacy_storage;
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
        r#"{"version":"1.1","files":{"x":{"id":"x","relativePath":"../../secret.png","originalFileName":"x.png"}}}"#,
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
