from __future__ import annotations

import io
import math
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import cv2
import numpy as np
from PIL import Image, ImageOps

from .schema import NormalizedPoint, NormalizedRect, Suggestion


ENGINE_NAME = "opencv-whitespace"
ENGINE_VERSION = "1.0.0"
MAX_INPUT_BYTES = 50 * 1024 * 1024
MAX_INPUT_PIXELS = 80_000_000


@dataclass(frozen=True, slots=True)
class Analysis:
    engine: str
    engine_version: str
    suggestions: tuple[Suggestion, ...]
    runtime_ms: float
    original_width: int
    original_height: int
    page_quad: tuple[NormalizedPoint, ...]
    skew_degrees: float


def _require_image(image: np.ndarray) -> None:
    if not isinstance(image, np.ndarray) or image.ndim not in (2, 3):
        raise ValueError("image must be a grayscale or BGR NumPy array")
    if image.shape[0] < 2 or image.shape[1] < 2:
        raise ValueError("image dimensions are too small")
    if image.shape[0] * image.shape[1] > MAX_INPUT_PIXELS:
        raise ValueError("image exceeds the 80-million-pixel lab limit")


def _gray(image: np.ndarray) -> np.ndarray:
    _require_image(image)
    if image.ndim == 2:
        return image
    if image.shape[2] == 4:
        return cv2.cvtColor(image, cv2.COLOR_BGRA2GRAY)
    return cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)


def _order_quad(points: np.ndarray) -> np.ndarray:
    points = np.asarray(points, dtype=np.float32).reshape(4, 2)
    sums = points.sum(axis=1)
    differences = np.diff(points, axis=1).reshape(-1)
    ordered = np.empty((4, 2), dtype=np.float32)
    ordered[0] = points[np.argmin(sums)]
    ordered[2] = points[np.argmax(sums)]
    ordered[1] = points[np.argmin(differences)]
    ordered[3] = points[np.argmax(differences)]
    return ordered


def detect_page_quad(image: np.ndarray) -> np.ndarray | None:
    gray = _gray(image)
    height, width = gray.shape
    blurred = cv2.GaussianBlur(gray, (5, 5), 0)
    edges = cv2.Canny(blurred, 45, 140)
    edges = cv2.morphologyEx(edges, cv2.MORPH_CLOSE, np.ones((5, 5), np.uint8), iterations=2)
    contours, _ = cv2.findContours(edges, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
    minimum_area = height * width * 0.22
    candidates: list[tuple[float, np.ndarray]] = []
    for contour in contours:
        area = abs(cv2.contourArea(contour))
        if area < minimum_area:
            continue
        perimeter = cv2.arcLength(contour, True)
        approximation = cv2.approxPolyDP(contour, 0.02 * perimeter, True)
        if len(approximation) != 4 or not cv2.isContourConvex(approximation):
            continue
        candidates.append((area, _order_quad(approximation[:, 0, :])))
    if not candidates:
        return None
    return max(candidates, key=lambda entry: entry[0])[1]


def warp_page(image: np.ndarray, quad: np.ndarray) -> np.ndarray:
    _require_image(image)
    ordered = _order_quad(quad)
    top_left, top_right, bottom_right, bottom_left = ordered
    width = int(
        round(
            max(
                np.linalg.norm(top_right - top_left),
                np.linalg.norm(bottom_right - bottom_left),
            )
        )
    )
    height = int(
        round(
            max(
                np.linalg.norm(bottom_left - top_left),
                np.linalg.norm(bottom_right - top_right),
            )
        )
    )
    if width < 2 or height < 2 or width * height > MAX_INPUT_PIXELS:
        raise ValueError("page quadrilateral produces invalid output dimensions")
    destination = np.array(
        [[0, 0], [width - 1, 0], [width - 1, height - 1], [0, height - 1]],
        dtype=np.float32,
    )
    transform = cv2.getPerspectiveTransform(ordered, destination)
    return cv2.warpPerspective(image, transform, (width, height), borderValue=(255, 255, 255))


def threshold_foreground(image: np.ndarray) -> np.ndarray:
    gray = _gray(image)
    _, binary = cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY_INV | cv2.THRESH_OTSU)
    # A one-pixel opening removes phone-camera grain without erasing thin formula strokes.
    return cv2.morphologyEx(binary, cv2.MORPH_OPEN, np.ones((2, 2), np.uint8))


def estimate_skew_degrees(image: np.ndarray) -> float:
    binary = threshold_foreground(image)
    height, width = binary.shape
    lines = cv2.HoughLinesP(
        binary,
        1,
        np.pi / 1800,
        threshold=max(30, width // 12),
        minLineLength=max(40, width // 5),
        maxLineGap=max(8, width // 50),
    )
    angles: list[float] = []
    if lines is not None:
        for x1, y1, x2, y2 in np.asarray(lines).reshape(-1, 4):
            angle = math.degrees(math.atan2(float(y2 - y1), float(x2 - x1)))
            while angle <= -90:
                angle += 180
            while angle > 90:
                angle -= 180
            if abs(angle) <= 15:
                angles.append(angle)
    if not angles:
        return 0.0
    value = float(np.median(np.asarray(angles, dtype=np.float64)))
    return max(-15.0, min(15.0, value))


def _runs(mask: np.ndarray) -> list[tuple[int, int]]:
    result: list[tuple[int, int]] = []
    start: int | None = None
    for index, active in enumerate(mask.tolist()):
        if active and start is None:
            start = index
        elif not active and start is not None:
            result.append((start, index))
            start = None
    if start is not None:
        result.append((start, len(mask)))
    return result


def detect_columns(binary: np.ndarray) -> tuple[tuple[int, int], ...]:
    if binary.ndim != 2:
        raise ValueError("column detection requires a binary grayscale image")
    height, width = binary.shape
    foreground = binary > 0
    coordinates = np.argwhere(foreground)
    if coordinates.size == 0:
        return ((0, width),)
    x_min = int(coordinates[:, 1].min())
    x_max = int(coordinates[:, 1].max()) + 1
    density = np.count_nonzero(foreground, axis=0) / max(1, height)
    central_start = int(width * 0.28)
    central_end = int(width * 0.72)
    positive_density = density[density > 0]
    low_threshold = max(
        0.001,
        min(0.01, float(np.percentile(positive_density, 20)) * 0.08)
        if positive_density.size
        else 0.001,
    )
    low_ink = density[central_start:central_end] <= low_threshold
    gaps = [(start + central_start, end + central_start) for start, end in _runs(low_ink)]
    gaps = [gap for gap in gaps if gap[1] - gap[0] >= max(3, round(width * 0.03))]
    if not gaps:
        padding = max(2, round(width * 0.01))
        return ((max(0, x_min - padding), min(width, x_max + padding)),)
    gap_start, gap_end = max(gaps, key=lambda gap: gap[1] - gap[0])
    left_ink = int(np.count_nonzero(foreground[:, :gap_start]))
    right_ink = int(np.count_nonzero(foreground[:, gap_end:]))
    total_ink = left_ink + right_ink
    if total_ink == 0 or left_ink / total_ink < 0.15 or right_ink / total_ink < 0.15:
        padding = max(2, round(width * 0.01))
        return ((max(0, x_min - padding), min(width, x_max + padding)),)

    left_coordinates = np.argwhere(foreground[:, :gap_start])
    right_coordinates = np.argwhere(foreground[:, gap_end:])
    padding = max(2, round(width * 0.01))
    left_min = int(left_coordinates[:, 1].min())
    left_max = int(left_coordinates[:, 1].max()) + 1
    right_min = int(right_coordinates[:, 1].min()) + gap_end
    right_max = int(right_coordinates[:, 1].max()) + 1 + gap_end
    return (
        (max(0, left_min - padding), min(gap_start, left_max + padding)),
        (max(gap_end, right_min - padding), min(width, right_max + padding)),
    )


def _merge_runs(runs: Iterable[tuple[int, int]], maximum_gap: int) -> list[tuple[int, int]]:
    merged: list[tuple[int, int]] = []
    for start, end in runs:
        if merged and start - merged[-1][1] <= maximum_gap:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged


def _expand_for_components(
    binary: np.ndarray,
    x_start: int,
    x_end: int,
    y_start: int,
    y_end: int,
) -> tuple[int, int]:
    column = binary[:, x_start:x_end]
    count, _, stats, _ = cv2.connectedComponentsWithStats(column, connectivity=8)
    expanded_start = y_start
    expanded_end = y_end
    for index in range(1, count):
        _, component_y, _, component_height, component_area = stats[index]
        if component_area < 4:
            continue
        component_end = int(component_y + component_height)
        if component_y < expanded_end and component_end > expanded_start:
            expanded_start = min(expanded_start, int(component_y))
            expanded_end = max(expanded_end, component_end)
    return expanded_start, expanded_end


def suggest_question_regions(image: np.ndarray) -> tuple[Suggestion, ...]:
    binary = threshold_foreground(image)
    height, width = binary.shape
    foreground_count = int(np.count_nonzero(binary))
    if foreground_count / (height * width) < 0.0005:
        return (
            Suggestion(
                rect=NormalizedRect(0.0, 0.0, 1.0, 1.0),
                confidence=0.1,
                anchor=NormalizedPoint(0.02, 0.02),
                engine=ENGINE_NAME,
                engine_version=ENGINE_VERSION,
                uncertain_reason="insufficient_foreground",
            ),
        )

    columns = detect_columns(binary)
    suggestions: list[Suggestion] = []
    for x_start, x_end in columns:
        column = binary[:, x_start:x_end]
        row_density = np.count_nonzero(column, axis=1) / max(1, x_end - x_start)
        active = row_density >= max(0.004, float(np.percentile(row_density[row_density > 0], 20)) * 0.15)
        runs = _merge_runs(_runs(active), max(3, round(height * 0.02)))
        runs = [(start, end) for start, end in runs if end - start >= 3]
        if not runs:
            continue
        strong_split = len(runs) > 1
        pad_y = max(2, round(height * 0.01))
        for run_index, (start, end) in enumerate(runs):
            start, end = _expand_for_components(binary, x_start, x_end, start, end)
            top = max(0, start - pad_y)
            bottom = min(height, end + pad_y)
            if bottom <= top:
                continue
            previous_gap = start - runs[run_index - 1][1] if run_index else start
            next_gap = runs[run_index + 1][0] - end if run_index + 1 < len(runs) else height - end
            gap_evidence = min(0.18, max(previous_gap, next_gap) / max(1, height))
            confidence = min(0.96, 0.78 + gap_evidence) if strong_split else 0.62
            reason = None if confidence >= 0.75 else "single_region_no_split_evidence"
            rect = NormalizedRect(
                x_start / width,
                top / height,
                (x_end - x_start) / width,
                (bottom - top) / height,
            )
            suggestions.append(
                Suggestion(
                    rect=rect,
                    confidence=confidence,
                    anchor=NormalizedPoint(rect.x, rect.y),
                    engine=ENGINE_NAME,
                    engine_version=ENGINE_VERSION,
                    uncertain_reason=reason,
                )
            )

    if not suggestions:
        return (
            Suggestion(
                rect=NormalizedRect(0.0, 0.0, 1.0, 1.0),
                confidence=0.2,
                anchor=NormalizedPoint(0.02, 0.02),
                engine=ENGINE_NAME,
                engine_version=ENGINE_VERSION,
                uncertain_reason="no_stable_region",
            ),
        )
    return tuple(sorted(suggestions, key=lambda entry: (entry.rect.x, entry.rect.y)))


def _decode_path(path: Path) -> np.ndarray:
    try:
        payload = path.read_bytes()
    except OSError as error:
        raise ValueError("benchmark image could not be read") from error
    if not payload or len(payload) > MAX_INPUT_BYTES:
        raise ValueError("benchmark image is empty or exceeds 50 MB")
    try:
        with Image.open(io.BytesIO(payload)) as source:
            corrected = ImageOps.exif_transpose(source)
            rgb = np.asarray(corrected.convert("RGB"))
            image = cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
    except (OSError, ValueError) as error:
        raise ValueError("benchmark image could not be decoded") from error
    _require_image(image)
    return image


def analyze_image(path: str | Path) -> Analysis:
    started = time.perf_counter()
    image = _decode_path(Path(path))
    height, width = image.shape[:2]
    quad = detect_page_quad(image)
    normalized_quad: tuple[NormalizedPoint, ...] = ()
    if quad is not None:
        normalized_quad = tuple(NormalizedPoint(float(x) / width, float(y) / height) for x, y in quad)
    suggestions = suggest_question_regions(image)
    elapsed = (time.perf_counter() - started) * 1000
    return Analysis(
        engine=ENGINE_NAME,
        engine_version=ENGINE_VERSION,
        suggestions=suggestions,
        runtime_ms=elapsed,
        original_width=width,
        original_height=height,
        page_quad=normalized_quad,
        skew_degrees=estimate_skew_degrees(image),
    )
