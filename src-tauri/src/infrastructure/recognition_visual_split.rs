//! Deterministic, model-free page splitting.
//!
//! This is a Rust deployment port of the validated `opencv-whitespace`
//! bakeoff baseline. It deliberately uses only foreground density and
//! whitespace geometry: no OCR text, subject inference, answer matching, or
//! downloadable model participates in the result.

use std::path::Path;

use image::GrayImage;

use crate::{
    infrastructure::capture_recognition_worker::{
        CaptureRecognitionEngine, RecognitionAnalysis, RecognitionEngineError,
    },
    modules::{
        capture_inbox::NormalizedCropRect,
        capture_recognition::{
            CaptureRecognitionReasonCode, CaptureRecognitionRegionProposal, CaptureRecognitionRole,
        },
    },
};

const MAX_INPUT_BYTES: u64 = 50 * 1024 * 1024;
const MAX_INPUT_PIXELS: u64 = 80_000_000;
const MAX_REGIONS: usize = 150;

pub struct VisualSplitRecognitionEngine;

impl CaptureRecognitionEngine for VisualSplitRecognitionEngine {
    fn analyze(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        let metadata = std::fs::metadata(image_path).map_err(|_| RecognitionEngineError::Failed)?;
        if metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES {
            return Err(RecognitionEngineError::InvalidResult);
        }
        let image = image::open(image_path)
            .map_err(|_| RecognitionEngineError::InvalidResult)?
            .into_luma8();
        analyze_gray(&image, staged_role)
    }
}

fn analyze_gray(
    image: &GrayImage,
    staged_role: CaptureRecognitionRole,
) -> Result<RecognitionAnalysis, RecognitionEngineError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    if width < 2 || height < 2 || (width as u64).saturating_mul(height as u64) > MAX_INPUT_PIXELS {
        return Err(RecognitionEngineError::InvalidResult);
    }

    let mask = opened_foreground(image);
    let foreground_count = mask.iter().filter(|value| **value).count();
    if foreground_count as f64 / ((width * height) as f64) < 0.0005 {
        return Ok(uncertain_full_page(staged_role, 1_000));
    }

    let columns = detect_columns(&mask, width, height);
    let mut regions = Vec::new();
    for (x_start, x_end) in columns.iter().copied() {
        let column_width = x_end.saturating_sub(x_start);
        if column_width == 0 {
            continue;
        }
        let row_density = (0..height)
            .map(|y| {
                let ink = (x_start..x_end)
                    .filter(|x| mask[index(*x, y, width)])
                    .count();
                ink as f64 / column_width as f64
            })
            .collect::<Vec<_>>();
        let positive = row_density
            .iter()
            .copied()
            .filter(|value| *value > 0.0)
            .collect::<Vec<_>>();
        let active_threshold = 0.004_f64.max(percentile(&positive, 20) * 0.15);
        let active = row_density
            .iter()
            .map(|density| *density >= active_threshold)
            .collect::<Vec<_>>();
        let line_groups = merge_runs(
            boolean_runs(&active),
            3_usize.max(((height as f64) * 0.02).round() as usize),
        )
        .into_iter()
        .filter(|(start, end)| end.saturating_sub(*start) >= 3)
        .collect::<Vec<_>>();
        let runs = merge_across_minor_block_gaps(line_groups, height);
        if runs.is_empty() {
            continue;
        }
        let strong_split = runs.len() > 1 || columns.len() > 1;
        let pad_y = 2_usize.max(((height as f64) * 0.01).round() as usize);
        for (run_index, (start, end)) in runs.iter().copied().enumerate() {
            let top = start.saturating_sub(pad_y);
            let bottom = height.min(end.saturating_add(pad_y));
            if bottom <= top {
                continue;
            }
            let previous_gap = if run_index == 0 {
                start
            } else {
                start.saturating_sub(runs[run_index - 1].1)
            };
            let next_gap = if run_index + 1 < runs.len() {
                runs[run_index + 1].0.saturating_sub(end)
            } else {
                height.saturating_sub(end)
            };
            let gap_evidence = (previous_gap.max(next_gap) as f64 / height as f64).min(0.18);
            let confidence = if strong_split {
                ((0.78 + gap_evidence).min(0.96) * 10_000.0).round() as u16
            } else {
                6_200
            };
            regions.push(CaptureRecognitionRegionProposal {
                rect: NormalizedCropRect {
                    x: x_start as f64 / width as f64,
                    y: top as f64 / height as f64,
                    width: column_width as f64 / width as f64,
                    height: (bottom - top) as f64 / height as f64,
                },
                role: staged_role,
                group_slot: None,
                confidence_basis_points: confidence,
            });
            if regions.len() > MAX_REGIONS {
                return Err(RecognitionEngineError::InvalidResult);
            }
        }
    }

    if regions.is_empty() {
        return Ok(uncertain_full_page(staged_role, 2_000));
    }
    regions.sort_by(|left, right| {
        left.rect
            .x
            .total_cmp(&right.rect.x)
            .then_with(|| left.rect.y.total_cmp(&right.rect.y))
    });
    let minimum_confidence = regions
        .iter()
        .map(|region| region.confidence_basis_points)
        .min()
        .unwrap_or(2_000);
    let reason_codes = if regions.len() > 1 {
        vec![CaptureRecognitionReasonCode::ConsistentReadingOrder]
    } else {
        vec![
            CaptureRecognitionReasonCode::WeakAnchor,
            CaptureRecognitionReasonCode::PossibleContentCut,
        ]
    };
    Ok(RecognitionAnalysis {
        regions,
        confidence_basis_points: minimum_confidence,
        reason_codes,
        pairing_tokens: Vec::new(),
    })
}

fn uncertain_full_page(
    role: CaptureRecognitionRole,
    confidence_basis_points: u16,
) -> RecognitionAnalysis {
    RecognitionAnalysis {
        regions: vec![CaptureRecognitionRegionProposal {
            rect: NormalizedCropRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            role,
            group_slot: None,
            confidence_basis_points,
        }],
        confidence_basis_points,
        reason_codes: vec![
            CaptureRecognitionReasonCode::WeakAnchor,
            CaptureRecognitionReasonCode::PossibleContentCut,
        ],
        pairing_tokens: Vec::new(),
    }
}

fn opened_foreground(image: &GrayImage) -> Vec<bool> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let threshold = otsu_threshold(image);
    let source = image
        .pixels()
        .map(|pixel| pixel[0] <= threshold)
        .collect::<Vec<_>>();
    let mut eroded = vec![false; source.len()];
    for y in 0..height.saturating_sub(1) {
        for x in 0..width.saturating_sub(1) {
            eroded[index(x, y, width)] = source[index(x, y, width)]
                && source[index(x + 1, y, width)]
                && source[index(x, y + 1, width)]
                && source[index(x + 1, y + 1, width)];
        }
    }
    let mut opened = vec![false; source.len()];
    for y in 0..height {
        for x in 0..width {
            if !eroded[index(x, y, width)] {
                continue;
            }
            opened[index(x, y, width)] = true;
            if x + 1 < width {
                opened[index(x + 1, y, width)] = true;
            }
            if y + 1 < height {
                opened[index(x, y + 1, width)] = true;
            }
            if x + 1 < width && y + 1 < height {
                opened[index(x + 1, y + 1, width)] = true;
            }
        }
    }
    opened
}

fn otsu_threshold(image: &GrayImage) -> u8 {
    let mut histogram = [0_u64; 256];
    for pixel in image.pixels() {
        histogram[pixel[0] as usize] += 1;
    }
    let total = u64::from(image.width()) * u64::from(image.height());
    let sum = histogram
        .iter()
        .enumerate()
        .map(|(value, count)| value as f64 * *count as f64)
        .sum::<f64>();
    let mut background_weight = 0_u64;
    let mut background_sum = 0.0;
    let mut best_variance = -1.0;
    let mut best_threshold = 0_u8;
    for (value, count) in histogram.iter().copied().enumerate() {
        background_weight += count;
        if background_weight == 0 {
            continue;
        }
        let foreground_weight = total.saturating_sub(background_weight);
        if foreground_weight == 0 {
            break;
        }
        background_sum += value as f64 * count as f64;
        let background_mean = background_sum / background_weight as f64;
        let foreground_mean = (sum - background_sum) / foreground_weight as f64;
        let variance = background_weight as f64
            * foreground_weight as f64
            * (background_mean - foreground_mean).powi(2);
        if variance > best_variance {
            best_variance = variance;
            best_threshold = value as u8;
        }
    }
    best_threshold
}

fn detect_columns(mask: &[bool], width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut x_min = width;
    let mut x_max = 0;
    let mut density = vec![0.0; width];
    for x in 0..width {
        let count = (0..height).filter(|y| mask[index(x, *y, width)]).count();
        density[x] = count as f64 / height as f64;
        if count > 0 {
            x_min = x_min.min(x);
            x_max = x_max.max(x + 1);
        }
    }
    if x_min >= x_max {
        return vec![(0, width)];
    }
    let central_start = ((width as f64) * 0.28) as usize;
    let central_end = (((width as f64) * 0.72) as usize).min(width);
    let positive = density
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect::<Vec<_>>();
    let low_threshold = (percentile(&positive, 20) * 0.08).clamp(0.001, 0.01);
    let low_ink = density[central_start..central_end]
        .iter()
        .map(|value| *value <= low_threshold)
        .collect::<Vec<_>>();
    let minimum_gap = 3_usize.max(((width as f64) * 0.03).round() as usize);
    let best_gap = boolean_runs(&low_ink)
        .into_iter()
        .map(|(start, end)| (start + central_start, end + central_start))
        .filter(|(start, end)| end.saturating_sub(*start) >= minimum_gap)
        .max_by_key(|(start, end)| end.saturating_sub(*start));
    let Some((gap_start, gap_end)) = best_gap else {
        return vec![padded_extent(x_min, x_max, width)];
    };
    let left_ink = (0..gap_start)
        .map(|x| (0..height).filter(|y| mask[index(x, *y, width)]).count())
        .sum::<usize>();
    let right_ink = (gap_end..width)
        .map(|x| (0..height).filter(|y| mask[index(x, *y, width)]).count())
        .sum::<usize>();
    let total_ink = left_ink + right_ink;
    if total_ink == 0
        || left_ink as f64 / (total_ink as f64) < 0.15
        || right_ink as f64 / (total_ink as f64) < 0.15
    {
        return vec![padded_extent(x_min, x_max, width)];
    }
    let left_min = (0..gap_start)
        .find(|x| (0..height).any(|y| mask[index(*x, y, width)]))
        .unwrap_or(0);
    let left_max = (0..gap_start)
        .rev()
        .find(|x| (0..height).any(|y| mask[index(*x, y, width)]))
        .map(|x| x + 1)
        .unwrap_or(gap_start);
    let right_min = (gap_end..width)
        .find(|x| (0..height).any(|y| mask[index(*x, y, width)]))
        .unwrap_or(gap_end);
    let right_max = (gap_end..width)
        .rev()
        .find(|x| (0..height).any(|y| mask[index(*x, y, width)]))
        .map(|x| x + 1)
        .unwrap_or(width);
    let padding = 2_usize.max(((width as f64) * 0.01).round() as usize);
    vec![
        (
            left_min.saturating_sub(padding),
            gap_start.min(left_max + padding),
        ),
        (
            gap_end.max(right_min.saturating_sub(padding)),
            width.min(right_max + padding),
        ),
    ]
}

fn padded_extent(start: usize, end: usize, width: usize) -> (usize, usize) {
    let padding = 2_usize.max(((width as f64) * 0.01).round() as usize);
    (start.saturating_sub(padding), width.min(end + padding))
}

fn boolean_runs(values: &[bool]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, active) in values.iter().copied().enumerate() {
        match (start, active) {
            (None, true) => start = Some(index),
            (Some(run_start), false) => {
                runs.push((run_start, index));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = start {
        runs.push((start, values.len()));
    }
    runs
}

fn merge_runs(runs: Vec<(usize, usize)>, maximum_gap: usize) -> Vec<(usize, usize)> {
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in runs {
        if let Some(last) = merged.last_mut() {
            if start.saturating_sub(last.1) <= maximum_gap {
                last.1 = end;
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

fn merge_across_minor_block_gaps(
    runs: Vec<(usize, usize)>,
    page_height: usize,
) -> Vec<(usize, usize)> {
    if runs.len() <= 2 {
        return runs;
    }
    let mut gaps = runs
        .windows(2)
        .map(|pair| pair[1].0.saturating_sub(pair[0].1))
        .collect::<Vec<_>>();
    gaps.sort_unstable();
    let smallest = gaps[0];
    let largest = *gaps.last().unwrap_or(&smallest);
    let gaps_are_uniform = largest <= smallest.saturating_mul(3) / 2;
    let relative_threshold = if gaps_are_uniform {
        smallest
    } else {
        let index = (((gaps.len() - 1) as f64) * 0.70).round() as usize;
        gaps[index]
    };
    let major_gap =
        relative_threshold.max(3_usize.max(((page_height as f64) * 0.025).round() as usize));
    let mut grouped = Vec::<(usize, usize)>::new();
    for (start, end) in runs {
        if let Some(previous) = grouped.last_mut() {
            let gap = start.saturating_sub(previous.1);
            let is_minor_gap = if gaps_are_uniform {
                gap < major_gap
            } else {
                gap <= major_gap
            };
            if is_minor_gap {
                previous.1 = end;
                continue;
            }
        }
        grouped.push((start, end));
    }
    grouped
}

fn percentile(values: &[f64], percentile: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() - 1) * percentile) / 100;
    sorted[index]
}

const fn index(x: usize, y: usize, width: usize) -> usize {
    y * width + x
}

#[cfg(test)]
mod tests {
    use image::{GrayImage, Luma};

    use super::*;

    fn white_page(width: u32, height: u32) -> GrayImage {
        GrayImage::from_pixel(width, height, Luma([255]))
    }

    fn draw_question(image: &mut GrayImage, x0: u32, x1: u32, y0: u32, y1: u32) {
        for y in (y0..y1).step_by(18) {
            for yy in y..(y + 8).min(y1) {
                for x in x0..x1 {
                    image.put_pixel(x, yy, Luma([20]));
                }
            }
        }
    }

    #[test]
    fn splits_vertical_blocks_in_reading_order_and_inherits_answer_role() {
        let mut page = white_page(700, 1_000);
        draw_question(&mut page, 55, 640, 80, 230);
        draw_question(&mut page, 55, 640, 370, 540);
        draw_question(&mut page, 55, 640, 700, 900);

        let result = analyze_gray(&page, CaptureRecognitionRole::Answer).unwrap();

        assert_eq!(result.regions.len(), 3);
        assert!(
            result
                .regions
                .windows(2)
                .all(|pair| pair[0].rect.y < pair[1].rect.y)
        );
        assert!(result.regions.iter().all(|region| {
            region.role == CaptureRecognitionRole::Answer
                && region.group_slot.is_none()
                && region.confidence_basis_points >= 7_500
        }));
        assert!(result.pairing_tokens.is_empty());
    }

    #[test]
    fn detects_two_columns_only_when_both_sides_have_ink() {
        let mut page = white_page(1_000, 900);
        draw_question(&mut page, 55, 420, 80, 250);
        draw_question(&mut page, 55, 420, 530, 760);
        draw_question(&mut page, 580, 945, 100, 300);
        draw_question(&mut page, 580, 945, 560, 790);

        let result = analyze_gray(&page, CaptureRecognitionRole::Question).unwrap();

        assert_eq!(result.regions.len(), 4);
        assert!(result.regions[1].rect.x < 0.2);
        assert!(result.regions[2].rect.x > 0.4);
    }

    #[test]
    fn keeps_options_and_explanation_with_their_question_when_the_next_gap_is_larger() {
        let mut page = white_page(700, 1_000);
        draw_question(&mut page, 55, 640, 80, 135);
        draw_question(&mut page, 80, 620, 175, 210);
        draw_question(&mut page, 80, 620, 250, 285);
        draw_question(&mut page, 80, 620, 325, 360);
        draw_question(&mut page, 55, 640, 560, 620);
        draw_question(&mut page, 80, 620, 660, 700);

        let result = analyze_gray(&page, CaptureRecognitionRole::Question).unwrap();

        assert_eq!(result.regions.len(), 2);
        assert!(result.regions[0].rect.height > 0.25);
        assert!(result.regions[1].rect.y > 0.5);
    }

    #[test]
    fn blank_page_returns_one_low_confidence_review_region() {
        let result =
            analyze_gray(&white_page(700, 1_000), CaptureRecognitionRole::Question).unwrap();

        assert_eq!(result.regions.len(), 1);
        assert_eq!(result.regions[0].rect.width, 1.0);
        assert!(result.confidence_basis_points < 6_500);
        assert!(
            result
                .reason_codes
                .contains(&CaptureRecognitionReasonCode::WeakAnchor)
        );
    }

    #[test]
    #[ignore = "developer probe: set VISUAL_SPLIT_SAMPLE_DIR to inspect local image samples"]
    fn probe_local_sample_directory() {
        let directory = std::env::var_os("VISUAL_SPLIT_SAMPLE_DIR")
            .map(std::path::PathBuf::from)
            .expect("VISUAL_SPLIT_SAMPLE_DIR is required");
        let engine = VisualSplitRecognitionEngine;
        let mut paths = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                matches!(
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("png" | "jpg" | "jpeg" | "webp")
                )
            })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let result = engine
                .analyze(&path, CaptureRecognitionRole::Question)
                .unwrap();
            println!(
                "{}\t{}\t{}\t{:?}",
                path.file_name().unwrap().to_string_lossy(),
                result.regions.len(),
                result.confidence_basis_points,
                result
                    .regions
                    .iter()
                    .map(|region| region.confidence_basis_points)
                    .collect::<Vec<_>>()
            );
        }
    }
}
