from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from .opencv_baseline import Analysis, analyze_image


Analyzer = Callable[[Path], Analysis]


def resolve_engine(name: str) -> Analyzer:
    if name == "opencv-whitespace":
        return analyze_image
    if name == "rapidocr-anchor":
        from .rapidocr_engine import analyze_image as analyze_with_rapidocr

        return analyze_with_rapidocr
    raise ValueError(f"unsupported question-region engine: {name}")
