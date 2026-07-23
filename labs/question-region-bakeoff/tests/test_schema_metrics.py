import json
import math
import tempfile
import unittest
from pathlib import Path

from question_bakeoff.metrics import aggregate_metrics, evaluate_sample
from question_bakeoff.schema import (
    BenchmarkSample,
    ManifestError,
    NormalizedPoint,
    NormalizedRect,
    Suggestion,
    load_manifest,
)


class NormalizedGeometryTests(unittest.TestCase):
    def test_rejects_non_finite_empty_and_out_of_bounds_rectangles(self) -> None:
        invalid = [
            (math.nan, 0.0, 0.2, 0.2),
            (0.0, 0.0, 0.0, 0.2),
            (-0.01, 0.0, 0.2, 0.2),
            (0.9, 0.0, 0.2, 0.2),
            (0.0, 0.9, 0.2, 0.2),
        ]

        for values in invalid:
            with self.subTest(values=values), self.assertRaises(ValueError):
                NormalizedRect(*values)

        with self.assertRaises(ValueError):
            NormalizedPoint(1.01, 0.2)

    def test_accepts_edges_with_small_float_tolerance(self) -> None:
        rect = NormalizedRect(0.2, 0.1, 0.8000000005, 0.9)
        self.assertAlmostEqual(rect.right, 1.0000000005)
        self.assertAlmostEqual(rect.area, 0.72000000045)


class ManifestTests(unittest.TestCase):
    def write_manifest(self, root: Path, *, image_path: str, consent: dict) -> Path:
        manifest = {
            "schemaVersion": 1,
            "consent": consent,
            "samples": [
                {
                    "id": "math-001",
                    "image": image_path,
                    "layout": "single-column",
                    "tags": ["formula", "handwriting"],
                    "regions": [{"x": 0.05, "y": 0.1, "width": 0.9, "height": 0.3}],
                    "anchors": [{"x": 0.07, "y": 0.11}],
                }
            ],
        }
        path = root / "manifest.json"
        path.write_text(json.dumps(manifest), encoding="utf-8")
        return path

    def test_loads_only_affirmatively_consented_relative_images(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            image = root / "images" / "math.png"
            image.parent.mkdir()
            image.write_bytes(b"fixture")
            path = self.write_manifest(
                root,
                image_path="images/math.png",
                consent={
                    "anonymized": True,
                    "authorizedForLocalEvaluation": True,
                    "recordedAt": "2026-07-22",
                },
            )

            manifest = load_manifest(path)

            self.assertEqual(manifest.schema_version, 1)
            self.assertEqual(manifest.samples[0].image_path, image.resolve())
            self.assertEqual(manifest.samples[0].tags, ("formula", "handwriting"))

    def test_rejects_missing_consent_and_paths_outside_the_fixture(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outside = root.parent / "outside-question.png"
            outside.write_bytes(b"private")
            self.addCleanup(lambda: outside.unlink(missing_ok=True))

            no_consent = self.write_manifest(
                root,
                image_path="images/missing.png",
                consent={
                    "anonymized": False,
                    "authorizedForLocalEvaluation": True,
                    "recordedAt": "2026-07-22",
                },
            )
            with self.assertRaisesRegex(ManifestError, "consent"):
                load_manifest(no_consent)

            escaped = self.write_manifest(
                root,
                image_path="../outside-question.png",
                consent={
                    "anonymized": True,
                    "authorizedForLocalEvaluation": True,
                    "recordedAt": "2026-07-22",
                },
            )
            with self.assertRaisesRegex(ManifestError, "inside"):
                load_manifest(escaped)


class MetricTests(unittest.TestCase):
    def sample(self) -> BenchmarkSample:
        return BenchmarkSample(
            sample_id="sample-a",
            image_path=Path("sample.png"),
            layout="single-column",
            tags=(),
            regions=(
                NormalizedRect(0.0, 0.0, 1.0, 0.4),
                NormalizedRect(0.0, 0.6, 1.0, 0.4),
            ),
            anchors=(NormalizedPoint(0.05, 0.02), NormalizedPoint(0.05, 0.62)),
        )

    def test_counts_cut_content_false_splits_anchors_and_uncertainty(self) -> None:
        suggestions = (
            Suggestion(
                rect=NormalizedRect(0.0, 0.0, 1.0, 0.4),
                confidence=0.95,
                anchor=NormalizedPoint(0.05, 0.02),
                engine="test",
                engine_version="1",
            ),
            Suggestion(
                rect=NormalizedRect(0.0, 0.43, 1.0, 0.1),
                confidence=0.4,
                anchor=NormalizedPoint(0.05, 0.43),
                engine="test",
                engine_version="1",
            ),
        )

        result = evaluate_sample(self.sample(), suggestions)

        self.assertEqual(result.matched_count, 1)
        self.assertEqual(result.matched_pairs, ((0, 0),))
        self.assertAlmostEqual(result.region_recall, 0.5)
        self.assertAlmostEqual(result.mean_matched_iou, 1.0)
        self.assertAlmostEqual(result.content_cut_rate, 0.5)
        self.assertAlmostEqual(result.false_split_rate, 0.5)
        self.assertAlmostEqual(result.question_start_recall, 0.5)
        self.assertEqual(result.uncertain_count, 1)

    def test_matching_uses_descending_iou_and_aggregate_is_weighted(self) -> None:
        first = evaluate_sample(
            self.sample(),
            (
                Suggestion(
                    rect=NormalizedRect(0.0, 0.0, 1.0, 0.4),
                    confidence=0.9,
                    anchor=NormalizedPoint(0.05, 0.02),
                    engine="test",
                    engine_version="1",
                ),
                Suggestion(
                    rect=NormalizedRect(0.0, 0.58, 1.0, 0.42),
                    confidence=0.9,
                    anchor=NormalizedPoint(0.05, 0.61),
                    engine="test",
                    engine_version="1",
                ),
            ),
        )
        second_sample = BenchmarkSample(
            sample_id="sample-b",
            image_path=Path("sample-b.png"),
            layout="single-column",
            tags=(),
            regions=(NormalizedRect(0.0, 0.0, 1.0, 1.0),),
            anchors=(),
        )
        second = evaluate_sample(second_sample, ())

        aggregate = aggregate_metrics((first, second))

        self.assertEqual(first.matched_pairs, ((0, 0), (1, 1)))
        self.assertEqual(aggregate.sample_count, 2)
        self.assertEqual(aggregate.truth_count, 3)
        self.assertAlmostEqual(aggregate.region_recall, 2 / 3)
        self.assertGreater(aggregate.content_cut_rate, 0.3)
        self.assertFalse(aggregate.passes_300_image_gate)


if __name__ == "__main__":
    unittest.main()
