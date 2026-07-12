use mistake_trainer_next_lib::modules::capture::{
    CaptureStage, StageCaptureError, stage_image_bytes,
};

const VALID_PNG: &[u8] = include_bytes!("../icons/32x32.png");

#[test]
fn valid_images_are_staged_by_opaque_id_without_exposing_source_paths() {
    let stage = CaptureStage::default();

    let asset = stage_image_bytes(
        &stage,
        "C:\\Users\\student\\Desktop\\secret-question.png",
        "question",
        VALID_PNG.to_vec(),
    )
    .expect("valid png");
    let json = serde_json::to_string(&asset).expect("asset json");

    assert_eq!(asset.file_name, "secret-question.png");
    assert_eq!(asset.media_type, "image/png");
    assert_eq!(asset.role, "question");
    assert!(!json.contains("Desktop"));
    assert!(!json.contains("student"));
    assert_eq!(stage.len().expect("stage lock"), 1);
}

#[test]
fn corrupt_or_unsupported_files_never_enter_the_stage() {
    let stage = CaptureStage::default();

    let error = stage_image_bytes(
        &stage,
        "answer.png",
        "answer",
        b"this is not an image".to_vec(),
    )
    .expect_err("corrupt image must fail");

    assert!(matches!(error, StageCaptureError::InvalidImage));
    assert_eq!(stage.len().expect("stage lock"), 0);
}

#[test]
fn staged_assets_can_be_listed_and_removed_without_file_paths() {
    let stage = CaptureStage::default();
    let asset = stage_image_bytes(&stage, "private-answer.png", "answer", VALID_PNG.to_vec())
        .expect("answer");

    let summaries = stage.summaries().expect("summaries");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, asset.id);
    assert!(stage.remove(&asset.id).expect("remove"));
    assert!(!stage.remove(&asset.id).expect("remove twice"));
    assert_eq!(stage.len().expect("stage lock"), 0);
}
