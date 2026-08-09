use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::{
    infrastructure::{
        capture_recognition_worker::{
            CaptureRecognitionEngine, RecognitionAnalysis, RecognitionEngineError,
        },
        recognition_visual_split::VisualSplitRecognitionEngine,
    },
    modules::capture_recognition::CaptureRecognitionRole,
};

pub const PRODUCT_ENGINE_NAME: &str = "local-question-split-hybrid";
pub const PRODUCT_ENGINE_VERSION: &str = "2.0.0";

/// Product recognition prefers the locally installed PP-OCRv6 small anchor
/// runtime on supported x64 machines and otherwise retains the conservative
/// built-in visual splitter. Model or runtime absence never blocks manual
/// capture, and low-confidence OCR output remains a full-page review item.
pub struct ProductRecognitionEngine {
    control_root: PathBuf,
    resource_root: PathBuf,
    private_temp_root: PathBuf,
    visual: VisualSplitRecognitionEngine,
    #[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
    ocr: Mutex<
        Option<
            crate::infrastructure::recognition_anchor_layout::AnchorRecognitionEngine<
                crate::infrastructure::recognition_ppocr_runtime::PpOcrLocalRuntime,
            >,
        >,
    >,
}

impl ProductRecognitionEngine {
    pub fn new(control_root: &Path, resource_root: &Path, private_temp_root: &Path) -> Self {
        Self {
            control_root: control_root.to_path_buf(),
            resource_root: resource_root.to_path_buf(),
            private_temp_root: private_temp_root.to_path_buf(),
            visual: VisualSplitRecognitionEngine,
            #[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
            ocr: Mutex::new(None),
        }
    }

    #[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
    fn analyze_with_ocr(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        use crate::infrastructure::{
            recognition_anchor_layout::AnchorRecognitionEngine,
            recognition_ppocr_runtime::PpOcrLocalRuntime,
        };

        let mut runtime = self
            .ocr
            .lock()
            .map_err(|_| RecognitionEngineError::Failed)?;
        let component_directory = self
            .control_root
            .join("optional-components")
            .join("ppocrv6-small");
        if !retain_cached_runtime_if_component_present(&mut *runtime, &component_directory) {
            // Removal is a user-visible capability change. Drop the cached
            // engine immediately so the next page uses the visual fallback.
            return Err(RecognitionEngineError::ModelMissing);
        }
        if runtime.is_none() {
            let runtime_library_path = self
                .resource_root
                .join("ocr-runtime")
                .join("win-x64")
                .join("onnxruntime.dll");
            let local_runtime = PpOcrLocalRuntime::from_verified_small_component(
                &component_directory,
                &runtime_library_path,
                &self.private_temp_root,
                inference_threads(),
            )?;
            *runtime = Some(AnchorRecognitionEngine::new(local_runtime));
        }
        runtime
            .as_ref()
            .ok_or(RecognitionEngineError::RuntimeUnavailable)?
            .analyze(image_path, staged_role)
    }
}

#[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
fn retain_cached_runtime_if_component_present<T>(
    runtime: &mut Option<T>,
    component_directory: &Path,
) -> bool {
    use crate::infrastructure::recognition_ppocr_runtime::PpOcrLocalRuntime;

    if PpOcrLocalRuntime::small_component_files_present(component_directory) {
        true
    } else {
        *runtime = None;
        false
    }
}

impl CaptureRecognitionEngine for ProductRecognitionEngine {
    fn analyze(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        #[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
        if let Ok(analysis) = self.analyze_with_ocr(image_path, staged_role) {
            return Ok(analysis);
        }

        self.visual.analyze(image_path, staged_role)
    }
}

#[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
fn inference_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().saturating_sub(1).clamp(1, 4))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_optional_model_falls_back_without_creating_component_directories() {
        let root = tempfile::tempdir().unwrap();
        let control_root = root.path().join("control");
        let resource_root = root.path().join("resources");
        let private_temp_root = root.path().join("private");
        let image_path = root.path().join("page.png");
        image::GrayImage::from_pixel(80, 80, image::Luma([255]))
            .save(&image_path)
            .unwrap();
        let engine =
            ProductRecognitionEngine::new(&control_root, &resource_root, &private_temp_root);

        let result = engine
            .analyze(&image_path, CaptureRecognitionRole::Question)
            .unwrap();

        assert_eq!(result.regions.len(), 1);
        assert!(!control_root.exists());
        assert!(!private_temp_root.exists());
    }

    #[cfg(all(feature = "local-ocr-runtime", target_arch = "x86_64"))]
    #[test]
    fn removing_component_files_is_observed_before_the_next_ocr_analysis() {
        use std::fs::{self, OpenOptions};

        let root = tempfile::tempdir().unwrap();
        let component = root.path().join("optional-components/ppocrv6-small");
        fs::create_dir_all(&component).unwrap();
        for (name, bytes) in [
            ("PP-OCRv6_det_small.onnx", 9_929_594_u64),
            ("PP-OCRv6_rec_small.onnx", 21_234_383_u64),
        ] {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(component.join(name))
                .unwrap()
                .set_len(bytes)
                .unwrap();
        }
        let mut cached_runtime = Some(());
        assert!(retain_cached_runtime_if_component_present(
            &mut cached_runtime,
            &component
        ));
        assert!(cached_runtime.is_some());

        fs::remove_dir_all(&component).unwrap();

        assert!(!retain_cached_runtime_if_component_present(
            &mut cached_runtime,
            &component
        ));
        assert!(cached_runtime.is_none());
    }
}
