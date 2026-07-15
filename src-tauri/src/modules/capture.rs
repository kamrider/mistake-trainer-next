use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
    sync::Mutex,
};

use image::ImageReader;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_CAPTURE_FILE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_STAGED_ASSETS: usize = 40;
const MAX_STAGED_BYTES: usize = 100 * 1024 * 1024;
const MAX_DIMENSION: u32 = 12_000;
const MAX_PIXELS: u64 = 80_000_000;

#[derive(Clone, Debug)]
pub struct CaptureImageMetadata {
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

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
    sequence: u64,
}

#[derive(Debug, Default)]
pub struct CaptureStage {
    state: Mutex<CaptureStageState>,
}

#[derive(Debug, Default)]
struct CaptureStageState {
    assets: HashMap<String, StagedCapture>,
    next_sequence: u64,
    total_bytes: usize,
}

pub(crate) enum ConsumeError<E> {
    Stage(StageCaptureError),
    Operation(E),
}

impl CaptureStage {
    pub fn len(&self) -> Result<usize, StageCaptureError> {
        self.state
            .lock()
            .map(|state| state.assets.len())
            .map_err(|_| StageCaptureError::StageUnavailable)
    }

    pub fn summaries(&self) -> Result<Vec<StagedAsset>, StageCaptureError> {
        let state = self
            .state
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        let mut captures = state.assets.values().collect::<Vec<_>>();
        captures.sort_by_key(|capture| capture.sequence);
        Ok(captures
            .into_iter()
            .map(|capture| capture.summary.clone())
            .collect())
    }

    pub fn remove(&self, id: &str) -> Result<bool, StageCaptureError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        let removed = state.assets.remove(id);
        if let Some(capture) = &removed {
            state.total_bytes = state.total_bytes.saturating_sub(capture.bytes.len());
        }
        Ok(removed.is_some())
    }

    pub(crate) fn remove_many(&self, ids: &[String]) -> Result<(), StageCaptureError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StageCaptureError::StageUnavailable)?;
        for id in ids {
            if let Some(capture) = state.assets.remove(id) {
                state.total_bytes = state.total_bytes.saturating_sub(capture.bytes.len());
            }
        }
        Ok(())
    }

    pub(crate) fn consume_on_success<T, E>(
        &self,
        ids: &[String],
        operation: impl FnOnce(Vec<StagedCapture>) -> Result<T, E>,
    ) -> Result<T, ConsumeError<E>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ConsumeError::Stage(StageCaptureError::StageUnavailable))?;
        let captures = ids
            .iter()
            .map(|id| {
                state
                    .assets
                    .get(id)
                    .cloned()
                    .ok_or(ConsumeError::Stage(StageCaptureError::NotFound))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output = operation(captures).map_err(ConsumeError::Operation)?;
        for id in ids {
            if let Some(capture) = state.assets.remove(id) {
                state.total_bytes = state.total_bytes.saturating_sub(capture.bytes.len());
            }
        }
        Ok(output)
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
    #[error("capture staging capacity has been reached")]
    StageFull,
}

#[derive(Debug, Error)]
pub enum CaptureFileReadError {
    #[error("capture file could not be read")]
    Unreadable,
    #[error("capture file exceeds the per-file limit")]
    TooLarge,
}

pub fn read_capture_file(path: &Path) -> Result<Vec<u8>, CaptureFileReadError> {
    let file = std::fs::File::open(path).map_err(|_| CaptureFileReadError::Unreadable)?;
    let mut reader = file.take(MAX_CAPTURE_FILE_BYTES + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| CaptureFileReadError::Unreadable)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CAPTURE_FILE_BYTES {
        return Err(CaptureFileReadError::TooLarge);
    }
    Ok(bytes)
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
    let metadata = inspect_capture_image(&bytes)?;

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
        media_type: metadata.media_type,
        byte_length: bytes.len() as f64,
        width: metadata.width,
        height: metadata.height,
    };
    let mut state = stage
        .state
        .lock()
        .map_err(|_| StageCaptureError::StageUnavailable)?;
    if state.assets.len() >= MAX_STAGED_ASSETS
        || state.total_bytes.saturating_add(bytes.len()) > MAX_STAGED_BYTES
    {
        return Err(StageCaptureError::StageFull);
    }
    let sequence = state.next_sequence;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.total_bytes = state.total_bytes.saturating_add(bytes.len());
    state.assets.insert(
        summary.id.clone(),
        StagedCapture {
            summary: summary.clone(),
            bytes,
            sequence,
        },
    );
    Ok(summary)
}

pub fn inspect_capture_image(bytes: &[u8]) -> Result<CaptureImageMetadata, StageCaptureError> {
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CAPTURE_FILE_BYTES {
        return Err(StageCaptureError::InvalidImage);
    }

    let format = image::guess_format(bytes).map_err(|_| StageCaptureError::InvalidImage)?;
    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(StageCaptureError::InvalidImage),
    };
    let reader = ImageReader::with_format(Cursor::new(bytes), format);
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
    image::load_from_memory_with_format(bytes, format)
        .map_err(|_| StageCaptureError::InvalidImage)?;
    Ok(CaptureImageMetadata {
        media_type: media_type.to_owned(),
        width,
        height,
    })
}
