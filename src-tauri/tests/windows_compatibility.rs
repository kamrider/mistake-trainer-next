use mistake_trainer_next_lib::modules::windows_compatibility::{
    MINIMUM_WINDOWS_BUILD, WindowsCompatibilityFacts, WindowsSupportLevel,
    assess_windows_compatibility,
};

fn facts(build_number: u32, process_architecture: &str) -> WindowsCompatibilityFacts {
    WindowsCompatibilityFacts {
        os_name: "Windows".to_owned(),
        display_version: "test".to_owned(),
        build_number,
        update_build_revision: 1,
        process_architecture: process_architecture.to_owned(),
        native_architecture: process_architecture.to_owned(),
        webview2_version: Some("140.0.0.0".to_owned()),
    }
}

#[test]
fn windows_11_x64_is_fully_supported() {
    let mut input = facts(22_631, "x86_64");
    input.os_name = "Windows 10 Pro".to_owned();
    let status = assess_windows_compatibility(input);

    assert_eq!(status.support_level, WindowsSupportLevel::Supported);
    assert!(status.supported);
    assert_eq!(status.os_name, "Windows 11 Pro");
    assert_eq!(status.minimum_windows_build, MINIMUM_WINDOWS_BUILD);
    assert!(status.summary.contains("Windows 11"));
}

#[test]
fn windows_10_22h2_and_ltsc_are_extended_compatibility() {
    for build_number in [19_045, 17_763] {
        let status = assess_windows_compatibility(facts(build_number, "x86_64"));

        assert_eq!(status.support_level, WindowsSupportLevel::Extended);
        assert!(status.supported);
        assert!(status.summary.contains("延伸兼容"));
    }
}

#[test]
fn native_windows_11_arm64_is_extended_compatibility() {
    let status = assess_windows_compatibility(facts(26_100, "arm64"));

    assert_eq!(status.support_level, WindowsSupportLevel::Extended);
    assert!(status.supported);
    assert!(status.summary.contains("ARM64 原生版本"));
}

#[test]
fn old_windows_and_32_bit_processes_are_unsupported() {
    for candidate in [
        facts(17_762, "x86_64"),
        facts(19_045, "arm64"),
        facts(22_631, "x86"),
    ] {
        let status = assess_windows_compatibility(candidate);

        assert_eq!(status.support_level, WindowsSupportLevel::Unsupported);
        assert!(!status.supported);
    }
}

#[test]
fn missing_webview_is_reported_without_changing_os_classification() {
    let mut candidate = facts(22_631, "x86_64");
    candidate.webview2_version = None;

    let status = assess_windows_compatibility(candidate);

    assert_eq!(status.support_level, WindowsSupportLevel::Supported);
    assert!(status.supported);
    assert_eq!(status.webview2_version, None);
}

#[test]
fn public_status_contains_no_registry_paths_or_machine_identity() {
    let status = assess_windows_compatibility(facts(26_100, "x86_64"));
    let serialized = serde_json::to_string(&status).unwrap();

    for forbidden in [
        "HKEY_",
        "SOFTWARE\\",
        "ComputerName",
        "MachineGuid",
        "ProductId",
        "RegisteredOwner",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}
