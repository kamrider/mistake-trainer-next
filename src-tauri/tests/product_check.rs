use mistake_trainer_next_lib::modules::product_check::{
    parse_windows_product_check_request, write_windows_product_check,
};

#[test]
fn installed_product_check_proves_the_offline_learning_lifecycle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let scratch = directory.path().join("scratch");
    std::fs::create_dir(&scratch).expect("scratch");
    let output = directory.path().join("product-check.json");

    let ready = write_windows_product_check(&output, &scratch, "1.2.3", 1_700_000_000_000)
        .expect("write product check");

    assert!(ready);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).expect("report")).expect("valid JSON");
    assert_eq!(
        report,
        serde_json::json!({
            "schemaVersion": 2,
            "applicationVersion": "1.2.3",
            "checkedAtUtcMs": 1_700_000_000_000_i64,
            "ready": true,
            "failureCodes": [],
            "checks": {
                "encryptedLibrary": true,
                "problemRoundTrip": true,
                "reviewRoundTrip": true,
                "backupValidation": true,
                "backupRestore": true,
                "docxExport": true,
                "libraryReopen": true
            }
        })
    );
    assert_eq!(
        std::fs::read_dir(&scratch)
            .expect("scratch remains")
            .count(),
        0,
        "the generated product-check workspace must be removed"
    );
}

#[test]
fn installed_product_check_reports_fixed_codes_without_leaking_paths() {
    let directory = tempfile::tempdir().expect("tempdir");
    let missing_scratch = directory.path().join("private-user-path");
    let output = directory.path().join("product-check.json");

    let ready = write_windows_product_check(&output, &missing_scratch, "1.2.3", 100)
        .expect("write failure report");

    assert!(!ready);
    let contents = std::fs::read_to_string(output).expect("report");
    let report: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    assert_eq!(
        report["failureCodes"],
        serde_json::json!(["scratch_unavailable"])
    );
    assert_eq!(
        report["checks"],
        serde_json::json!({
            "encryptedLibrary": false,
            "problemRoundTrip": false,
            "reviewRoundTrip": false,
            "backupValidation": false,
            "backupRestore": false,
            "docxExport": false,
            "libraryReopen": false
        })
    );
    assert!(!contents.contains("private-user-path"));
    assert!(!contents.contains("Users"));
}

#[test]
fn installed_product_check_cli_requires_two_absolute_paths() {
    assert!(
        parse_windows_product_check_request(["--other"].map(Into::into))
            .expect("unrelated arguments")
            .is_none()
    );
    assert!(
        parse_windows_product_check_request(["--windows-product-check"].map(Into::into)).is_err()
    );
    assert!(
        parse_windows_product_check_request(
            ["--windows-product-check", "report.json", "scratch"].map(Into::into)
        )
        .is_err()
    );

    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("report.json");
    let scratch = directory.path().join("scratch");
    let request = parse_windows_product_check_request([
        "--windows-product-check".into(),
        output.clone().into_os_string(),
        scratch.clone().into_os_string(),
    ])
    .expect("valid request")
    .expect("product check request");
    assert_eq!(request.output_path, output);
    assert_eq!(request.scratch_root, scratch);
}

#[test]
fn desktop_binary_dispatches_product_check_without_starting_tauri() {
    let directory = tempfile::tempdir().expect("tempdir");
    let scratch = directory.path().join("scratch");
    std::fs::create_dir(&scratch).expect("scratch");
    let output = directory.path().join("product-check.json");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_mistake-trainer-next"))
        .args(["--windows-product-check"])
        .arg(&output)
        .arg(&scratch)
        .status()
        .expect("run desktop binary");

    assert!(status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output).expect("report")).expect("valid JSON");
    assert_eq!(report["ready"], true);
    assert_eq!(report["failureCodes"], serde_json::json!([]));
}
