use std::{fs::File, io::{Cursor, Read}};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        exports::{
            CreateExportSnapshot, ExportError, ExportLayout, create_export_snapshot,
            generate_export,
        },
        problems::{AssetRole, CaptureAsset, CreateProblem, create_problem},
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::tempdir;

fn png(color: [u8; 4]) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(24, 16, Rgba(color)));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

fn document_xml(path: &std::path::Path) -> String {
    let file = File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

#[test]
fn generation_writes_original_images_and_word_documents_without_exposing_paths() {
    let directory = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let mut connection =
        open_encrypted_database(&directory.path().join("library.db"), "key").unwrap();
    run_migrations(&mut connection).unwrap();
    let profile = create_profile(
        &mut connection,
        CreateProfile {
            account_id: "account-1".to_owned(),
            name: "小树".to_owned(),
            now_utc_ms: 10,
        },
    )
    .unwrap();
    let question = png([24, 80, 65, 255]);
    let answer = png([185, 88, 63, 255]);
    let asset_key = [9_u8; 32];
    let problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &asset_key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "数学".to_owned(),
            note: "二次函数".to_owned(),
            assets: vec![
                CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: question.clone(),
                },
                CaptureAsset {
                    role: AssetRole::Answer,
                    media_type: "image/png".to_owned(),
                    bytes: answer.clone(),
                },
            ],
            now_utc_ms: 20,
        },
    )
    .unwrap();
    let second_problem = create_problem(
        &mut connection,
        &directory.path().join("assets"),
        &asset_key,
        CreateProblem {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            subject: "物理".to_owned(),
            note: "受力分析".to_owned(),
            assets: vec![
                CaptureAsset {
                    role: AssetRole::Question,
                    media_type: "image/png".to_owned(),
                    bytes: png([30, 60, 90, 255]),
                },
                CaptureAsset {
                    role: AssetRole::Answer,
                    media_type: "image/png".to_owned(),
                    bytes: png([120, 90, 60, 255]),
                },
            ],
            now_utc_ms: 21,
        },
    )
    .unwrap();

    let folder_snapshot = create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            title: "周测：复盘/一".to_owned(),
            problem_ids: vec![problem.id.clone(), second_problem.id.clone()],
            layout: ExportLayout::OriginalImageFolder,
            now_utc_ms: 30,
        },
    )
    .unwrap();
    let folder = generate_export(
        &connection,
        &directory.path().join("assets"),
        &asset_key,
        "account-1",
        &profile.id,
        &folder_snapshot.id,
        destination.path(),
    )
    .unwrap();
    assert!(!folder.output_name.contains([':', '/', '\\']));
    let folder_path = destination.path().join(&folder.output_name);
    assert_eq!(std::fs::read(folder_path.join("001-question-01.png")).unwrap(), question);
    assert_eq!(std::fs::read(folder_path.join("001-answer-01.png")).unwrap(), answer);
    assert!(folder_path.join("002-question-01.png").is_file());
    assert!(folder_path.join("002-answer-01.png").is_file());

    let docx_snapshot = create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            title: "题答交替".to_owned(),
            problem_ids: vec![problem.id.clone(), second_problem.id.clone()],
            layout: ExportLayout::QuestionAnswerAlternating,
            now_utc_ms: 40,
        },
    )
    .unwrap();
    let first = generate_export(
        &connection,
        &directory.path().join("assets"),
        &asset_key,
        "account-1",
        &profile.id,
        &docx_snapshot.id,
        destination.path(),
    )
    .unwrap();
    let second = generate_export(
        &connection,
        &directory.path().join("assets"),
        &asset_key,
        "account-1",
        &profile.id,
        &docx_snapshot.id,
        destination.path(),
    )
    .unwrap();
    assert_ne!(first.output_name, second.output_name);
    for generated in [first, second] {
        let path = destination.path().join(generated.output_name);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"PK"));
        assert!(bytes.len() > 1_000);
        let xml = document_xml(&path);
        let first_question = xml.find("1. 题目 · 数学").unwrap();
        let first_answer = xml.find("答案").unwrap();
        let second_question = xml.find("2. 题目 · 物理").unwrap();
        assert!(first_question < first_answer && first_answer < second_question);
    }

    let grouped_snapshot = create_export_snapshot(
        &mut connection,
        CreateExportSnapshot {
            account_id: "account-1".to_owned(),
            profile_id: profile.id.clone(),
            title: "题答分组".to_owned(),
            problem_ids: vec![problem.id, second_problem.id],
            layout: ExportLayout::QuestionsThenAnswers,
            now_utc_ms: 50,
        },
    )
    .unwrap();
    let grouped = generate_export(
        &connection,
        &directory.path().join("assets"),
        &asset_key,
        "account-1",
        &profile.id,
        &grouped_snapshot.id,
        destination.path(),
    )
    .unwrap();
    let grouped_xml = document_xml(&destination.path().join(grouped.output_name));
    let first_question = grouped_xml.find("1. 题目 · 数学").unwrap();
    let second_question = grouped_xml.find("2. 题目 · 物理").unwrap();
    let first_answer = grouped_xml.find("1. 答案 · 数学").unwrap();
    let second_answer = grouped_xml.find("2. 答案 · 物理").unwrap();
    assert!(first_question < second_question);
    assert!(second_question < first_answer);
    assert!(first_answer < second_answer);

    let relative = generate_export(
        &connection,
        &directory.path().join("assets"),
        &asset_key,
        "account-1",
        &profile.id,
        &docx_snapshot.id,
        std::path::Path::new("relative"),
    )
    .expect_err("a relative destination must be rejected");
    assert!(matches!(relative, ExportError::InvalidDestination));
}
