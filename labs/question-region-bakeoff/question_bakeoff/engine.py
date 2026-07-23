from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from .opencv_baseline import Analysis, analyze_image


Analyzer = Callable[[Path], Analysis]


def resolve_engine(name: str) -> Analyzer:
    if name == "opencv-whitespace":
        return analyze_image
    if name in {"rapidocr-anchor", "ppocrv6-small-anchor"}:
        from .rapidocr_engine import make_analyzer

        return make_analyzer("small")
    if name == "ppocrv6-medium-anchor":
        from .rapidocr_engine import make_analyzer

        return make_analyzer("medium")
    raise ValueError(f"unsupported question-region engine: {name}")
