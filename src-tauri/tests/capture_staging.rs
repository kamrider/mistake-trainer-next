use mistake_trainer_next_lib::modules::capture::{
    CaptureFileReadError, CaptureStage, MAX_CAPTURE_FILE_BYTES, StageCaptureError,
    read_capture_file, stage_image_bytes,
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

#[test]
fn staging_has_a_process_memory_safety_budget() {
    let stage = CaptureStage::default();
    for index in 0..40 {
        stage_image_bytes(
            &stage,
            &format!("question-{index}.png"),
            "question",
            VALID_PNG.to_vec(),
        )
        .expect("within item budget");
    }

    let error = stage_image_bytes(&stage, "one-too-many.png", "question", VALID_PNG.to_vec())
        .expect_err("stage must reject unbounded item growth");

    assert!(matches!(error, StageCaptureError::StageFull));
    assert_eq!(stage.len().expect("stage lock"), 40);
}

#[test]
fn file_read_is_hard_limited_even_when_the_source_is_larger() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("oversized.png");
    let file = std::fs::File::create(&path).expect("create sparse file");
    file.set_len(MAX_CAPTURE_FILE_BYTES + 2)
        .expect("grow sparse file");
    drop(file);

    let error = read_capture_file(&path).expect_err("oversized file must not be fully read");

    assert!(matches!(error, CaptureFileReadError::TooLarge));
}

#[test]
fn unreadable_capture_file_fails_closed() {
    let error = read_capture_file(std::path::Path::new("missing-image.png"))
        .expect_err("missing file must fail");

    assert!(matches!(error, CaptureFileReadError::Unreadable));
}
