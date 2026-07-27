import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

import cv2
import numpy as np

from question_bakeoff.cli import main
from question_bakeoff.engine import resolve_engine
from question_bakeoff.opencv_baseline import analyze_image
from question_bakeoff.report import OUTPUT_MARKER_VALUE


def write_fixture(root: Path) -> Path:
    image = np.full((800, 600, 3), 255, dtype=np.uint8)
    for y0, y1 in ((70, 280), (470, 730)):
        for y in range(y0, y1, 20):
            cv2.rectangle(image, (45, y), (555, min(y + 8, y1 - 1)), (20, 20, 20), -1)
    encoded, payload = cv2.imencode(".png", image)
    if not encoded:
        raise AssertionError("encode fixture")
    images = root / "images"
    images.mkdir()
    (images / "sample.png").write_bytes(payload.tobytes())
    manifest = {
        "schemaVersion": 1,
        "consent": {
            "anonymized": True,
            "authorizedForLocalEvaluation": True,
            "recordedAt": "2026-07-22",
        },
        "samples": [
            {
                "id": "sample-001",
                "image": "images/sample.png",
                "layout": "single-column <not-html>",
                "tags": ["synthetic"],
                "regions": [
                    {"x": 0.05, "y": 0.07, "width": 0.9, "height": 0.3},
                    {"x": 0.05, "y": 0.57, "width": 0.9, "height": 0.37},
                ],
                "anchors": [{"x": 0.07, "y": 0.08}, {"x": 0.07, "y": 0.58}],
            }
        ],
    }
    path = root / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


class CliReportTests(unittest.TestCase):
    def test_annotate_dispatches_without_opening_a_browser_when_requested(self) -> None:
        with patch("question_bakeoff.cli.serve_annotator") as serve:
            exit_code = main(["annotate", "data", "--no-open"])

        self.assertEqual(exit_code, 0)
        serve.assert_called_once_with(Path("data"), open_browser=False)

    def test_self_check_reports_both_pinned_engines(self) -> None:
        output = io.StringIO()

        with redirect_stdout(output):
            exit_code = main(["self-check"])

        report = json.loads(output.getvalue())
        self.assertEqual(exit_code, 0)
        self.assertEqual(report["rapidocr"], "3.9.2")
        self.assertEqual(report["onnxruntime"], "1.27.0")
        self.assertEqual(
            report["availableEngines"],
            [
                "opencv-whitespace",
                "ppocrv6-small-anchor",
                "ppocrv6-medium-anchor",
            ],
        )

    def test_resolve_engine_rejects_unknown_name(self) -> None:
        self.assertIs(resolve_engine("opencv-whitespace"), analyze_image)
        self.assertEqual(
            resolve_engine("ppocrv6-small-anchor").model_type,
            "small",
        )
        self.assertEqual(
            resolve_engine("ppocrv6-medium-anchor").model_type,
            "medium",
        )
        self.assertEqual(resolve_engine("rapidocr-anchor").model_type, "small")
        with self.assertRaisesRegex(ValueError, "unsupported question-region engine"):
            resolve_engine("chat-model")

    def test_cli_selects_the_named_engine_without_global_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = write_fixture(root)
            selected = []
            fake_report = {
                "sampleCount": 0,
                "aggregate": {
                    "contentCutRate": 0.0,
                    "falseSplitRate": 0.0,
                },
            }

            with patch(
                "question_bakeoff.cli.write_benchmark_report",
                side_effect=lambda _path, _manifest, _output, *, analyzer: (
                    selected.append(analyzer) or fake_report
                ),
            ):
                exit_code = main(
                    [
                        "run",
                        str(manifest),
                        "--output",
                        str(root / "report"),
                        "--engine",
                        "opencv-whitespace",
                    ]
                )

            self.assertEqual(exit_code, 0)
            self.assertEqual(selected, [analyze_image])

    def test_run_writes_deterministic_path_safe_report_html_and_overlay(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = write_fixture(root)
            output = root / "report"

            exit_code = main(["run", str(manifest), "--output", str(output)])

            self.assertEqual(exit_code, 0)
            self.assertTrue((output / ".question-bakeoff-output").is_file())
            report = json.loads((output / "report.json").read_text(encoding="utf-8"))
            self.assertEqual(report["schemaVersion"], 1)
            self.assertEqual(report["engine"], "opencv-whitespace")
            self.assertEqual(report["runtimeVersions"]["opencv"], cv2.__version__)
            self.assertEqual(report["sampleCount"], 1)
            self.assertEqual(report["samples"][0]["id"], "sample-001")
            self.assertIn("runtimeMs", report["samples"][0])
            self.assertEqual(
                report["samples"][0]["suggestions"][0]["engine"],
                "opencv-whitespace",
            )
            self.assertIn("contentCutRate", report["aggregate"])
            self.assertIn("passes300ImageGate", report["aggregate"])
            self.assertIn("questionStartRecall", report["thresholds"])
            serialized = json.dumps(report)
            self.assertNotIn(str(root), serialized)
            overlay_path = output / report["samples"][0]["overlay"]
            overlay = cv2.imdecode(np.frombuffer(overlay_path.read_bytes(), np.uint8), cv2.IMREAD_COLOR)
            self.assertEqual(overlay.shape[:2], (800, 600))
            html = (output / "index.html").read_text(encoding="utf-8")
            self.assertIn("Question Region Bake-off", html)
            self.assertIn("single-column &lt;not-html&gt;", html)
            self.assertNotIn("single-column <not-html>", html)
            self.assertNotIn(str(root), html)

    def test_validate_does_not_create_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = write_fixture(root)

            exit_code = main(["validate", str(manifest)])

            self.assertEqual(exit_code, 0)
            self.assertFalse((root / "report").exists())

    def test_refuses_foreign_output_and_replaces_only_owned_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = write_fixture(root)
            output = root / "report"
            output.mkdir()
            sentinel = output / "keep.txt"
            sentinel.write_text("foreign", encoding="utf-8")

            refused = main(["run", str(manifest), "--output", str(output)])

            self.assertEqual(refused, 2)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "foreign")
            sentinel.unlink()
            (output / ".question-bakeoff-output").write_text(
                OUTPUT_MARKER_VALUE,
                encoding="ascii",
            )
            stale = output / "stale.txt"
            stale.write_text("old", encoding="utf-8")

            replaced = main(["run", str(manifest), "--output", str(output)])

            self.assertEqual(replaced, 0)
            self.assertFalse(stale.exists())
            self.assertTrue((output / "report.json").is_file())

    def test_failed_run_keeps_the_previous_owned_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = write_fixture(root)
            output = root / "report"
            self.assertEqual(main(["run", str(manifest), "--output", str(output)]), 0)
            previous = (output / "report.json").read_bytes()
            (root / "images" / "sample.png").write_bytes(b"corrupt")

            failed = main(["run", str(manifest), "--output", str(output)])

            self.assertEqual(failed, 2)
            self.assertEqual((output / "report.json").read_bytes(), previous)
            leftovers = list(root.glob(".report.question-bakeoff-*.tmp"))
            self.assertEqual(leftovers, [])


if __name__ == "__main__":
    unittest.main()
