from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Sequence

from .schema import BenchmarkSample, Suggestion


MATCH_IOU_THRESHOLD = 0.5
ANCHOR_Y_TOLERANCE = 0.035
UNCERTAIN_CONFIDENCE = 0.75


@dataclass(frozen=True, slots=True)
class SampleMetrics:
    sample_id: str
    truth_count: int
    prediction_count: int
    matched_count: int
    matched_pairs: tuple[tuple[int, int], ...]
    region_recall: float
    mean_matched_iou: float
    content_cut_rate: float
    false_split_rate: float
    question_start_recall: float
    uncertain_count: int
    truth_area: float
    cut_area: float
    iou_sum: float
    false_split_count: int
    anchor_count: int
    anchor_hit_count: int


@dataclass(frozen=True, slots=True)
class AggregateMetrics:
    sample_count: int
    truth_count: int
    prediction_count: int
    matched_count: int
    anchor_count: int
    uncertain_count: int
    region_recall: float
    mean_matched_iou: float
    content_cut_rate: float
    false_split_rate: float
    question_start_recall: float
    passes_60_image_gate: bool
    passes_300_image_gate: bool


def _anchor_hits(sample: BenchmarkSample, suggestions: Sequence[Suggestion]) -> int:
    unused = {index for index, suggestion in enumerate(suggestions) if suggestion.anchor is not None}
    hits = 0
    for anchor in sample.anchors:
        candidates: list[tuple[float, int]] = []
        for index in unused:
            suggestion = suggestions[index]
            predicted = suggestion.anchor
            if predicted is None:
                continue
            same_column = suggestion.rect.x - 0.03 <= anchor.x <= suggestion.rect.right + 0.03
            distance = abs(predicted.y - anchor.y)
            if same_column and distance <= ANCHOR_Y_TOLERANCE:
                candidates.append((distance, index))
        if candidates:
            _, selected = min(candidates)
            unused.remove(selected)
            hits += 1
    return hits


def evaluate_sample(
    sample: BenchmarkSample,
    suggestions: Sequence[Suggestion],
) -> SampleMetrics:
    ranked_pairs: list[tuple[float, int, int]] = []
    for truth_index, truth in enumerate(sample.regions):
        for prediction_index, prediction in enumerate(suggestions):
            ranked_pairs.append((truth.iou(prediction.rect), truth_index, prediction_index))
    ranked_pairs.sort(key=lambda entry: (-entry[0], entry[1], entry[2]))

    used_truth: set[int] = set()
    used_predictions: set[int] = set()
    matches: list[tuple[int, int]] = []
    iou_sum = 0.0
    cut_area = sum(rect.area for rect in sample.regions)
    for iou, truth_index, prediction_index in ranked_pairs:
        if iou < MATCH_IOU_THRESHOLD:
            break
        if truth_index in used_truth or prediction_index in used_predictions:
            continue
        used_truth.add(truth_index)
        used_predictions.add(prediction_index)
        matches.append((truth_index, prediction_index))
        iou_sum += iou
        truth = sample.regions[truth_index]
        cut_area -= truth.intersection_area(suggestions[prediction_index].rect)

    matches.sort()
    truth_count = len(sample.regions)
    prediction_count = len(suggestions)
    matched_count = len(matches)
    false_split_count = prediction_count - matched_count
    truth_area = sum(rect.area for rect in sample.regions)
    anchor_hits = _anchor_hits(sample, suggestions)
    anchor_count = len(sample.anchors)
    return SampleMetrics(
        sample_id=sample.sample_id,
        truth_count=truth_count,
        prediction_count=prediction_count,
        matched_count=matched_count,
        matched_pairs=tuple(matches),
        region_recall=matched_count / truth_count if truth_count else 1.0,
        mean_matched_iou=iou_sum / matched_count if matched_count else 0.0,
        content_cut_rate=max(0.0, cut_area) / truth_area if truth_area else 0.0,
        false_split_rate=false_split_count / prediction_count if prediction_count else 0.0,
        question_start_recall=anchor_hits / anchor_count if anchor_count else 1.0,
        uncertain_count=sum(1 for suggestion in suggestions if suggestion.confidence < UNCERTAIN_CONFIDENCE),
        truth_area=truth_area,
        cut_area=max(0.0, cut_area),
        iou_sum=iou_sum,
        false_split_count=false_split_count,
        anchor_count=anchor_count,
        anchor_hit_count=anchor_hits,
    )

def aggregate_metrics(results: Iterable[SampleMetrics]) -> AggregateMetrics:
    values = tuple(results)
    sample_count = len(values)
    truth_count = sum(value.truth_count for value in values)
    prediction_count = sum(value.prediction_count for value in values)
    matched_count = sum(value.matched_count for value in values)
    anchor_count = sum(value.anchor_count for value in values)
    anchor_hits = sum(value.anchor_hit_count for value in values)
    truth_area = sum(value.truth_area for value in values)
    cut_area = sum(value.cut_area for value in values)
    false_splits = sum(value.false_split_count for value in values)
    iou_sum = sum(value.iou_sum for value in values)
    uncertain_count = sum(value.uncertain_count for value in values)
    region_recall = matched_count / truth_count if truth_count else 1.0
    mean_iou = iou_sum / matched_count if matched_count else 0.0
    content_cut_rate = cut_area / truth_area if truth_area else 0.0
    false_split_rate = false_splits / prediction_count if prediction_count else 0.0
    question_start_recall = anchor_hits / anchor_count if anchor_count else 1.0
    metric_gate = (
        anchor_count > 0
        and question_start_recall >= 0.95
        and content_cut_rate < 0.005
        and false_split_rate < 0.03
    )
    return AggregateMetrics(
        sample_count=sample_count,
        truth_count=truth_count,
        prediction_count=prediction_count,
        matched_count=matched_count,
        anchor_count=anchor_count,
        uncertain_count=uncertain_count,
        region_recall=region_recall,
        mean_matched_iou=mean_iou,
        content_cut_rate=content_cut_rate,
        false_split_rate=false_split_rate,
        question_start_recall=question_start_recall,
        passes_60_image_gate=sample_count >= 60 and metric_gate,
        passes_300_image_gate=sample_count >= 300 and metric_gate,
    )
