use std::path::Path;

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

// ppocr-rs reports the conservative minimum of detector and recognizer scores.
// On readable low-resolution exam scans, clear numbered lines commonly land
// around 0.62-0.70 even when the recognized number and text are correct.
const MIN_ANCHOR_CONFIDENCE: f64 = 0.60;
// A final short marker such as `4.` can score slightly below the normal text
// threshold on answer sheets. It is admitted only when at least two strong
// anchors establish the same left alignment; the consecutive-run selector
// still has to validate the numbering.
const MIN_ALIGNED_ANCHOR_CONFIDENCE: f64 = 0.55;
const FALLBACK_CONFIDENCE_BASIS_POINTS: u16 = 4_500;

#[derive(Clone, Debug, PartialEq)]
pub struct LocalOcrBox {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub text: String,
    pub confidence: f64,
    pub is_text: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalOcrPage {
    pub width: u32,
    pub height: u32,
    pub boxes: Vec<LocalOcrBox>,
}

pub trait LocalOcrRuntime: Send + Sync {
    fn recognize(&self, image_path: &Path) -> Result<LocalOcrPage, RecognitionEngineError>;
}

pub struct AnchorRecognitionEngine<R> {
    runtime: R,
}

impl<R> AnchorRecognitionEngine<R> {
    pub const fn new(runtime: R) -> Self {
        Self { runtime }
    }
}

impl<R: LocalOcrRuntime> CaptureRecognitionEngine for AnchorRecognitionEngine<R> {
    fn analyze(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        let page = self.runtime.recognize(image_path)?;
        analyze_ocr_page(page, staged_role)
    }
}

pub fn analyze_ocr_page(
    page: LocalOcrPage,
    staged_role: CaptureRecognitionRole,
) -> Result<RecognitionAnalysis, RecognitionEngineError> {
    validate_page(&page)?;
    let strong_anchors = page
        .boxes
        .iter()
        .filter_map(|entry| {
            (entry.is_text && entry.confidence >= MIN_ANCHOR_CONFIDENCE)
                .then(|| question_anchor_number(&entry.text).map(|number| (entry, number)))
                .flatten()
        })
        .collect::<Vec<_>>();
    let strong_lefts = strong_anchors
        .iter()
        .map(|(entry, _)| entry.left)
        .collect::<Vec<_>>();
    let mut anchors = strong_anchors;
    anchors.extend(page.boxes.iter().filter_map(|entry| {
        if !entry.is_text
            || entry.confidence < MIN_ALIGNED_ANCHOR_CONFIDENCE
            || question_anchor_number(&entry.text).is_some()
                && entry.confidence >= MIN_ANCHOR_CONFIDENCE
        {
            return None;
        }
        if entry.confidence < MIN_ANCHOR_CONFIDENCE && strong_lefts.len() < 2 {
            return None;
        }
        let number = relaxed_question_anchor_number(&entry.text).or_else(|| {
            // Answer explanations can start with a number immediately after
            // the question marker (for example `1.2, 8, 14...`). Keep the
            // strict decimal guard, and only admit this ambiguous shape when
            // unambiguous anchors on the same page provide an alignment and
            // consecutive-sequence check.
            (!strong_lefts.is_empty())
                .then(|| ambiguous_punctuated_question_anchor_number(&entry.text))
                .flatten()
        })?;
        let aligned = entry.left <= f64::from(page.width) * 0.15
            || strong_lefts
                .iter()
                .any(|left| (entry.left - left).abs() <= f64::from(page.width) * 0.035);
        aligned.then_some((entry, number))
    }));
    if anchors.len() < 2 {
        return Ok(fallback_analysis(
            staged_role,
            CaptureRecognitionReasonCode::WeakAnchor,
        ));
    }

    let columns = anchor_columns(page.width, &anchors)
        .into_iter()
        .map(select_question_run)
        .collect::<Vec<_>>();
    if columns.iter().any(|column| column.len() < 2) {
        return Ok(fallback_analysis(
            staged_role,
            CaptureRecognitionReasonCode::WeakAnchor,
        ));
    }
    let bounds = column_bounds(page.width, &columns);

    let mut regions = Vec::with_capacity(anchors.len());
    let mut pairing_tokens = Vec::with_capacity(anchors.len());
    for (column_anchors, (left, right)) in columns.iter().zip(bounds) {
        for (index, (anchor, number)) in column_anchors.iter().enumerate() {
            let next_top = column_anchors.get(index + 1).map_or_else(
                || last_question_bottom(&page.boxes, anchor, left, right, page.height),
                |(entry, _)| entry.top,
            );
            let overlap_y = f64::from(page.height) * 0.015;
            let content_left = (anchor.left - f64::from(page.width) * 0.01).max(left);
            if page.boxes.iter().any(|entry| {
                let crossed_anchor_count = column_anchors
                    .iter()
                    .filter(|(candidate, _)| {
                        entry.top < candidate.top - overlap_y
                            && candidate.top + overlap_y < entry.bottom
                    })
                    .count();
                !entry.is_text
                    && entry.right >= content_left
                    && entry.left <= right
                    && entry.top < next_top - overlap_y
                    && next_top + overlap_y < entry.bottom
                    // A detector-only box covering several numbered rows is
                    // an overlaid watermark/stamp, not content owned by one
                    // question. A box crossing exactly one boundary remains
                    // protected and falls back to manual review.
                    && (column_anchors.len() < 3 || crossed_anchor_count < 2)
            }) {
                return Ok(fallback_analysis(
                    staged_role,
                    CaptureRecognitionReasonCode::PossibleContentCut,
                ));
            }
            let confidence = (anchor.confidence * 9_600.0)
                .clamp(7_500.0, 9_600.0)
                .round() as u16;
            let top = (anchor.top - overlap_y).max(0.0);
            let bottom = (next_top + overlap_y).min(f64::from(page.height));
            regions.push(CaptureRecognitionRegionProposal {
                rect: NormalizedCropRect {
                    x: left / f64::from(page.width),
                    y: top / f64::from(page.height),
                    width: (right - left) / f64::from(page.width),
                    height: (bottom - top) / f64::from(page.height),
                },
                role: staged_role,
                group_slot: None,
                confidence_basis_points: confidence,
            });
            pairing_tokens.push(Some(*number));
        }
    }
    let confidence_basis_points = regions
        .iter()
        .map(|region| region.confidence_basis_points)
        .min()
        .unwrap_or(FALLBACK_CONFIDENCE_BASIS_POINTS);
    Ok(RecognitionAnalysis {
        regions,
        confidence_basis_points,
        reason_codes: vec![
            CaptureRecognitionReasonCode::ClearQuestionAnchor,
            CaptureRecognitionReasonCode::ConsistentReadingOrder,
        ],
        pairing_tokens,
    })
}

fn validate_page(page: &LocalOcrPage) -> Result<(), RecognitionEngineError> {
    if page.width == 0 || page.height == 0 {
        return Err(RecognitionEngineError::InvalidResult);
    }
    let width = f64::from(page.width);
    let height = f64::from(page.height);
    if page.boxes.iter().any(|entry| {
        [
            entry.left,
            entry.top,
            entry.right,
            entry.bottom,
            entry.confidence,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || entry.left < 0.0
            || entry.top < 0.0
            || entry.right > width
            || entry.bottom > height
            || entry.right <= entry.left
            || entry.bottom <= entry.top
            || !(0.0..=1.0).contains(&entry.confidence)
    }) {
        return Err(RecognitionEngineError::InvalidResult);
    }
    Ok(())
}

fn fallback_analysis(
    role: CaptureRecognitionRole,
    reason: CaptureRecognitionReasonCode,
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
            confidence_basis_points: FALLBACK_CONFIDENCE_BASIS_POINTS,
        }],
        confidence_basis_points: FALLBACK_CONFIDENCE_BASIS_POINTS,
        reason_codes: vec![reason],
        pairing_tokens: vec![None],
    }
}

fn anchor_columns<'a>(
    width: u32,
    anchors: &[(&'a LocalOcrBox, u16)],
) -> Vec<Vec<(&'a LocalOcrBox, u16)>> {
    let mut ordered = anchors.to_vec();
    ordered.sort_by(|(left, _), (right, _)| left.left.total_cmp(&right.left));
    if ordered.len() < 4 {
        ordered.sort_by(|(left, _), (right, _)| left.top.total_cmp(&right.top));
        return vec![ordered];
    }
    let (largest_gap, split_index) = ordered
        .windows(2)
        .enumerate()
        .map(|(index, pair)| (pair[1].0.left - pair[0].0.left, index))
        .max_by(|(left, _), (right, _)| left.total_cmp(right))
        .expect("four anchors always contain a gap");
    let mut right = ordered.split_off(split_index + 1);
    let mut left = ordered;
    if largest_gap < f64::from(width) * 0.20 || left.len() < 2 || right.len() < 2 {
        left.append(&mut right);
        left.sort_by(|(first, _), (second, _)| first.top.total_cmp(&second.top));
        return vec![left];
    }
    left.sort_by(|(first, _), (second, _)| first.top.total_cmp(&second.top));
    right.sort_by(|(first, _), (second, _)| first.top.total_cmp(&second.top));
    vec![left, right]
}

fn select_question_run(mut anchors: Vec<(&LocalOcrBox, u16)>) -> Vec<(&LocalOcrBox, u16)> {
    anchors.sort_by(|(left, _), (right, _)| left.top.total_cmp(&right.top));
    let mut deduplicated: Vec<(&LocalOcrBox, u16)> = Vec::with_capacity(anchors.len());
    for candidate in anchors {
        if let Some(current) = deduplicated.last_mut()
            && (candidate.0.top - current.0.top).abs() <= 2.0
        {
            if candidate.0.confidence > current.0.confidence {
                *current = candidate;
            }
            continue;
        }
        deduplicated.push(candidate);
    }

    // Formula fragments can look like aligned bare question numbers. Build
    // the longest consecutive subsequence while allowing those noisy
    // candidates to be skipped. For equal lengths, a later sequence is more
    // likely to be the actual question section after numbered instructions.
    let mut best_ending_at: std::collections::HashMap<u16, Vec<(&LocalOcrBox, u16)>> =
        std::collections::HashMap::new();
    let mut runs: Vec<Vec<(&LocalOcrBox, u16)>> = Vec::new();
    for candidate in deduplicated {
        let mut run = candidate
            .1
            .checked_sub(1)
            .and_then(|previous| best_ending_at.get(&previous).cloned())
            .unwrap_or_default();
        run.push(candidate);
        let replace = best_ending_at
            .get(&candidate.1)
            .is_none_or(|current| (run.len(), run[0].0.top).ge(&(current.len(), current[0].0.top)));
        if replace {
            best_ending_at.insert(candidate.1, run.clone());
        }
        runs.push(run);
    }
    runs.into_iter()
        .max_by(|left, right| {
            left.len().cmp(&right.len()).then_with(|| {
                left.first()
                    .map_or(f64::NEG_INFINITY, |(entry, _)| entry.top)
                    .total_cmp(
                        &right
                            .first()
                            .map_or(f64::NEG_INFINITY, |(entry, _)| entry.top),
                    )
            })
        })
        .unwrap_or_default()
}

fn column_bounds(width: u32, columns: &[Vec<(&LocalOcrBox, u16)>]) -> Vec<(f64, f64)> {
    if columns.len() == 1 {
        return vec![(0.0, f64::from(width))];
    }
    let left_content_edge = columns[0]
        .iter()
        .map(|(entry, _)| entry.right)
        .fold(f64::NEG_INFINITY, f64::max);
    let right_content_edge = columns[1]
        .iter()
        .map(|(entry, _)| entry.left)
        .fold(f64::INFINITY, f64::min);
    let split = if left_content_edge < right_content_edge {
        (left_content_edge + right_content_edge) / 2.0
    } else {
        let left_anchor = columns[0]
            .iter()
            .map(|(entry, _)| entry.left)
            .fold(f64::NEG_INFINITY, f64::max);
        let right_anchor = columns[1]
            .iter()
            .map(|(entry, _)| entry.left)
            .fold(f64::INFINITY, f64::min);
        (left_anchor + right_anchor) / 2.0
    };
    let overlap = f64::from(width) * 0.025;
    vec![
        (0.0, (split + overlap).min(f64::from(width))),
        ((split - overlap).max(0.0), f64::from(width)),
    ]
}

fn last_question_bottom(
    boxes: &[LocalOcrBox],
    anchor: &LocalOcrBox,
    left_bound: f64,
    right_bound: f64,
    height: u32,
) -> f64 {
    let height = f64::from(height);
    let candidates = boxes
        .iter()
        .filter(|entry| {
            let center_x = (entry.left + entry.right) / 2.0;
            left_bound <= center_x
                && center_x <= right_bound
                && entry.bottom > anchor.top
                && entry.top >= anchor.top - height * 0.01
                && !looks_like_page_footer(&entry.text, entry.top / height)
        })
        .collect::<Vec<_>>();
    let group_bottom = candidates
        .iter()
        .map(|entry| entry.bottom)
        .fold(anchor.bottom, f64::max);
    // Region assembly adds another 1.5%, leaving a total 2.5% conservative
    // tail margin without carrying a whole blank page footer.
    (group_bottom + height * 0.01).min(height)
}

fn looks_like_page_footer(text: &str, normalized_top: f64) -> bool {
    if normalized_top < 0.90 {
        return false;
    }
    let trimmed = text.trim();
    trimmed.contains('页')
        || (!trimmed.is_empty()
            && trimmed
                .chars()
                .all(|character| character.is_ascii_digit() || "-—–".contains(character)))
}

fn question_anchor_number(text: &str) -> Option<u16> {
    let trimmed = text.trim_start();
    if option_marker(trimmed) {
        return None;
    }
    parenthesized_number(trimmed)
        .or_else(|| q_number(trimmed))
        .or_else(|| chinese_number(trimmed))
        .or_else(|| punctuated_number(trimmed))
}

fn relaxed_question_anchor_number(text: &str) -> Option<u16> {
    let trimmed = text.trim_start();
    if option_marker(trimmed) {
        return None;
    }
    let (number, consumed) = leading_number(trimmed)?;
    let suffix = &trimmed[consumed..];
    (suffix.is_empty() || suffix.chars().next().is_some_and(char::is_whitespace)).then_some(number)
}

fn ambiguous_punctuated_question_anchor_number(text: &str) -> Option<u16> {
    let trimmed = text.trim_start();
    if option_marker(trimmed) {
        return None;
    }
    let (number, consumed) = leading_number(trimmed)?;
    trimmed[consumed..]
        .chars()
        .next()
        .is_some_and(is_marker)
        .then_some(number)
}

fn option_marker(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let option = matches!(first, 'A'..='H' | 'a'..='h' | 'Ａ'..='Ｈ' | 'ａ'..='ｈ');
    option && chars.next().is_some_and(is_marker)
}

fn parenthesized_number(text: &str) -> Option<u16> {
    let mut chars = text.chars();
    let opening = chars.next()?;
    if !matches!(opening, '(' | '（') {
        return None;
    }
    let (number, consumed) = leading_number(chars.as_str())?;
    let suffix = &chars.as_str()[consumed..];
    let mut suffix_chars = suffix.chars();
    if !matches!(suffix_chars.next(), Some(')' | '）')) {
        return None;
    }
    no_following_digit(suffix_chars.as_str()).then_some(number)
}

fn punctuated_number(text: &str) -> Option<u16> {
    let (number, consumed) = leading_number(text)?;
    let suffix = &text[consumed..];
    let mut suffix_chars = suffix.chars();
    if !suffix_chars.next().is_some_and(is_marker) {
        return None;
    }
    no_following_digit(suffix_chars.as_str()).then_some(number)
}

fn q_number(text: &str) -> Option<u16> {
    let mut chars = text.chars();
    if !matches!(chars.next(), Some('Q' | 'q')) {
        return None;
    }
    let rest = chars.as_str();
    let (number, consumed) = leading_number(rest)?;
    let suffix = &rest[consumed..];
    let mut suffix_chars = suffix.chars();
    if !suffix_chars.next().is_some_and(is_q_marker) {
        return None;
    }
    no_following_digit(suffix_chars.as_str()).then_some(number)
}

fn chinese_number(text: &str) -> Option<u16> {
    let rest = text.strip_prefix('第')?.trim_start();
    let (number, consumed) = leading_number(rest)?;
    let suffix = rest[consumed..].trim_start();
    suffix.strip_prefix('题').map(|_| number)
}

fn leading_number(text: &str) -> Option<(u16, usize)> {
    let mut byte_length = 0;
    let mut value = 0_u16;
    let mut digits = 0_u8;
    for character in text.chars() {
        let digit = character.to_digit(10)?;
        if digits == 3 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(digit as u16)?;
        byte_length += character.len_utf8();
        digits += 1;
        if text[byte_length..]
            .chars()
            .next()
            .is_none_or(|next| next.to_digit(10).is_none())
        {
            break;
        }
    }
    (digits > 0).then_some((value, byte_length))
}

fn no_following_digit(text: &str) -> bool {
    text.chars()
        .next()
        .is_none_or(|character| character.to_digit(10).is_none())
}

fn is_marker(character: char) -> bool {
    matches!(character, '.' | '、' | '．' | ')')
}

fn is_q_marker(character: char) -> bool {
    is_marker(character) || matches!(character, ':' | '：')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_box(left: f64, top: f64, right: f64, bottom: f64, text: &str) -> LocalOcrBox {
        LocalOcrBox {
            left,
            top,
            right,
            bottom,
            text: text.to_owned(),
            confidence: 0.95,
            is_text: true,
        }
    }

    #[test]
    fn recognizes_supported_question_markers_without_treating_options_or_decimals_as_questions() {
        for (text, expected) in [
            ("(1) first", Some(1)),
            ("（12）second", Some(12)),
            ("23、third", Some(23)),
            ("Q7: fourth", Some(7)),
            ("第 99 题 fifth", Some(99)),
            ("A. option", None),
            ("Ｇ． option", None),
            ("3.14", None),
            ("1234. too long", None),
        ] {
            assert_eq!(question_anchor_number(text), expected, "{text}");
        }
    }

    #[test]
    fn recovers_an_ambiguous_numeric_first_question_only_from_an_aligned_sequence() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_200,
                boxes: vec![
                    text_box(40.0, 100.0, 180.0, 140.0, "1.2, 8, 14, 16"),
                    text_box(40.0, 350.0, 180.0, 390.0, "2. second"),
                    text_box(40.0, 600.0, 180.0, 640.0, "3. third"),
                    text_box(300.0, 800.0, 420.0, 840.0, "3.14"),
                ],
            },
            CaptureRecognitionRole::Answer,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 3);
        assert_eq!(result.pairing_tokens, vec![Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn creates_conservative_full_width_regions_for_a_single_column() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_200,
                boxes: vec![
                    text_box(40.0, 100.0, 120.0, 140.0, "1. first"),
                    text_box(40.0, 500.0, 120.0, 540.0, "2. second"),
                ],
            },
            CaptureRecognitionRole::Question,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 2);
        assert_eq!(result.regions[0].rect.width, 1.0);
        assert_eq!(result.regions[0].rect.y, 82.0 / 1_200.0);
        assert_eq!(result.regions[0].rect.height, 436.0 / 1_200.0);
        assert_eq!(result.regions[1].rect.height, 88.0 / 1_200.0);
        assert_eq!(result.pairing_tokens, vec![Some(1), Some(2)]);
    }

    #[test]
    fn uses_left_then_right_reading_order_only_for_a_strong_two_column_gap() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_000,
                boxes: vec![
                    text_box(40.0, 100.0, 120.0, 140.0, "1. left"),
                    text_box(40.0, 500.0, 120.0, 540.0, "2. left"),
                    text_box(600.0, 100.0, 680.0, 140.0, "3. right"),
                    text_box(600.0, 500.0, 680.0, 540.0, "4. right"),
                ],
            },
            CaptureRecognitionRole::Answer,
        )
        .unwrap();

        assert_eq!(
            result.pairing_tokens,
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
        assert!(result.regions[..2].iter().all(|item| item.rect.x == 0.0));
        assert!(result.regions[2..].iter().all(|item| item.rect.x > 0.0));
        assert!(
            result
                .regions
                .iter()
                .all(|item| item.role == CaptureRecognitionRole::Answer)
        );
    }

    #[test]
    fn ignores_a_wide_header_when_question_anchors_form_two_clear_columns() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_000,
                boxes: vec![
                    text_box(40.0, 100.0, 120.0, 140.0, "1. left"),
                    text_box(40.0, 500.0, 120.0, 540.0, "2. left"),
                    text_box(600.0, 100.0, 680.0, 140.0, "3. right"),
                    text_box(600.0, 500.0, 680.0, 540.0, "4. right"),
                    text_box(100.0, 20.0, 900.0, 60.0, "2026 exam header"),
                ],
            },
            CaptureRecognitionRole::Question,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 4);
        assert!(result.regions[0].rect.x + result.regions[0].rect.width > result.regions[2].rect.x);
        assert_eq!(
            result.pairing_tokens,
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
    }

    #[test]
    fn ignores_a_vertical_page_margin_box_but_protects_crossing_question_content() {
        let page_with_margin = LocalOcrPage {
            width: 1_000,
            height: 1_000,
            boxes: vec![
                text_box(100.0, 200.0, 700.0, 240.0, "1. first"),
                text_box(100.0, 600.0, 700.0, 640.0, "2. second"),
                LocalOcrBox {
                    left: 10.0,
                    top: 100.0,
                    right: 70.0,
                    bottom: 900.0,
                    text: String::new(),
                    confidence: 0.90,
                    is_text: false,
                },
            ],
        };
        let accepted =
            analyze_ocr_page(page_with_margin.clone(), CaptureRecognitionRole::Question).unwrap();
        assert_eq!(accepted.regions.len(), 2);

        let mut page_with_crossing_content = page_with_margin;
        page_with_crossing_content.boxes[2].left = 150.0;
        page_with_crossing_content.boxes[2].right = 500.0;
        let fallback =
            analyze_ocr_page(page_with_crossing_content, CaptureRecognitionRole::Question).unwrap();
        assert_eq!(fallback.regions.len(), 1);
        assert_eq!(
            fallback.confidence_basis_points,
            FALLBACK_CONFIDENCE_BASIS_POINTS
        );
        assert_eq!(
            fallback.reason_codes,
            vec![CaptureRecognitionReasonCode::PossibleContentCut]
        );
    }

    #[test]
    fn ignores_a_watermark_overlay_that_crosses_multiple_numbered_rows() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_000,
                boxes: vec![
                    text_box(100.0, 150.0, 700.0, 190.0, "1. first"),
                    text_box(100.0, 350.0, 700.0, 390.0, "2. second"),
                    text_box(100.0, 550.0, 700.0, 590.0, "3. third"),
                    text_box(100.0, 750.0, 700.0, 790.0, "4. fourth"),
                    LocalOcrBox {
                        left: 300.0,
                        top: 250.0,
                        right: 800.0,
                        bottom: 850.0,
                        text: String::new(),
                        confidence: 0.0,
                        is_text: false,
                    },
                ],
            },
            CaptureRecognitionRole::Question,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 4);
        assert_eq!(
            result.pairing_tokens,
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
    }

    #[test]
    fn accepts_an_aligned_low_confidence_marker_only_as_part_of_a_strong_run() {
        let mut weak_fourth = text_box(100.0, 650.0, 130.0, 690.0, "4.");
        weak_fourth.confidence = 0.56;
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 900,
                boxes: vec![
                    text_box(100.0, 150.0, 700.0, 190.0, "1. first"),
                    text_box(100.0, 320.0, 700.0, 360.0, "2. second"),
                    text_box(100.0, 490.0, 700.0, 530.0, "3. third"),
                    weak_fourth,
                ],
            },
            CaptureRecognitionRole::Answer,
        )
        .unwrap();
        assert_eq!(result.regions.len(), 4);
        assert_eq!(
            result.pairing_tokens,
            vec![Some(1), Some(2), Some(3), Some(4)]
        );

        let mut isolated_weak = text_box(100.0, 150.0, 130.0, 190.0, "1.");
        isolated_weak.confidence = 0.56;
        let fallback = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 900,
                boxes: vec![
                    isolated_weak,
                    text_box(100.0, 320.0, 700.0, 360.0, "2. second"),
                ],
            },
            CaptureRecognitionRole::Answer,
        )
        .unwrap();
        assert_eq!(fallback.regions.len(), 1);
        assert_eq!(
            fallback.reason_codes,
            vec![CaptureRecognitionReasonCode::WeakAnchor]
        );
    }

    #[test]
    fn discards_numbered_instructions_and_recovers_aligned_bare_question_numbers() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 700,
                boxes: vec![
                    text_box(30.0, 40.0, 800.0, 70.0, "1. instructions"),
                    text_box(30.0, 80.0, 800.0, 110.0, "2. instructions"),
                    text_box(30.0, 120.0, 800.0, 150.0, "3. instructions"),
                    text_box(30.0, 240.0, 700.0, 280.0, "1 first question"),
                    text_box(30.0, 360.0, 45.0, 390.0, "2"),
                    text_box(30.0, 500.0, 700.0, 540.0, "3. third question"),
                    text_box(30.0, 650.0, 60.0, 675.0, "40"),
                ],
            },
            CaptureRecognitionRole::Question,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 3);
        assert_eq!(result.pairing_tokens, vec![Some(1), Some(2), Some(3)]);
        assert_eq!(result.regions[0].rect.y, 229.5 / 700.0);
    }

    #[test]
    fn formula_numbers_do_not_break_a_consecutive_answer_sequence() {
        let result = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 700,
                boxes: vec![
                    text_box(30.0, 80.0, 700.0, 120.0, "5. first answer"),
                    text_box(30.0, 240.0, 700.0, 280.0, "6. second answer"),
                    text_box(45.0, 360.0, 60.0, 385.0, "2"),
                    text_box(30.0, 500.0, 700.0, 540.0, "7. third answer"),
                    text_box(45.0, 610.0, 60.0, 635.0, "5"),
                ],
            },
            CaptureRecognitionRole::Answer,
        )
        .unwrap();

        assert_eq!(result.regions.len(), 3);
        assert_eq!(result.pairing_tokens, vec![Some(5), Some(6), Some(7)]);
        assert_eq!(result.regions[0].rect.y, 69.5 / 700.0);
        assert_eq!(result.regions[1].rect.y, 229.5 / 700.0);
        assert_eq!(result.regions[2].rect.y, 489.5 / 700.0);
    }

    #[test]
    fn rejects_invalid_runtime_geometry() {
        let error = analyze_ocr_page(
            LocalOcrPage {
                width: 1_000,
                height: 1_000,
                boxes: vec![text_box(40.0, 100.0, 1_100.0, 140.0, "1. invalid")],
            },
            CaptureRecognitionRole::Question,
        )
        .unwrap_err();

        assert!(matches!(error, RecognitionEngineError::InvalidResult));
    }
}
