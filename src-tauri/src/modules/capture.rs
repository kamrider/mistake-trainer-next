use std::{collections::HashMap, io::Cursor, path::Path, sync::Mutex};

use image::ImageReader;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
const MAX_DIMENSION: u32 = 12_000;
const MAX_PIXELS: u64 = 80_000_000;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StagedAsset {
    pub id: String,
    pub file_name: String,
    pub role: String,
    pub media_type: String,
    pub byte_length: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct StagedCapture {
    pub summary: StagedAsset,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct CaptureStage {
    assets: Mutex<HashMap<String, StagedCapture>>,
}

impl CaptureStage {
    pub fn len(&self) -> Result<usize, StageCaptureError> {
        self.assets
            .lock()
            .map(|assets| assets.len())
            .map_err(|_| StageCaptureError::StageUnavailable)
    }

    pub fn summaries(&self) -> Result<Vec<StagedAsset>, StageCaptureError> {
        let assets = self
            .assets
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        let mut summaries = assets
            .values()
            .map(|capture| capture.summary.clone())
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(summaries)
    }

    pub fn remove(&self, id: &str) -> Result<bool, StageCaptureError> {
        self.assets
            .lock()
            .map(|mut assets| assets.remove(id).is_some())
            .map_err(|_| StageCaptureError::StageUnavailable)
    }

    pub(crate) fn captures(&self, ids: &[String]) -> Result<Vec<StagedCapture>, StageCaptureError> {
        let assets = self
            .assets
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        ids.iter()
            .map(|id| assets.get(id).cloned().ok_or(StageCaptureError::NotFound))
            .collect()
    }

    pub(crate) fn remove_many(&self, ids: &[String]) -> Result<(), StageCaptureError> {
        let mut assets = self
            .assets
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        for id in ids {
            assets.remove(id);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StageCaptureError {
    #[error("capture role is invalid")]
    InvalidRole,
    #[error("image is empty, corrupt, unsupported, or too large")]
    InvalidImage,
    #[error("capture staging is unavailable")]
    StageUnavailable,
    #[error("staged image was not found")]
    NotFound,
}

pub fn stage_image_bytes(
    stage: &CaptureStage,
    source_label: &str,
    role: &str,
    bytes: Vec<u8>,
) -> Result<StagedAsset, StageCaptureError> {
    if !matches!(role, "question" | "answer") {
        return Err(StageCaptureError::InvalidRole);
    }
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
        return Err(StageCaptureError::InvalidImage);
    }

    let format = image::guess_format(&bytes).map_err(|_| StageCaptureError::InvalidImage)?;
    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(StageCaptureError::InvalidImage),
    };
    let reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| StageCaptureError::InvalidImage)?;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(StageCaptureError::InvalidImage);
    }
    image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| StageCaptureError::InvalidImage)?;

    let file_name = Path::new(source_label)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("image")
        .to_owned();
    let summary = StagedAsset {
        id: Uuid::now_v7().to_string(),
        file_name,
        role: role.to_owned(),
        media_type: media_type.to_owned(),
        byte_length: bytes.len() as f64,
        width,
        height,
    };
    stage
        .assets
        .lock()
        .map_err(|_| StageCaptureError::StageUnavailable)?
        .insert(
            summary.id.clone(),
            StagedCapture {
                summary: summary.clone(),
                bytes,
            },
        );
    Ok(summary)
}
