from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .annotator import AnnotationError, serve_annotator
from .schema import ManifestError


def resolve_engine(name: str):
    from .engine import resolve_engine as resolve

    return resolve(name)


def write_benchmark_report(*args, **kwargs):
    from .report import write_benchmark_report as write

    return write(*args, **kwargs)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="question-bakeoff",
        description="Offline, consent-first question-region benchmark lab.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("self-check", help="print the isolated lab runtime versions")
    annotate = commands.add_parser(
        "annotate",
        help="open the localhost-only visual ground-truth annotator",
    )
    annotate.add_argument("data", type=Path)
    annotate.add_argument(
        "--no-open",
        action="store_true",
        help="print the local URL without opening the default browser",
    )
    validate = commands.add_parser("validate", help="validate consent, paths, and labels")
    validate.add_argument("manifest", type=Path)
    run = commands.add_parser("run", help="run a named engine and write an auditable report")
    run.add_argument("manifest", type=Path)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--engine", default="opencv-whitespace")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "self-check":
            import cv2
            import numpy
            import onnxruntime
            import PIL
            from importlib.metadata import version

            from .opencv_baseline import ENGINE_NAME, ENGINE_VERSION

            print(
                json.dumps(
                    {
                        "python": sys.version.split()[0],
                        "numpy": numpy.__version__,
                        "opencv": cv2.__version__,
                        "pillow": PIL.__version__,
                        "rapidocr": version("rapidocr"),
                        "onnxruntime": onnxruntime.__version__,
                        "engine": ENGINE_NAME,
                        "engineVersion": ENGINE_VERSION,
                        "availableEngines": [
                            "opencv-whitespace",
                            "ppocrv6-small-anchor",
                            "ppocrv6-medium-anchor",
                        ],
                    },
                    ensure_ascii=False,
                )
            )
            return 0
        if arguments.command == "annotate":
            serve_annotator(arguments.data, open_browser=not arguments.no_open)
            return 0
        from .schema import load_manifest

        manifest_path = arguments.manifest.resolve()
        manifest = load_manifest(manifest_path)
        if arguments.command == "validate":
            print(f"validated {len(manifest.samples)} consented sample(s)")
            return 0
        analyzer = resolve_engine(arguments.engine)
        report = write_benchmark_report(
            manifest_path,
            manifest,
            arguments.output,
            analyzer=analyzer,
        )
        aggregate = report["aggregate"]
        print(
            f"wrote {report['sampleCount']} sample(s); "
            f"content cut {aggregate['contentCutRate']:.4%}; "
            f"false split {aggregate['falseSplitRate']:.4%}"
        )
        return 0
    except (AnnotationError, ManifestError, ValueError, OSError) as error:
        print(f"question-bakeoff: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
