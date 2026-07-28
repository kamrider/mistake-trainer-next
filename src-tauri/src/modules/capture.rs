use std::{
    io::{Cursor, Read},
    path::Path,
};

use image::ImageReader;
use thiserror::Error;

pub const MAX_CAPTURE_FILE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_DIMENSION: u32 = 12_000;
const MAX_PIXELS: u64 = 80_000_000;

#[derive(Clone, Debug)]
pub struct CaptureImageMetadata {
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Error)]
pub enum CaptureImageError {
    #[error("image is empty, corrupt, unsupported, or too large")]
    InvalidImage,
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

pub fn inspect_capture_image(bytes: &[u8]) -> Result<CaptureImageMetadata, CaptureImageError> {
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CAPTURE_FILE_BYTES {
        return Err(CaptureImageError::InvalidImage);
    }

    let format = image::guess_format(bytes).map_err(|_| CaptureImageError::InvalidImage)?;
    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(CaptureImageError::InvalidImage),
    };
    let reader = ImageReader::with_format(Cursor::new(bytes), format);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| CaptureImageError::InvalidImage)?;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(CaptureImageError::InvalidImage);
    }
    image::load_from_memory_with_format(bytes, format)
        .map_err(|_| CaptureImageError::InvalidImage)?;
    Ok(CaptureImageMetadata {
        media_type: media_type.to_owned(),
        width,
        height,
    })
}
