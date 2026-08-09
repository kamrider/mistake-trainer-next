from __future__ import annotations

import argparse
import json
from pathlib import Path

from .rapidocr_engine import make_analyzer


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Compare PP-OCRv6 anchor tiers on an unlabelled local image folder. "
            "The report contains only basenames, counts, confidence, reasons, and timings."
        )
    )
    parser.add_argument("folder", type=Path)
    parser.add_argument(
        "--tiers",
        nargs="+",
        choices=("small", "medium"),
        default=("small", "medium"),
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    images = sorted(
        path
        for path in args.folder.iterdir()
        if path.is_file() and path.suffix.lower() in {".jpg", ".jpeg", ".png", ".webp"}
    )
    report: dict[str, object] = {"sampleCount": len(images), "tiers": {}}
    tiers: dict[str, list[dict[str, object]]] = {}
    for tier in args.tiers:
        analyze = make_analyzer(tier)
        results: list[dict[str, object]] = []
        for image in images:
            analysis = analyze(image)
            results.append(
                {
                    "file": image.name,
                    "regionCount": len(analysis.suggestions),
                    "minimumConfidence": min(
                        (suggestion.confidence for suggestion in analysis.suggestions),
                        default=0.0,
                    ),
                    "uncertainReasons": sorted(
                        {
                            suggestion.uncertain_reason
                            for suggestion in analysis.suggestions
                            if suggestion.uncertain_reason
                        }
                    ),
                    "runtimeMs": round(analysis.runtime_ms, 1),
                }
            )
        tiers[tier] = results
    report["tiers"] = tiers
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
