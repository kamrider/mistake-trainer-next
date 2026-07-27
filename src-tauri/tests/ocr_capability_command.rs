use mistake_trainer_next_lib::{
    application::result::AppResult,
    modules::ocr_capability::{
        OcrCapabilityStatus, OcrComponentId, OcrComponentState, OcrRecognitionFeatureState,
        capability_status, download_client, install_component, recognition_feature_status,
        visual_split_feature_status,
    },
};
use tempfile::tempdir;

#[test]
fn capability_contract_is_optional_redacted_and_disabled_by_default() {
    let root = tempdir().unwrap();
    let status = capability_status(root.path()).unwrap();
    let response = AppResult::success(status.clone());
    let serialized = serde_json::to_string(&response).unwrap();

    assert!(!status.automatic_recognition_enabled);
    assert_eq!(status.recognition_feature, visual_split_feature_status());
    assert_eq!(
        status.recognition_feature.required_component_id,
        OcrComponentId::OpencvPreprocess
    );
    assert_eq!(status.components.len(), 3);
    assert!(!serialized.contains(root.path().to_string_lossy().as_ref()));
    assert!(!serialized.contains("https://"));
    assert!(!serialized.contains("resolve/v3.9.2"));
}

#[test]
fn typed_component_ids_have_stable_public_values() {
    assert_eq!(
        serde_json::to_string(&OcrComponentId::Ppocrv6Small).unwrap(),
        r#""ppocrv6_small""#
    );
    assert_eq!(
        serde_json::to_string(&OcrComponentId::Ppocrv6Medium).unwrap(),
        r#""ppocrv6_medium""#
    );
    assert_eq!(
        serde_json::to_string(&OcrComponentId::OpencvPreprocess).unwrap(),
        r#""opencv_preprocess""#
    );
}

#[test]
fn question_organizing_requires_both_the_evidence_gate_and_verified_small_model() {
    assert_eq!(
        recognition_feature_status(true, true, OcrComponentState::Installed).state,
        OcrRecognitionFeatureState::Ready
    );
    assert_eq!(
        recognition_feature_status(true, true, OcrComponentState::NotInstalled).state,
        OcrRecognitionFeatureState::ModelMissing
    );
    assert_eq!(
        recognition_feature_status(false, true, OcrComponentState::Installed).state,
        OcrRecognitionFeatureState::EvidenceGatePending
    );
    assert_eq!(
        recognition_feature_status(true, false, OcrComponentState::Installed).state,
        OcrRecognitionFeatureState::RuntimeMissing
    );
    assert_eq!(
        recognition_feature_status(true, false, OcrComponentState::NotInstalled).state,
        OcrRecognitionFeatureState::RuntimeMissing,
        "a missing runtime must not send the user to download an unusable model"
    );
}

#[test]
fn capability_status_exposes_model_free_visual_splitting_without_enabling_full_recognition() {
    let root = tempdir().unwrap();
    let status = capability_status(root.path()).unwrap();

    assert_eq!(
        status.recognition_feature.state,
        OcrRecognitionFeatureState::Ready
    );
    assert!(!status.automatic_recognition_enabled);
    assert!(status.recognition_feature.detail.contains("不需要下载模型"));
}

#[test]
fn built_in_visual_splitter_is_installed_but_never_downloadable() {
    let root = tempdir().unwrap();
    let OcrCapabilityStatus { components, .. } = capability_status(root.path()).unwrap();
    let opencv = components
        .iter()
        .find(|component| component.id == OcrComponentId::OpencvPreprocess)
        .unwrap();

    assert_eq!(opencv.state, OcrComponentState::Installed);
    assert!(!opencv.install_allowed);
    assert_eq!(opencv.download_bytes, 0.0);
}

#[tokio::test]
#[ignore = "downloads and verifies the pinned 31 MB PP-OCRv6 model bundle"]
async fn pinned_small_bundle_downloads_and_verifies_end_to_end() {
    let root = tempdir().unwrap();
    let client = download_client().unwrap();

    let installed = install_component(root.path(), OcrComponentId::Ppocrv6Small, &client)
        .await
        .unwrap();
    assert_eq!(installed.state, OcrComponentState::Installed);
    assert_eq!(installed.installed_bytes, installed.download_bytes);

    let restored = capability_status(root.path()).unwrap();
    let small = restored
        .components
        .iter()
        .find(|component| component.id == OcrComponentId::Ppocrv6Small)
        .unwrap();
    assert_eq!(small.state, OcrComponentState::Installed);
}
