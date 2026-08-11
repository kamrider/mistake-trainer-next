use std::path::Path;

use image::{DynamicImage, GrayImage};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::infrastructure::assets::decrypt_asset;

use super::capture_inbox::{
    CaptureInboxError, NormalizedCropRect, image_format_for_media_type, query_batch,
    read_encrypted_blob,
};

const ANALYSIS_MAX_EDGE: u32 = 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureQualityIssueCode {
    Blurry,
    TooDark,
    TooBright,
    LowContrast,
    PossibleEdgeCut,
    Skewed,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureQualityReport {
    pub item_id: String,
    pub issues: Vec<CaptureQualityIssueCode>,
    pub sharpness_score: f64,
    pub dark_fraction: f64,
    pub bright_fraction: f64,
    pub contrast_score: f64,
    pub suggested_rotation_degrees: f64,
    pub suggested_crop: Option<NormalizedCropRect>,
}

pub fn check_capture_quality(
    connection: &Connection,
    blob_root: &Path,
    key: &[u8; 32],
    account_id: &str,
    profile_id: &str,
    batch_id: &str,
    item_id: &str,
) -> Result<CaptureQualityReport, CaptureInboxError> {
    query_batch(connection, account_id, profile_id, batch_id)?;
    let (media_type, encrypted_path) = connection
        .query_row(
            "SELECT a.media_type, a.encrypted_path FROM capture_items i
             JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1 AND i.batch_id = ?2 AND a.account_id = ?3
               AND i.superseded_by_derivation_id IS NULL",
            params![item_id, batch_id, account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(CaptureInboxError::ItemNotFound)?;
    let encrypted = read_encrypted_blob(blob_root, &encrypted_path)?;
    let plaintext = decrypt_asset(&encrypted, key).map_err(|_| CaptureInboxError::Crypto)?;
    let image =
        image::load_from_memory_with_format(&plaintext, image_format_for_media_type(&media_type)?)
            .map_err(|_| CaptureInboxError::InvalidImage)?;
    Ok(analyze_capture_image(item_id, &image))
}

pub fn analyze_capture_image(item_id: &str, source: &DynamicImage) -> CaptureQualityReport {
    let image = source
        .thumbnail(ANALYSIS_MAX_EDGE, ANALYSIS_MAX_EDGE)
        .to_luma8();
    let total = f64::from(image.width()) * f64::from(image.height());
    if total == 0.0 {
        return empty_report(item_id);
    }

    let mut histogram = [0_u64; 256];
    let mut dark_count = 0_u64;
    let mut bright_count = 0_u64;
    let mut luma_sum = 0_u64;
    for pixel in image.pixels() {
        let luma = pixel[0];
        histogram[usize::from(luma)] += 1;
        luma_sum += u64::from(luma);
        if luma <= 35 {
            dark_count += 1;
        }
        if luma >= 248 {
            bright_count += 1;
        }
    }

    let pixel_count = total as u64;
    let p05 = percentile(&histogram, pixel_count, 0.05);
    let p50 = percentile(&histogram, pixel_count, 0.50);
    let p95 = percentile(&histogram, pixel_count, 0.95);
    let contrast_score = f64::from(p95.saturating_sub(p05)) / 255.0;
    let dark_fraction = dark_count as f64 / total;
    let bright_fraction = bright_count as f64 / total;
    let mean_luma = luma_sum as f64 / total;
    let sharpness_score = laplacian_variance(&image);
    let foreground_threshold = p50.saturating_sub(18).min(210);
    let foreground = foreground_bounds(&image, foreground_threshold);
    let edge_cut = foreground
        .as_ref()
        .is_some_and(|bounds| foreground_touches_edge(&image, foreground_threshold, bounds.count));
    let suggested_crop = foreground
        .as_ref()
        .and_then(|bounds| suggested_crop(&image, bounds, edge_cut));
    let suggested_rotation_degrees = estimate_rotation(&image, foreground_threshold);

    let mut issues = Vec::new();
    if sharpness_score < 0.003 && contrast_score >= 0.10 {
        issues.push(CaptureQualityIssueCode::Blurry);
    }
    if dark_fraction > 0.52 || mean_luma < 58.0 {
        issues.push(CaptureQualityIssueCode::TooDark);
    }
    if bright_fraction > 0.94 && contrast_score < 0.12 {
        issues.push(CaptureQualityIssueCode::TooBright);
    }
    if contrast_score < 0.12 {
        issues.push(CaptureQualityIssueCode::LowContrast);
    }
    if edge_cut {
        issues.push(CaptureQualityIssueCode::PossibleEdgeCut);
    }
    if suggested_rotation_degrees.abs() >= 1.5 {
        issues.push(CaptureQualityIssueCode::Skewed);
    }

    CaptureQualityReport {
        item_id: item_id.to_owned(),
        issues,
        sharpness_score: round_score(sharpness_score),
        dark_fraction: round_score(dark_fraction),
        bright_fraction: round_score(bright_fraction),
        contrast_score: round_score(contrast_score),
        suggested_rotation_degrees: (suggested_rotation_degrees * 10.0).round() / 10.0,
        suggested_crop,
    }
}

fn empty_report(item_id: &str) -> CaptureQualityReport {
    CaptureQualityReport {
        item_id: item_id.to_owned(),
        issues: vec![CaptureQualityIssueCode::LowContrast],
        sharpness_score: 0.0,
        dark_fraction: 0.0,
        bright_fraction: 0.0,
        contrast_score: 0.0,
        suggested_rotation_degrees: 0.0,
        suggested_crop: None,
    }
}

fn percentile(histogram: &[u64; 256], count: u64, quantile: f64) -> u8 {
    let target = ((count.saturating_sub(1)) as f64 * quantile).round() as u64;
    let mut cumulative = 0_u64;
    for (value, frequency) in histogram.iter().enumerate() {
        cumulative += frequency;
        if cumulative > target {
            return value as u8;
        }
    }
    255
}

fn laplacian_variance(image: &GrayImage) -> f64 {
    if image.width() < 3 || image.height() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut sum_squares = 0.0;
    let mut count = 0.0;
    for y in 1..image.height() - 1 {
        for x in 1..image.width() - 1 {
            let center = f64::from(image.get_pixel(x, y)[0]);
            let laplacian = f64::from(image.get_pixel(x - 1, y)[0])
                + f64::from(image.get_pixel(x + 1, y)[0])
                + f64::from(image.get_pixel(x, y - 1)[0])
                + f64::from(image.get_pixel(x, y + 1)[0])
                - 4.0 * center;
            sum += laplacian;
            sum_squares += laplacian * laplacian;
            count += 1.0;
        }
    }
    let variance = (sum_squares / count) - (sum / count).powi(2);
    (variance / 65_025.0).clamp(0.0, 1.0)
}

struct ForegroundBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    count: u64,
}

fn foreground_bounds(image: &GrayImage, threshold: u8) -> Option<ForegroundBounds> {
    let mut bounds = ForegroundBounds {
        min_x: image.width(),
        min_y: image.height(),
        max_x: 0,
        max_y: 0,
        count: 0,
    };
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[0] >= threshold {
            continue;
        }
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
        bounds.count += 1;
    }
    (bounds.count >= 16).then_some(bounds)
}

fn foreground_touches_edge(image: &GrayImage, threshold: u8, foreground_count: u64) -> bool {
    let band_x = (image.width() / 40).max(1);
    let band_y = (image.height() / 40).max(1);
    let mut edge_foreground = 0_u64;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[0] < threshold
            && (x < band_x
                || x >= image.width() - band_x
                || y < band_y
                || y >= image.height() - band_y)
        {
            edge_foreground += 1;
        }
    }
    foreground_count >= 16 && edge_foreground as f64 / foreground_count as f64 > 0.08
}

fn suggested_crop(
    image: &GrayImage,
    bounds: &ForegroundBounds,
    edge_cut: bool,
) -> Option<NormalizedCropRect> {
    if edge_cut {
        return None;
    }
    let padding_x = (image.width() / 50).max(2);
    let padding_y = (image.height() / 50).max(2);
    let min_x = bounds.min_x.saturating_sub(padding_x);
    let min_y = bounds.min_y.saturating_sub(padding_y);
    let max_x = bounds
        .max_x
        .saturating_add(padding_x)
        .min(image.width() - 1);
    let max_y = bounds
        .max_y
        .saturating_add(padding_y)
        .min(image.height() - 1);
    let width = max_x - min_x + 1;
    let height = max_y - min_y + 1;
    if width as f64 / f64::from(image.width()) > 0.94
        && height as f64 / f64::from(image.height()) > 0.94
    {
        return None;
    }
    Some(NormalizedCropRect {
        x: f64::from(min_x) / f64::from(image.width()),
        y: f64::from(min_y) / f64::from(image.height()),
        width: f64::from(width) / f64::from(image.width()),
        height: f64::from(height) / f64::from(image.height()),
    })
}

fn estimate_rotation(image: &GrayImage, threshold: u8) -> f64 {
    let mut count = 0.0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_xy = 0.0;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[0] >= threshold {
            continue;
        }
        let x = f64::from(x);
        let y = f64::from(y);
        count += 1.0;
        sum_x += x;
        sum_y += y;
        sum_xx += x * x;
        sum_yy += y * y;
        sum_xy += x * y;
    }
    if count < 32.0 {
        return 0.0;
    }
    let variance_x = sum_xx / count - (sum_x / count).powi(2);
    let variance_y = sum_yy / count - (sum_y / count).powi(2);
    let covariance = sum_xy / count - (sum_x / count) * (sum_y / count);
    let trace = variance_x + variance_y;
    let discriminant = ((variance_x - variance_y).powi(2) + 4.0 * covariance.powi(2)).sqrt();
    let major = (trace + discriminant) / 2.0;
    let minor = ((trace - discriminant) / 2.0).max(0.001);
    if major / minor < 2.5 {
        return 0.0;
    }
    let angle = 0.5
        * (2.0 * covariance)
            .atan2(variance_x - variance_y)
            .to_degrees();
    if angle.abs() > 8.0 {
        0.0
    } else {
        -angle.clamp(-8.0, 8.0)
    }
}

fn round_score(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GrayImage, Luma};

    use super::{CaptureQualityIssueCode, analyze_capture_image};

    fn document_fixture() -> DynamicImage {
        let mut image = GrayImage::from_pixel(320, 220, Luma([238]));
        for row in 0..6 {
            let y = 30 + row * 28;
            for py in y..y + 4 {
                for x in 40..280 {
                    image.put_pixel(x, py, Luma([28]));
                }
            }
        }
        DynamicImage::ImageLuma8(image)
    }

    #[test]
    fn clean_document_remains_quiet_and_suggests_safe_crop() {
        let report = analyze_capture_image("item", &document_fixture());

        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert!(report.sharpness_score > 0.003, "{}", report.sharpness_score);
        assert!(report.contrast_score > 0.5);
        assert!(report.suggested_crop.is_some());
    }

    #[test]
    fn flat_bright_image_reports_exposure_and_contrast() {
        let image = DynamicImage::ImageLuma8(GrayImage::from_pixel(160, 120, Luma([254])));
        let report = analyze_capture_image("bright", &image);

        assert!(report.issues.contains(&CaptureQualityIssueCode::TooBright));
        assert!(
            report
                .issues
                .contains(&CaptureQualityIssueCode::LowContrast)
        );
    }

    #[test]
    fn smooth_high_contrast_gradient_is_reported_as_blurry() {
        let mut image = GrayImage::new(320, 180);
        for (x, _y, pixel) in image.enumerate_pixels_mut() {
            let luma = 25 + ((f64::from(x) / 319.0) * 210.0).round() as u8;
            *pixel = Luma([luma]);
        }
        let report = analyze_capture_image("blur", &DynamicImage::ImageLuma8(image));

        assert!(report.issues.contains(&CaptureQualityIssueCode::Blurry));
    }

    #[test]
    fn foreground_at_edge_is_reported_without_unsafe_crop_suggestion() {
        let mut image = GrayImage::from_pixel(200, 120, Luma([240]));
        for y in 25..95 {
            for x in 0..12 {
                image.put_pixel(x, y, Luma([20]));
            }
        }
        let report = analyze_capture_image("edge", &DynamicImage::ImageLuma8(image));

        assert!(
            report
                .issues
                .contains(&CaptureQualityIssueCode::PossibleEdgeCut)
        );
        assert!(report.suggested_crop.is_none());
    }

    #[test]
    fn slight_baseline_skew_returns_conservative_correction() {
        let mut image = GrayImage::from_pixel(360, 180, Luma([240]));
        for x in 35..325 {
            let y = 70 + ((x - 35) as f64 * 0.07).round() as u32;
            for py in y..y + 5 {
                image.put_pixel(x, py, Luma([20]));
            }
        }
        let report = analyze_capture_image("skew", &DynamicImage::ImageLuma8(image));

        assert!(report.issues.contains(&CaptureQualityIssueCode::Skewed));
        assert!((-8.0..=-1.5).contains(&report.suggested_rotation_degrees));
    }
}
