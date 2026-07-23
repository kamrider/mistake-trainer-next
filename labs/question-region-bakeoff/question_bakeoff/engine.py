from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from .opencv_baseline import Analysis, analyze_image


Analyzer = Callable[[Path], Analysis]


def resolve_engine(name: str) -> Analyzer:
    engines: dict[str, Analyzer] = {
        "opencv-whitespace": analyze_image,
    }
    try:
        return engines[name]
    except KeyError as error:
        raise ValueError(f"unsupported question-region engine: {name}") from error
