from __future__ import annotations

import re
import time
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Any, Callable, Iterable, Literal, Sequence

import numpy as np

from .opencv_baseline import (
    Analysis,
    _decode_path,
    detect_page_quad,
    estimate_skew_degrees,
)
from .schema import NormalizedPoint, NormalizedRect, Suggestion


ModelTier = Literal["small", "medium"]
ENGINE_NAME = "ppocrv6-small-anchor"
ENGINE_VERSION = (
    "rapidocr-3.9.2+ppocrv6-small+onnxruntime-1.27.0+anchor-1.1.0"
)
# RapidOCR reports the conservative minimum of detector and recognizer
# confidence. Readable low-resolution exam anchors commonly land around
# 0.62–0.70, so keep the lab aligned with the production Rust adapter.
MIN_ANCHOR_CONFIDENCE = 0.60
QUESTION_ANCHOR = re.compile(
    r"^\s*(?:"
    r"[（(]\d{1,3}[）)]"
    r"|\d{1,3}[.、．)]"
    r"|Q\d{1,3}[.:：．]"
    r"|第\s*\d{1,3}\s*题"
    r")\s*",
    re.IGNORECASE,
)
OPTION_ONLY = re.compile(r"^\s*[A-HＡ-Ｈ][.、．)]\s*", re.IGNORECASE)
RELAXED_QUESTION_ANCHOR = re.compile(r"^\s*(\d{1,3})(?:\s+|$)")
AMBIGUOUS_PUNCTUATED_ANCHOR = re.compile(r"^\s*(\d{1,3})[.、．)]")


@dataclass(frozen=True, slots=True)
class OcrBox:
    left: float
    top: float
    right: float
    bottom: float
    text: str
    confidence: float
    kind: str = "text"

    @property
    def center_x(self) -> float:
        return (self.left + self.right) / 2


def question_anchor_number(text: str, *, allow_relaxed: bool = False) -> int | None:
    match = QUESTION_ANCHOR.match(text)
    if match is None or OPTION_ONLY.match(text):
        if not allow_relaxed:
            return None
        relaxed = RELAXED_QUESTION_ANCHOR.match(text)
        return int(relaxed.group(1)) if relaxed else None
    # A decimal such as ``3.14`` starts with a syntactically valid ``3.``.
    # Treat a digit immediately after the matched marker as numeric content,
    # not as a question heading.
    if match.end() < len(text) and text[match.end()].isdigit():
        return None
    digits = re.search(r"\d{1,3}", match.group(0))
    return int(digits.group(0)) if digits else None


def is_question_anchor(text: str) -> bool:
    return question_anchor_number(text) is not None


def _validate_boxes(width: int, height: int, boxes: Sequence[OcrBox]) -> None:
    if width <= 0 or height <= 0:
        raise ValueError("source dimensions must be positive")
    for entry in boxes:
        values = (
            entry.left,
            entry.top,
            entry.right,
            entry.bottom,
            entry.confidence,
        )
        if not all(np.isfinite(value) for value in values):
            raise ValueError("OCR box values must be finite")
        if (
            entry.left < 0
            or entry.top < 0
            or entry.right > width
            or entry.bottom > height
            or entry.right <= entry.left
            or entry.bottom <= entry.top
            or not 0 <= entry.confidence <= 1
            or entry.kind not in {"text", "non_text"}
        ):
            raise ValueError("OCR box must remain inside the source image")


def _content_suggestion(
    width: int,
    height: int,
    boxes: Sequence[OcrBox],
    *,
    reason: str,
) -> tuple[Suggestion, ...]:
    return (
        Suggestion(
            # OCR may miss a diagram, formula, graph, or handwritten block.
            # An uncertain fallback therefore keeps the complete source rather
            # than pretending the detected text bounds contain all content.
            rect=NormalizedRect(0.0, 0.0, 1.0, 1.0),
            confidence=0.45,
            anchor=NormalizedPoint(0.0, 0.0),
            engine=ENGINE_NAME,
            engine_version=ENGINE_VERSION,
            uncertain_reason=reason,
        ),
    )


def _anchor_columns(
    width: int,
    anchors: Sequence[tuple[OcrBox, int]],
) -> tuple[tuple[tuple[OcrBox, int], ...], ...]:
    ordered = sorted(anchors, key=lambda entry: entry[0].left)
    if len(ordered) < 4:
        return (tuple(sorted(ordered, key=lambda entry: entry[0].top)),)
    gaps = [
        (ordered[index + 1][0].left - ordered[index][0].left, index)
        for index in range(len(ordered) - 1)
    ]
    largest_gap, split_index = max(gaps)
    left = ordered[: split_index + 1]
    right = ordered[split_index + 1 :]
    if largest_gap < width * 0.20 or len(left) < 2 or len(right) < 2:
        return (tuple(sorted(ordered, key=lambda entry: entry[0].top)),)
    return (
        tuple(sorted(left, key=lambda entry: entry[0].top)),
        tuple(sorted(right, key=lambda entry: entry[0].top)),
    )


def _select_question_run(
    anchors: Sequence[tuple[OcrBox, int]],
) -> tuple[tuple[OcrBox, int], ...]:
    ordered = sorted(anchors, key=lambda entry: entry[0].top)
    deduplicated: list[tuple[OcrBox, int]] = []
    for candidate in ordered:
        if deduplicated and abs(candidate[0].top - deduplicated[-1][0].top) <= 2:
            current = deduplicated[-1]
            if candidate[0].confidence > current[0].confidence:
                deduplicated[-1] = candidate
            continue
        deduplicated.append(candidate)

    # Formula fragments can look like aligned bare question numbers. Build the
    # longest consecutive subsequence while allowing those noisy candidates to
    # be skipped. When an instruction list and the real questions have the
    # same length, prefer the later sequence on the page.
    best_ending_at: dict[int, list[tuple[OcrBox, int]]] = {}
    runs: list[list[tuple[OcrBox, int]]] = []
    for candidate in deduplicated:
        previous = best_ending_at.get(candidate[1] - 1, [])
        run = [*previous, candidate]
        current = best_ending_at.get(candidate[1])
        if current is None or (len(run), run[0][0].top) >= (
            len(current),
            current[0][0].top,
        ):
            best_ending_at[candidate[1]] = run
        runs.append(run)
    if not runs:
        return ()
    selected = max(runs, key=lambda run: (len(run), run[0][0].top))
    return tuple(selected)


def _candidate_anchors(
    width: int,
    boxes: Sequence[OcrBox],
) -> tuple[tuple[OcrBox, int], ...]:
    strong = tuple(
        (entry, number)
        for entry in boxes
        if entry.kind == "text"
        and entry.confidence >= MIN_ANCHOR_CONFIDENCE
        and (number := question_anchor_number(entry.text)) is not None
    )
    strong_lefts = tuple(entry.left for entry, _ in strong)
    relaxed: list[tuple[OcrBox, int]] = []
    for entry in boxes:
        if entry.kind != "text" or entry.confidence < MIN_ANCHOR_CONFIDENCE:
            continue
        if question_anchor_number(entry.text) is not None:
            continue
        number = question_anchor_number(entry.text, allow_relaxed=True)
        if number is None and strong_lefts:
            # Answer explanations can begin with a number immediately after the
            # question marker, for example ``1.2, 8, 14...``. The strict parser
            # correctly treats that shape as a possible decimal. It becomes a
            # question anchor only when it aligns with other unambiguous
            # anchors; the later consecutive-run selection must still confirm
            # the sequence.
            ambiguous = AMBIGUOUS_PUNCTUATED_ANCHOR.match(entry.text)
            number = int(ambiguous.group(1)) if ambiguous else None
        if number is None:
            continue
        aligned = entry.left <= width * 0.15 or any(
            abs(entry.left - left) <= width * 0.035 for left in strong_lefts
        )
        if aligned:
            relaxed.append((entry, number))
    return strong + tuple(relaxed)


def _column_bounds(
    width: int,
    columns: Sequence[Sequence[tuple[OcrBox, int]]],
) -> tuple[tuple[float, float], ...]:
    if len(columns) == 1:
        return ((0.0, float(width)),)
    left_content_edge = max(entry.right for entry, _ in columns[0])
    right_content_edge = min(entry.left for entry, _ in columns[1])
    if left_content_edge < right_content_edge:
        split = (left_content_edge + right_content_edge) / 2
    else:
        split = (
            max(entry.left for entry, _ in columns[0])
            + min(entry.left for entry, _ in columns[1])
        ) / 2
    overlap = width * 0.025
    return (
        (0.0, min(float(width), split + overlap)),
        (max(0.0, split - overlap), float(width)),
    )


def _assemble_column(
    width: int,
    height: int,
    anchors: Sequence[tuple[OcrBox, int]],
    boxes: Sequence[OcrBox],
    *,
    left_bound: float,
    right_bound: float,
) -> tuple[Suggestion, ...]:
    if len(anchors) < 2:
        return _content_suggestion(
            width,
            height,
            boxes,
            reason="insufficient_question_anchors",
        )
    suggestions: list[Suggestion] = []
    overlap_y = height * 0.015
    for index, (anchor, _) in enumerate(anchors):
        next_top = (
            anchors[index + 1][0].top
            if index + 1 < len(anchors)
            else _last_question_bottom(
                boxes,
                anchor,
                left_bound=left_bound,
                right_bound=right_bound,
                height=height,
            )
        )
        # OCR text polygons can overlap a horizontal boundary by a few pixels
        # on skewed scans. Only a non-text block crossing the next anchor is
        # strong enough evidence that an automatic crop may cut a figure.
        if index + 1 < len(anchors) and any(
            entry.kind == "non_text"
            and entry.top < next_top < entry.bottom
            for entry in boxes
        ):
            return _content_suggestion(
                width,
                height,
                boxes,
                reason="overlapping_question_blocks",
            )
        # RapidOCR exposes text polygons, not a guaranteed box for every
        # figure. Use the entire detected column and the next question anchor
        # as a conservative lower boundary so invisible visual content cannot
        # be clipped by a tight text-only rectangle.
        top = max(0.0, anchor.top - overlap_y)
        bottom = min(float(height), next_top + overlap_y)
        confidence = min(0.96, max(0.75, anchor.confidence * 0.96))
        suggestions.append(
            Suggestion(
                rect=NormalizedRect(
                    left_bound / width,
                    top / height,
                    (right_bound - left_bound) / width,
                    (bottom - top) / height,
                ),
                confidence=confidence,
                anchor=NormalizedPoint(anchor.left / width, anchor.top / height),
                engine=ENGINE_NAME,
                engine_version=ENGINE_VERSION,
            )
        )
    return tuple(suggestions)


def _last_question_bottom(
    boxes: Sequence[OcrBox],
    anchor: OcrBox,
    *,
    left_bound: float,
    right_bound: float,
    height: int,
) -> float:
    candidates = tuple(
        entry
        for entry in boxes
        if left_bound <= entry.center_x <= right_bound
        and entry.bottom > anchor.top
        and entry.top >= anchor.top - height * 0.01
        and not _looks_like_page_footer(entry.text, entry.top / height)
    )
    group_bottom = max((entry.bottom for entry in candidates), default=anchor.bottom)
    # The regular assembly padding contributes another 1.5%, for a total
    # conservative tail margin of 2.5% on the final question.
    return min(float(height), group_bottom + height * 0.01)


def _looks_like_page_footer(text: str, normalized_top: float) -> bool:
    if normalized_top < 0.90:
        return False
    normalized = text.strip()
    return "页" in normalized or re.fullmatch(r"[-—–\s]*\d+[-—–\s]*", normalized) is not None


def suggest_from_ocr_boxes(
    width: int,
    height: int,
    boxes: Iterable[OcrBox],
) -> tuple[Suggestion, ...]:
    materialized = tuple(boxes)
    _validate_boxes(width, height, materialized)
    anchors = _candidate_anchors(width, materialized)
    if len(anchors) < 2:
        return _content_suggestion(
            width,
            height,
            materialized,
            reason="insufficient_question_anchors",
        )
    columns = tuple(_select_question_run(column) for column in _anchor_columns(width, anchors))
    if any(len(column) < 2 for column in columns):
        return _content_suggestion(
            width,
            height,
            materialized,
            reason="insufficient_question_anchors",
        )
    bounds = _column_bounds(width, columns)

    suggestions: list[Suggestion] = []
    for column_anchors, (left_bound, right_bound) in zip(
        columns,
        bounds,
        strict=True,
    ):
        column_suggestions = _assemble_column(
            width,
            height,
            column_anchors,
            materialized,
            left_bound=left_bound,
            right_bound=right_bound,
        )
        if any(item.uncertain_reason for item in column_suggestions):
            return column_suggestions
        suggestions.extend(column_suggestions)
    return tuple(suggestions)


def _polygon_box(polygon: Any, text: Any, confidence: Any) -> OcrBox:
    points = np.asarray(polygon, dtype=np.float64).reshape(-1, 2)
    if len(points) < 4:
        raise ValueError("RapidOCR returned an invalid polygon")
    return OcrBox(
        float(points[:, 0].min()),
        float(points[:, 1].min()),
        float(points[:, 0].max()),
        float(points[:, 1].max()),
        str(text),
        float(confidence),
    )


def _result_boxes(result: Any) -> tuple[OcrBox, ...]:
    if hasattr(result, "boxes") and hasattr(result, "txts") and hasattr(result, "scores"):
        return tuple(
            _polygon_box(polygon, text, confidence)
            for polygon, text, confidence in zip(
                result.boxes,
                result.txts,
                result.scores,
                strict=True,
            )
        )
    rows = result[0] if isinstance(result, tuple) and len(result) == 2 else result
    if rows is None:
        return ()
    try:
        return tuple(_polygon_box(row[0], row[1], row[2]) for row in rows)
    except (IndexError, TypeError, ValueError) as error:
        raise ValueError("RapidOCR returned an unsupported result shape") from error


def engine_name_for(model_type: ModelTier) -> str:
    if model_type not in {"small", "medium"}:
        raise ValueError(f"unsupported PP-OCRv6 model tier: {model_type}")
    return f"ppocrv6-{model_type}-anchor"


def _engine_version_for(model_type: ModelTier) -> str:
    return (
        f"rapidocr-3.9.2+ppocrv6-{model_type}"
        "+onnxruntime-1.27.0+anchor-1.1.0"
    )


def _analyze_with_runtime(
    path: str | Path,
    *,
    runtime: Any,
    engine_name: str,
    engine_version: str,
    started: float,
) -> Analysis:
    image = _decode_path(Path(path))
    height, width = image.shape[:2]
    result = runtime(image)
    boxes = _result_boxes(result)
    suggestions = tuple(
        replace(
            item,
            engine=engine_name,
            engine_version=engine_version,
        )
        for item in suggest_from_ocr_boxes(width, height, boxes)
    )
    quad = detect_page_quad(image)
    normalized_quad: tuple[NormalizedPoint, ...] = ()
    if quad is not None:
        normalized_quad = tuple(
            NormalizedPoint(float(x) / width, float(y) / height) for x, y in quad
        )
    return Analysis(
        engine=engine_name,
        engine_version=engine_version,
        suggestions=suggestions,
        runtime_ms=(time.perf_counter() - started) * 1000,
        original_width=width,
        original_height=height,
        page_quad=normalized_quad,
        skew_degrees=estimate_skew_degrees(image),
    )


def make_analyzer(
    model_type: ModelTier,
    *,
    runtime_factory: Callable[..., Any] | None = None,
) -> Callable[[str | Path], Analysis]:
    engine_name = engine_name_for(model_type)
    engine_version = _engine_version_for(model_type)
    runtime: Any | None = None

    def analyze(path: str | Path) -> Analysis:
        nonlocal runtime
        started = time.perf_counter()
        if runtime is None:
            factory = runtime_factory
            try:
                from rapidocr import ModelType, OCRVersion, RapidOCR
            except ImportError as error:
                raise ValueError(
                    "RapidOCR runtime is not installed in the isolated lab"
                ) from error
            if factory is None:
                factory = RapidOCR
            model_tier = (
                ModelType.SMALL if model_type == "small" else ModelType.MEDIUM
            )
            runtime = factory(
                params={
                    "Det.ocr_version": OCRVersion.PPOCRV6,
                    "Det.model_type": model_tier,
                    "Rec.ocr_version": OCRVersion.PPOCRV6,
                    "Rec.model_type": model_tier,
                }
            )
        return _analyze_with_runtime(
            path,
            runtime=runtime,
            engine_name=engine_name,
            engine_version=engine_version,
            started=started,
        )

    analyze.model_type = model_type  # type: ignore[attr-defined]
    analyze.engine_name = engine_name  # type: ignore[attr-defined]
    return analyze


analyze_image = make_analyzer("small")
