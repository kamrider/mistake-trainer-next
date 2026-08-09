use thiserror::Error;

const CHARACTER_METADATA_KEY: &str = "character";
const MAX_CHARACTER_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MIN_CHARACTER_COUNT: usize = 100;
const MAX_CHARACTER_COUNT: usize = 20_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OnnxMetadataError {
    #[error("the ONNX protobuf is malformed")]
    Malformed,
    #[error("the ONNX character metadata is missing")]
    CharacterMetadataMissing,
    #[error("the ONNX character metadata is invalid")]
    InvalidCharacterMetadata,
}

/// Reads the `character` entry from ONNX `ModelProto.metadata_props` (field 14)
/// without loading an inference runtime. The returned string is model-owned
/// public vocabulary data, not OCR output.
pub fn extract_character_dictionary(model: &[u8]) -> Result<&str, OnnxMetadataError> {
    let mut offset = 0;
    let mut dictionary = None;
    while offset < model.len() {
        let key = read_varint(model, &mut offset)?;
        let field = key >> 3;
        let wire = (key & 0x07) as u8;
        if field == 14 && wire == 2 {
            let entry = read_length_delimited(model, &mut offset)?;
            if let Some(value) = parse_metadata_entry(entry)?
                && dictionary.replace(value).is_some()
            {
                return Err(OnnxMetadataError::InvalidCharacterMetadata);
            }
        } else {
            skip_value(model, &mut offset, wire)?;
        }
    }
    let dictionary = dictionary.ok_or(OnnxMetadataError::CharacterMetadataMissing)?;
    validate_character_dictionary(dictionary)?;
    Ok(dictionary)
}

fn parse_metadata_entry(entry: &[u8]) -> Result<Option<&str>, OnnxMetadataError> {
    let mut offset = 0;
    let mut key = None;
    let mut value = None;
    while offset < entry.len() {
        let tag = read_varint(entry, &mut offset)?;
        let field = tag >> 3;
        let wire = (tag & 0x07) as u8;
        match (field, wire) {
            (1, 2) => {
                key = Some(
                    std::str::from_utf8(read_length_delimited(entry, &mut offset)?)
                        .map_err(|_| OnnxMetadataError::Malformed)?,
                );
            }
            (2, 2) => {
                value = Some(
                    std::str::from_utf8(read_length_delimited(entry, &mut offset)?)
                        .map_err(|_| OnnxMetadataError::Malformed)?,
                );
            }
            _ => skip_value(entry, &mut offset, wire)?,
        }
    }
    if key == Some(CHARACTER_METADATA_KEY) {
        Ok(Some(
            value.ok_or(OnnxMetadataError::InvalidCharacterMetadata)?,
        ))
    } else {
        Ok(None)
    }
}

fn validate_character_dictionary(dictionary: &str) -> Result<(), OnnxMetadataError> {
    if dictionary.is_empty()
        || dictionary.len() > MAX_CHARACTER_METADATA_BYTES
        || dictionary.contains('\0')
    {
        return Err(OnnxMetadataError::InvalidCharacterMetadata);
    }
    let count = dictionary.lines().count();
    if !(MIN_CHARACTER_COUNT..=MAX_CHARACTER_COUNT).contains(&count)
        || dictionary
            .split('\n')
            .any(|line| line.trim_end_matches('\r').len() > 64)
    {
        return Err(OnnxMetadataError::InvalidCharacterMetadata);
    }
    Ok(())
}

fn read_varint(input: &[u8], offset: &mut usize) -> Result<u64, OnnxMetadataError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *input.get(*offset).ok_or(OnnxMetadataError::Malformed)?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err(OnnxMetadataError::Malformed);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(OnnxMetadataError::Malformed)
}

fn read_length_delimited<'a>(
    input: &'a [u8],
    offset: &mut usize,
) -> Result<&'a [u8], OnnxMetadataError> {
    let length =
        usize::try_from(read_varint(input, offset)?).map_err(|_| OnnxMetadataError::Malformed)?;
    let end = offset
        .checked_add(length)
        .ok_or(OnnxMetadataError::Malformed)?;
    let value = input
        .get(*offset..end)
        .ok_or(OnnxMetadataError::Malformed)?;
    *offset = end;
    Ok(value)
}

fn skip_value(input: &[u8], offset: &mut usize, wire: u8) -> Result<(), OnnxMetadataError> {
    match wire {
        0 => {
            read_varint(input, offset)?;
        }
        1 => {
            *offset = offset
                .checked_add(8)
                .filter(|end| *end <= input.len())
                .ok_or(OnnxMetadataError::Malformed)?;
        }
        2 => {
            read_length_delimited(input, offset)?;
        }
        5 => {
            *offset = offset
                .checked_add(4)
                .filter(|end| *end <= input.len())
                .ok_or(OnnxMetadataError::Malformed)?;
        }
        _ => return Err(OnnxMetadataError::Malformed),
    }
    Ok(())
}

#[cfg(feature = "local-ocr-runtime")]
mod runtime {
    use std::{
        fs::{self, OpenOptions},
        io::{Read, Write},
        path::{Path, PathBuf},
        sync::Mutex,
    };

    use image::GenericImageView;
    use ppocr_rs::OcrLite;
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::extract_character_dictionary;
    use crate::infrastructure::{
        capture_recognition_worker::RecognitionEngineError,
        recognition_anchor_layout::{LocalOcrBox, LocalOcrPage, LocalOcrRuntime},
    };

    const DET_NAME: &str = "PP-OCRv6_det_small.onnx";
    const DET_BYTES: u64 = 9_929_594;
    const DET_SHA256: &str = "090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f";
    const REC_NAME: &str = "PP-OCRv6_rec_small.onnx";
    const REC_BYTES: u64 = 21_234_383;
    const REC_SHA256: &str = "6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884";
    const RUNTIME_NAME: &str = "onnxruntime.dll";
    const RUNTIME_BYTES: u64 = 11_569_696;
    const RUNTIME_SHA256: &str = "4cb41e89b8bf30578e1dd95e9c40292d61974a4bfcd666409302c4f0c5aa8ce0";
    const PROVIDERS_NAME: &str = "onnxruntime_providers_shared.dll";
    const PROVIDERS_BYTES: u64 = 22_048;
    const PROVIDERS_SHA256: &str =
        "3b164f0019863266971843baca9551f015db7e502e2d9c4575f74c6a2bbbb78a";
    const MAX_IMAGE_SIDE: u32 = 12_000;
    const MAX_IMAGE_PIXELS: u64 = 80_000_000;

    pub struct PpOcrLocalRuntime {
        engine: Mutex<OcrLite>,
    }

    impl PpOcrLocalRuntime {
        /// Cheaply checks that the exact component files used by an already
        /// verified in-memory runtime are still installed. Full hashes are
        /// verified when the runtime is constructed; this metadata check lets
        /// component removal take effect before the next analysis without
        /// hashing both models for every page.
        pub(crate) fn small_component_files_present(component_directory: &Path) -> bool {
            [
                (component_directory.join(DET_NAME), DET_BYTES),
                (component_directory.join(REC_NAME), REC_BYTES),
            ]
            .into_iter()
            .all(|(path, expected_bytes)| {
                fs::metadata(path)
                    .map(|metadata| metadata.is_file() && metadata.len() == expected_bytes)
                    .unwrap_or(false)
            })
        }

        /// Creates an offline CPU runtime from the already-installed small
        /// component. Both model files are verified again before ONNX Runtime
        /// sees them, and the matching vocabulary is extracted from the
        /// recognition model itself.
        pub fn from_verified_small_component(
            component_directory: &Path,
            runtime_library_path: &Path,
            private_temp_root: &Path,
            threads: usize,
        ) -> Result<Self, RecognitionEngineError> {
            let det_path = component_directory.join(DET_NAME);
            let rec_path = component_directory.join(REC_NAME);
            verify_file(&det_path, DET_BYTES, DET_SHA256)
                .map_err(|_| RecognitionEngineError::ModelMissing)?;
            verify_file(&rec_path, REC_BYTES, REC_SHA256)
                .map_err(|_| RecognitionEngineError::ModelMissing)?;
            verify_runtime_bundle(runtime_library_path)
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;

            let rec_model =
                fs::read(&rec_path).map_err(|_| RecognitionEngineError::ModelMissing)?;
            let dictionary = extract_character_dictionary(&rec_model)
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            fs::create_dir_all(private_temp_root)
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            let dictionary_path =
                private_temp_root.join(format!("ocr-dict-{}.txt", Uuid::now_v7()));
            let dictionary_guard = TemporaryFile(dictionary_path.clone());
            let mut dictionary_file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&dictionary_path)
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            dictionary_file
                .write_all(dictionary.as_bytes())
                .and_then(|_| dictionary_file.sync_all())
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            drop(dictionary_file);
            drop(rec_model);

            ort::init_from(
                path_text(runtime_library_path)
                    .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?,
            )
            .with_telemetry(false)
            .commit()
            .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            let mut engine = OcrLite::new();
            engine
                .init_models_no_angle(
                    path_text(&det_path).map_err(|_| RecognitionEngineError::RuntimeUnavailable)?,
                    path_text(&rec_path).map_err(|_| RecognitionEngineError::RuntimeUnavailable)?,
                    path_text(&dictionary_path)
                        .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?,
                    threads.clamp(1, 8),
                )
                .map_err(|_| RecognitionEngineError::RuntimeUnavailable)?;
            drop(dictionary_guard);
            Ok(Self {
                engine: Mutex::new(engine),
            })
        }
    }

    impl LocalOcrRuntime for PpOcrLocalRuntime {
        fn recognize(&self, image_path: &Path) -> Result<LocalOcrPage, RecognitionEngineError> {
            let image =
                image::open(image_path).map_err(|_| RecognitionEngineError::InvalidResult)?;
            let (width, height) = image.dimensions();
            if width == 0
                || height == 0
                || width > MAX_IMAGE_SIDE
                || height > MAX_IMAGE_SIDE
                || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
            {
                return Err(RecognitionEngineError::InvalidResult);
            }
            let image = image.to_rgb8();
            let result = self
                .engine
                .lock()
                .map_err(|_| RecognitionEngineError::Failed)?
                .detect(&image, 0, 1_024, 0.50, 0.30, 1.60, false, false)
                .map_err(|_| RecognitionEngineError::Failed)?;
            let mut boxes = Vec::with_capacity(result.text_blocks.len());
            for block in result.text_blocks {
                let Some(raw_left) = block.box_points.iter().map(|point| point.x).min() else {
                    continue;
                };
                let Some(raw_top) = block.box_points.iter().map(|point| point.y).min() else {
                    continue;
                };
                let Some(raw_right) = block.box_points.iter().map(|point| point.x).max() else {
                    continue;
                };
                let Some(raw_bottom) = block.box_points.iter().map(|point| point.y).max() else {
                    continue;
                };
                if !block.box_score.is_finite() || !block.text_score.is_finite() {
                    continue;
                }
                // A trusted detector can return an empty polygon or extend a
                // text box by a few pixels beyond an image edge. Discard a
                // malformed individual block and clamp harmless edge
                // overshoot; missing anchors later become a low-confidence
                // full-page review suggestion.
                let left = f64::from(raw_left).clamp(0.0, f64::from(width));
                let top = f64::from(raw_top).clamp(0.0, f64::from(height));
                let right = f64::from(raw_right).clamp(0.0, f64::from(width));
                let bottom = f64::from(raw_bottom).clamp(0.0, f64::from(height));
                if right <= left || bottom <= top {
                    continue;
                }
                let confidence = f64::from(block.box_score.min(block.text_score));
                boxes.push(LocalOcrBox {
                    left,
                    top,
                    right,
                    bottom,
                    is_text: !block.text.trim().is_empty(),
                    text: block.text,
                    confidence,
                });
            }
            Ok(LocalOcrPage {
                width,
                height,
                boxes,
            })
        }
    }

    struct TemporaryFile(PathBuf);

    impl Drop for TemporaryFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<(), ()> {
        let mut file = OpenOptions::new().read(true).open(path).map_err(|_| ())?;
        if file.metadata().map_err(|_| ())?.len() != expected_bytes {
            return Err(());
        }
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|_| ())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        (format!("{:x}", hasher.finalize()) == expected_sha256)
            .then_some(())
            .ok_or(())
    }

    pub(super) fn verify_runtime_bundle(runtime_library_path: &Path) -> Result<(), ()> {
        if runtime_library_path
            .file_name()
            .and_then(|name| name.to_str())
            != Some(RUNTIME_NAME)
        {
            return Err(());
        }
        verify_file(runtime_library_path, RUNTIME_BYTES, RUNTIME_SHA256)?;
        let providers_path = runtime_library_path
            .parent()
            .ok_or(())?
            .join(PROVIDERS_NAME);
        verify_file(&providers_path, PROVIDERS_BYTES, PROVIDERS_SHA256)
    }

    fn path_text(path: &Path) -> Result<&str, RecognitionEngineError> {
        path.to_str().ok_or(RecognitionEngineError::Failed)
    }
}

#[cfg(feature = "local-ocr-runtime")]
pub use runtime::PpOcrLocalRuntime;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_model_owned_character_dictionary_without_an_inference_runtime() {
        let dictionary = (0..120)
            .map(|index| format!("token-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let model = model_with_metadata("character", &dictionary);

        assert_eq!(extract_character_dictionary(&model).unwrap(), dictionary);
    }

    #[test]
    fn ignores_unrelated_metadata_and_fails_closed_for_missing_or_duplicate_dictionary() {
        let unrelated = model_with_metadata("author", "test");
        assert_eq!(
            extract_character_dictionary(&unrelated),
            Err(OnnxMetadataError::CharacterMetadataMissing)
        );

        let dictionary = (0..120)
            .map(|index| format!("token-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut duplicate = model_with_metadata("character", &dictionary);
        duplicate.extend(model_with_metadata("character", &dictionary));
        assert_eq!(
            extract_character_dictionary(&duplicate),
            Err(OnnxMetadataError::InvalidCharacterMetadata)
        );
    }

    #[test]
    fn rejects_truncated_protobuf_and_unbounded_vocabulary_entries() {
        assert_eq!(
            extract_character_dictionary(&[0x72, 0x80]),
            Err(OnnxMetadataError::Malformed)
        );
        let long_line = "x".repeat(65);
        let dictionary = (0..120)
            .map(|_| long_line.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            extract_character_dictionary(&model_with_metadata("character", &dictionary)),
            Err(OnnxMetadataError::InvalidCharacterMetadata)
        );
    }

    #[cfg(feature = "local-ocr-runtime")]
    #[test]
    fn rejects_an_unverified_runtime_bundle_before_loading_foreign_code() {
        let private_temp = tempfile::tempdir().unwrap();
        let runtime_path = private_temp.path().join("onnxruntime.dll");
        let providers_path = private_temp.path().join("onnxruntime_providers_shared.dll");
        std::fs::write(&runtime_path, b"not an approved runtime").unwrap();
        std::fs::write(&providers_path, b"not an approved provider library").unwrap();

        assert_eq!(
            super::runtime::verify_runtime_bundle(&runtime_path),
            Err(())
        );
        assert_eq!(
            super::runtime::verify_runtime_bundle(&private_temp.path().join("other.dll")),
            Err(())
        );
    }

    #[cfg(feature = "local-ocr-runtime")]
    #[test]
    #[ignore = "requires separately installed, hash-pinned models and a CPU ONNX Runtime DLL"]
    fn initializes_the_real_runtime_and_processes_an_image_offline() {
        use std::path::PathBuf;

        use crate::infrastructure::recognition_anchor_layout::LocalOcrRuntime as _;

        let component_directory = PathBuf::from(
            std::env::var_os("MISTAKE_TRAINER_OCR_COMPONENT_DIR")
                .expect("MISTAKE_TRAINER_OCR_COMPONENT_DIR is required"),
        );
        let runtime_library_path = PathBuf::from(
            std::env::var_os("MISTAKE_TRAINER_ORT_DLL")
                .expect("MISTAKE_TRAINER_ORT_DLL is required"),
        );
        let private_temp = tempfile::tempdir().unwrap();
        let image_path = private_temp.path().join("blank.png");
        image::RgbImage::from_pixel(640, 480, image::Rgb([255, 255, 255]))
            .save(&image_path)
            .unwrap();

        let runtime = PpOcrLocalRuntime::from_verified_small_component(
            &component_directory,
            &runtime_library_path,
            private_temp.path(),
            2,
        )
        .unwrap();
        let page = runtime.recognize(&image_path).unwrap();

        assert_eq!((page.width, page.height), (640, 480));
        assert!(page.boxes.is_empty());
    }

    #[cfg(feature = "local-ocr-runtime")]
    #[test]
    #[ignore = "requires the local six-image question-region corpus and hash-pinned OCR runtime"]
    fn real_question_region_corpus_matches_the_review_contract() {
        use std::path::PathBuf;

        use crate::{
            infrastructure::recognition_anchor_layout::{LocalOcrRuntime as _, analyze_ocr_page},
            modules::capture_recognition::CaptureRecognitionRole,
        };

        let component_directory = PathBuf::from(
            std::env::var_os("MISTAKE_TRAINER_OCR_COMPONENT_DIR")
                .expect("MISTAKE_TRAINER_OCR_COMPONENT_DIR is required"),
        );
        let runtime_library_path = PathBuf::from(
            std::env::var_os("MISTAKE_TRAINER_ORT_DLL")
                .expect("MISTAKE_TRAINER_ORT_DLL is required"),
        );
        let corpus_directory = PathBuf::from(
            std::env::var_os("MISTAKE_TRAINER_OCR_REAL_IMAGE_DIR")
                .expect("MISTAKE_TRAINER_OCR_REAL_IMAGE_DIR is required"),
        );
        let private_temp = tempfile::tempdir().unwrap();
        let runtime = PpOcrLocalRuntime::from_verified_small_component(
            &component_directory,
            &runtime_library_path,
            private_temp.path(),
            2,
        )
        .unwrap();
        let cases = [
            ("sample-0001.png", 10, false),
            ("sample-0002.png", 4, false),
            ("sample-0003.png", 1, true),
            ("sample-0004.png", 3, false),
            ("sample-0005.png", 4, false),
            ("sample-0006.png", 12, false),
        ];

        for (file_name, expected_regions, expect_low_confidence) in cases {
            let page = runtime
                .recognize(&corpus_directory.join(file_name))
                .unwrap_or_else(|error| panic!("{file_name}: {error}"));
            let ocr_box_count = page.boxes.len();
            let analysis = analyze_ocr_page(page, CaptureRecognitionRole::Question)
                .unwrap_or_else(|error| panic!("{file_name}: {error}"));
            eprintln!(
                "{file_name}: {ocr_box_count} OCR boxes, {} proposed regions, confidence {}",
                analysis.regions.len(),
                analysis.confidence_basis_points
            );
            assert_eq!(
                analysis.regions.len(),
                expected_regions,
                "{file_name}: unexpected region count"
            );
            assert_eq!(
                analysis.confidence_basis_points < 6_500,
                expect_low_confidence,
                "{file_name}: unexpected review band"
            );
            if expect_low_confidence {
                assert_eq!(analysis.regions[0].rect.x, 0.0);
                assert_eq!(analysis.regions[0].rect.y, 0.0);
                assert_eq!(analysis.regions[0].rect.width, 1.0);
                assert_eq!(analysis.regions[0].rect.height, 1.0);
            } else {
                assert_eq!(
                    analysis
                        .pairing_tokens
                        .iter()
                        .filter(|token| token.is_some())
                        .count(),
                    expected_regions,
                    "{file_name}: every accepted region needs a question-number token"
                );
            }
        }

        assert_eq!(
            std::fs::read_dir(private_temp.path()).unwrap().count(),
            0,
            "the OCR runtime must not leave its temporary dictionary behind"
        );
    }

    fn model_with_metadata(key: &str, value: &str) -> Vec<u8> {
        let mut entry = Vec::new();
        entry.push(0x0a);
        encode_varint(key.len() as u64, &mut entry);
        entry.extend_from_slice(key.as_bytes());
        entry.push(0x12);
        encode_varint(value.len() as u64, &mut entry);
        entry.extend_from_slice(value.as_bytes());

        let mut model = vec![0x08, 0x08, 0x72];
        encode_varint(entry.len() as u64, &mut model);
        model.extend(entry);
        model
    }

    fn encode_varint(mut value: u64, output: &mut Vec<u8>) {
        while value >= 0x80 {
            output.push((value as u8 & 0x7f) | 0x80);
            value >>= 7;
        }
        output.push(value as u8);
    }
}
