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
MIN_ANCHOR_CONFIDENCE = 0.75
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


def is_question_anchor(text: str) -> bool:
    match = QUESTION_ANCHOR.match(text)
    if match is None or OPTION_ONLY.match(text):
        return False
    # A decimal such as ``3.14`` starts with a syntactically valid ``3.``.
    # Treat a digit immediately after the matched marker as numeric content,
    # not as a question heading.
    return match.end() >= len(text) or not text[match.end()].isdigit()


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
    anchors: Sequence[OcrBox],
) -> tuple[tuple[OcrBox, ...], ...]:
    ordered = sorted(anchors, key=lambda entry: entry.center_x)
    if len(ordered) < 4:
        return (tuple(sorted(ordered, key=lambda entry: entry.top)),)
    gaps = [
        (ordered[index + 1].center_x - ordered[index].center_x, index)
        for index in range(len(ordered) - 1)
    ]
    largest_gap, split_index = max(gaps)
    left = ordered[: split_index + 1]
    right = ordered[split_index + 1 :]
    if largest_gap < width * 0.20 or len(left) < 2 or len(right) < 2:
        return (tuple(sorted(ordered, key=lambda entry: entry.top)),)
    return (
        tuple(sorted(left, key=lambda entry: entry.top)),
        tuple(sorted(right, key=lambda entry: entry.top)),
    )


def _boxes_for_columns(
    boxes: Sequence[OcrBox],
    columns: Sequence[Sequence[OcrBox]],
) -> tuple[tuple[OcrBox, ...], ...] | None:
    if len(columns) == 1:
        return (tuple(boxes),)
    left_edge = max(anchor.center_x for anchor in columns[0])
    right_edge = min(anchor.center_x for anchor in columns[1])
    split = (left_edge + right_edge) / 2
    assigned: list[list[OcrBox]] = [[], []]
    for entry in boxes:
        if entry.left < split < entry.right:
            return None
        assigned[0 if entry.center_x < split else 1].append(entry)
    return tuple(tuple(entries) for entries in assigned)


def _assemble_column(
    width: int,
    height: int,
    anchors: Sequence[OcrBox],
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
    for index, anchor in enumerate(anchors):
        next_top = anchors[index + 1].top if index + 1 < len(anchors) else float(height)
        content = [
            entry
            for entry in boxes
            if entry.bottom > anchor.top and entry.top < next_top
        ]
        if index + 1 < len(anchors) and any(
            entry.bottom > next_top for entry in content
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
        top = anchor.top
        bottom = next_top
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


def suggest_from_ocr_boxes(
    width: int,
    height: int,
    boxes: Iterable[OcrBox],
) -> tuple[Suggestion, ...]:
    materialized = tuple(boxes)
    _validate_boxes(width, height, materialized)
    anchors = tuple(
        entry
        for entry in materialized
        if entry.kind == "text"
        and entry.confidence >= MIN_ANCHOR_CONFIDENCE
        and is_question_anchor(entry.text)
    )
    if len(anchors) < 2:
        return _content_suggestion(
            width,
            height,
            materialized,
            reason="insufficient_question_anchors",
        )
    columns = _anchor_columns(width, anchors)
    assigned = _boxes_for_columns(materialized, columns)
    if assigned is None:
        return _content_suggestion(
            width,
            height,
            materialized,
            reason="cross_column_content",
        )
    if len(columns) == 1:
        bounds = ((0.0, float(width)),)
    else:
        left_edge = max(anchor.center_x for anchor in columns[0])
        right_edge = min(anchor.center_x for anchor in columns[1])
        split = (left_edge + right_edge) / 2
        bounds = ((0.0, split), (split, float(width)))

    suggestions: list[Suggestion] = []
    for column_anchors, column_boxes, (left_bound, right_bound) in zip(
        columns,
        assigned,
        bounds,
        strict=True,
    ):
        column_suggestions = _assemble_column(
            width,
            height,
            column_anchors,
            column_boxes,
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
            if factory is None:
                try:
                    from rapidocr import RapidOCR
                except ImportError as error:
                    raise ValueError(
                        "RapidOCR runtime is not installed in the isolated lab"
                    ) from error
                factory = RapidOCR
            runtime = factory(
                params={
                    "Det.ocr_version": "PP-OCRv6",
                    "Det.model_type": model_type,
                    "Rec.ocr_version": "PP-OCRv6",
                    "Rec.model_type": model_type,
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
