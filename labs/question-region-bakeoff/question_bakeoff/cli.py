from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .report import ReportError, write_benchmark_report
from .opencv_baseline import ENGINE_NAME, ENGINE_VERSION
from .schema import ManifestError, load_manifest


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="question-bakeoff",
        description="Offline, consent-first question-region benchmark lab.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("self-check", help="print the isolated lab runtime versions")
    validate = commands.add_parser("validate", help="validate consent, paths, and labels")
    validate.add_argument("manifest", type=Path)
    run = commands.add_parser("run", help="run OpenCV baseline and write an auditable report")
    run.add_argument("manifest", type=Path)
    run.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "self-check":
            import cv2
            import numpy
            import PIL

            print(
                json.dumps(
                    {
                        "python": sys.version.split()[0],
                        "numpy": numpy.__version__,
                        "opencv": cv2.__version__,
                        "pillow": PIL.__version__,
                        "engine": ENGINE_NAME,
                        "engineVersion": ENGINE_VERSION,
                    },
                    ensure_ascii=False,
                )
            )
            return 0
        manifest_path = arguments.manifest.resolve()
        manifest = load_manifest(manifest_path)
        if arguments.command == "validate":
            print(f"validated {len(manifest.samples)} consented sample(s)")
            return 0
        report = write_benchmark_report(manifest_path, manifest, arguments.output)
        aggregate = report["aggregate"]
        print(
            f"wrote {report['sampleCount']} sample(s); "
            f"content cut {aggregate['contentCutRate']:.4%}; "
            f"false split {aggregate['falseSplitRate']:.4%}"
        )
        return 0
    except (ManifestError, ReportError, ValueError, OSError) as error:
        print(f"question-bakeoff: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
