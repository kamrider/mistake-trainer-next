from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any, Mapping, Sequence


ENGINE_CONTRACT_VERSION = "1"
_TOLERANCE = 1e-9
_SAMPLE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")


class ManifestError(ValueError):
    """Raised when a benchmark manifest could expose data or corrupt metrics."""


def _finite_number(value: Any, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{field} must be a finite number")
    converted = float(value)
    if not math.isfinite(converted):
        raise ValueError(f"{field} must be a finite number")
    return converted


@dataclass(frozen=True, slots=True)
class NormalizedRect:
    x: float
    y: float
    width: float
    height: float

    def __post_init__(self) -> None:
        values = {
            "x": _finite_number(self.x, "x"),
            "y": _finite_number(self.y, "y"),
            "width": _finite_number(self.width, "width"),
            "height": _finite_number(self.height, "height"),
        }
        for key, value in values.items():
            object.__setattr__(self, key, value)
        if self.x < -_TOLERANCE or self.y < -_TOLERANCE:
            raise ValueError("rectangle origin must be inside normalized bounds")
        if self.width <= 0 or self.height <= 0:
            raise ValueError("rectangle must have positive area")
        if self.right > 1 + _TOLERANCE or self.bottom > 1 + _TOLERANCE:
            raise ValueError("rectangle must remain inside normalized bounds")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "NormalizedRect":
        try:
            return cls(value["x"], value["y"], value["width"], value["height"])
        except KeyError as error:
            raise ValueError(f"rectangle is missing {error.args[0]}") from error

    @property
    def right(self) -> float:
        return self.x + self.width

    @property
    def bottom(self) -> float:
        return self.y + self.height

    @property
    def area(self) -> float:
        return self.width * self.height

    def intersection_area(self, other: "NormalizedRect") -> float:
        width = max(0.0, min(self.right, other.right) - max(self.x, other.x))
        height = max(0.0, min(self.bottom, other.bottom) - max(self.y, other.y))
        return width * height

    def iou(self, other: "NormalizedRect") -> float:
        intersection = self.intersection_area(other)
        union = self.area + other.area - intersection
        return intersection / union if union > 0 else 0.0

    def as_dict(self) -> dict[str, float]:
        return {"x": self.x, "y": self.y, "width": self.width, "height": self.height}


@dataclass(frozen=True, slots=True)
class NormalizedPoint:
    x: float
    y: float

    def __post_init__(self) -> None:
        x = _finite_number(self.x, "x")
        y = _finite_number(self.y, "y")
        object.__setattr__(self, "x", x)
        object.__setattr__(self, "y", y)
        if x < -_TOLERANCE or y < -_TOLERANCE or x > 1 + _TOLERANCE or y > 1 + _TOLERANCE:
            raise ValueError("point must remain inside normalized bounds")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "NormalizedPoint":
        try:
            return cls(value["x"], value["y"])
        except KeyError as error:
            raise ValueError(f"point is missing {error.args[0]}") from error

    def as_dict(self) -> dict[str, float]:
        return {"x": self.x, "y": self.y}


@dataclass(frozen=True, slots=True)
class BenchmarkConsent:
    anonymized: bool
    authorized_for_local_evaluation: bool
    recorded_at: str


@dataclass(frozen=True, slots=True)
class BenchmarkSample:
    sample_id: str
    image_path: Path
    layout: str
    tags: tuple[str, ...]
    regions: tuple[NormalizedRect, ...]
    anchors: tuple[NormalizedPoint, ...]


@dataclass(frozen=True, slots=True)
class BenchmarkManifest:
    schema_version: int
    root: Path
    consent: BenchmarkConsent
    samples: tuple[BenchmarkSample, ...]


@dataclass(frozen=True, slots=True)
class Suggestion:
    rect: NormalizedRect
    confidence: float
    anchor: NormalizedPoint | None
    engine: str
    engine_version: str
    uncertain_reason: str | None = None

    def __post_init__(self) -> None:
        confidence = _finite_number(self.confidence, "confidence")
        object.__setattr__(self, "confidence", confidence)
        if confidence < 0 or confidence > 1:
            raise ValueError("confidence must be between zero and one")
        if not self.engine.strip() or not self.engine_version.strip():
            raise ValueError("engine metadata must be non-empty")
        if confidence < 0.75 and not (self.uncertain_reason or "").strip():
            object.__setattr__(self, "uncertain_reason", "confidence_below_review_threshold")


def _mapping(value: Any, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ManifestError(f"{field} must be an object")
    return value


def _sequence(value: Any, field: str) -> Sequence[Any]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise ManifestError(f"{field} must be an array")
    return value


def _parse_consent(value: Any) -> BenchmarkConsent:
    consent = _mapping(value, "consent")
    anonymized = consent.get("anonymized") is True
    authorized = consent.get("authorizedForLocalEvaluation") is True
    recorded_at = consent.get("recordedAt")
    if not anonymized or not authorized:
        raise ManifestError("consent must affirm anonymization and local evaluation authorization")
    if not isinstance(recorded_at, str) or not recorded_at.strip():
        raise ManifestError("consent recordedAt must be a non-empty ISO date")
    try:
        date.fromisoformat(recorded_at)
    except ValueError as error:
        raise ManifestError("consent recordedAt must be a valid ISO date") from error
    return BenchmarkConsent(anonymized, authorized, recorded_at)


def _parse_text_list(value: Any, field: str) -> tuple[str, ...]:
    result: list[str] = []
    for entry in _sequence(value, field):
        if not isinstance(entry, str) or not entry.strip():
            raise ManifestError(f"{field} entries must be non-empty strings")
        normalized = entry.strip()
        if normalized not in result:
            result.append(normalized)
    return tuple(result)


def _safe_image_path(root: Path, raw: Any) -> Path:
    if not isinstance(raw, str) or not raw.strip():
        raise ManifestError("sample image must be a non-empty relative path")
    candidate = Path(raw)
    if candidate.is_absolute():
        raise ManifestError("sample image must remain inside the fixture directory")
    resolved = (root / candidate).resolve()
    if not resolved.is_relative_to(root):
        raise ManifestError("sample image must remain inside the fixture directory")
    if not resolved.is_file():
        raise ManifestError(f"sample image is missing: {candidate.as_posix()}")
    return resolved


def load_manifest(path: str | Path) -> BenchmarkManifest:
    manifest_path = Path(path).resolve()
    try:
        raw = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ManifestError("manifest must be readable UTF-8 JSON") from error
    data = _mapping(raw, "manifest")
    if data.get("schemaVersion") != 1:
        raise ManifestError("manifest schemaVersion must be 1")
    consent = _parse_consent(data.get("consent"))
    sample_values = _sequence(data.get("samples"), "samples")
    if not sample_values:
        raise ManifestError("manifest must contain at least one sample")

    root = manifest_path.parent.resolve()
    seen_ids: set[str] = set()
    samples: list[BenchmarkSample] = []
    for index, value in enumerate(sample_values):
        sample = _mapping(value, f"samples[{index}]")
        sample_id = sample.get("id")
        if not isinstance(sample_id, str) or not _SAMPLE_ID.fullmatch(sample_id):
            raise ManifestError(f"samples[{index}].id is not a safe stable identifier")
        if sample_id in seen_ids:
            raise ManifestError(f"duplicate sample id: {sample_id}")
        seen_ids.add(sample_id)
        layout = sample.get("layout", "unknown")
        if not isinstance(layout, str) or not layout.strip():
            raise ManifestError(f"samples[{index}].layout must be a non-empty string")
        try:
            regions = tuple(
                NormalizedRect.from_mapping(_mapping(entry, "region"))
                for entry in _sequence(sample.get("regions"), f"samples[{index}].regions")
            )
            anchors = tuple(
                NormalizedPoint.from_mapping(_mapping(entry, "anchor"))
                for entry in _sequence(sample.get("anchors", []), f"samples[{index}].anchors")
            )
        except ValueError as error:
            raise ManifestError(f"samples[{index}] contains invalid normalized geometry: {error}") from error
        if not regions:
            raise ManifestError(f"samples[{index}] must contain at least one ground-truth region")
        samples.append(
            BenchmarkSample(
                sample_id=sample_id,
                image_path=_safe_image_path(root, sample.get("image")),
                layout=layout.strip(),
                tags=_parse_text_list(sample.get("tags", []), f"samples[{index}].tags"),
                regions=regions,
                anchors=anchors,
            )
        )

    return BenchmarkManifest(1, root, consent, tuple(samples))
