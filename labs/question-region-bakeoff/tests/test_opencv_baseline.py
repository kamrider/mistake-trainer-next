import tempfile
import unittest
from pathlib import Path

import cv2
import numpy as np
from PIL import Image

from question_bakeoff.opencv_baseline import (
    ENGINE_NAME,
    ENGINE_VERSION,
    analyze_image,
    detect_columns,
    detect_page_quad,
    estimate_skew_degrees,
    suggest_question_regions,
    threshold_foreground,
    warp_page,
)


def white_page(height: int = 1000, width: int = 700) -> np.ndarray:
    return np.full((height, width, 3), 255, dtype=np.uint8)


def draw_question(image: np.ndarray, x0: int, x1: int, y0: int, y1: int) -> None:
    for y in range(y0, y1, 18):
        cv2.rectangle(image, (x0, y), (x1, min(y + 7, y1 - 1)), (20, 20, 20), -1)
    cv2.circle(image, (x1 - 30, y0 + (y1 - y0) // 2), 18, (20, 20, 20), 3)


class PageGeometryTests(unittest.TestCase):
    def test_detects_and_warps_a_large_page_quadrilateral(self) -> None:
        image = np.full((800, 1000, 3), 35, dtype=np.uint8)
        source = np.array([[150, 90], [870, 130], [820, 720], [110, 670]], dtype=np.int32)
        cv2.fillConvexPoly(image, source, (250, 250, 250))
        cv2.polylines(image, [source], True, (0, 0, 0), 8)

        quad = detect_page_quad(image)

        self.assertIsNotNone(quad)
        self.assertEqual(quad.shape, (4, 2))
        warped = warp_page(image, quad)
        self.assertGreater(warped.shape[0], 500)
        self.assertGreater(warped.shape[1], 600)

    def test_estimates_small_skew_and_rejects_extreme_rotation(self) -> None:
        base = white_page(600, 800)
        for y in range(120, 500, 55):
            cv2.rectangle(base, (100, y), (700, y + 9), (0, 0, 0), -1)
        matrix = cv2.getRotationMatrix2D((400, 300), 7.0, 1.0)
        rotated = cv2.warpAffine(base, matrix, (800, 600), borderValue=(255, 255, 255))

        skew = estimate_skew_degrees(rotated)

        self.assertGreater(abs(skew), 5.0)
        self.assertLess(abs(skew), 9.0)


class RegionSuggestionTests(unittest.TestCase):
    def test_splits_three_vertical_question_blocks_in_reading_order(self) -> None:
        image = white_page()
        draw_question(image, 55, 640, 80, 230)
        draw_question(image, 55, 640, 370, 540)
        draw_question(image, 55, 640, 700, 900)

        suggestions = suggest_question_regions(image)

        self.assertEqual(len(suggestions), 3)
        self.assertEqual([entry.engine for entry in suggestions], [ENGINE_NAME] * 3)
        self.assertEqual([entry.engine_version for entry in suggestions], [ENGINE_VERSION] * 3)
        self.assertEqual([entry.rect.y for entry in suggestions], sorted(entry.rect.y for entry in suggestions))
        self.assertTrue(all(entry.confidence >= 0.75 for entry in suggestions))

    def test_detects_two_columns_only_when_both_sides_have_ink(self) -> None:
        image = white_page(900, 1000)
        draw_question(image, 55, 420, 80, 250)
        draw_question(image, 55, 420, 530, 760)
        draw_question(image, 580, 945, 100, 300)
        draw_question(image, 580, 945, 560, 790)
        binary = threshold_foreground(image)

        columns = detect_columns(binary)
        suggestions = suggest_question_regions(image)

        self.assertEqual(len(columns), 2)
        self.assertEqual(len(suggestions), 4)
        self.assertLess(max(entry.rect.right for entry in suggestions[:2]), 0.56)
        self.assertGreater(min(entry.rect.x for entry in suggestions[2:]), 0.44)

    def test_region_boundaries_do_not_cross_foreground_components(self) -> None:
        image = white_page(700, 600)
        draw_question(image, 50, 550, 70, 250)
        draw_question(image, 50, 550, 420, 620)
        binary = threshold_foreground(image)

        suggestions = suggest_question_regions(image)

        self.assertEqual(len(suggestions), 2)
        for suggestion in suggestions:
            top = max(0, min(binary.shape[0] - 1, round(suggestion.rect.y * binary.shape[0])))
            bottom = max(0, min(binary.shape[0] - 1, round(suggestion.rect.bottom * binary.shape[0]) - 1))
            left = max(0, round(suggestion.rect.x * binary.shape[1]))
            right = min(binary.shape[1], round(suggestion.rect.right * binary.shape[1]))
            self.assertEqual(int(np.count_nonzero(binary[top, left:right])), 0)
            self.assertEqual(int(np.count_nonzero(binary[bottom, left:right])), 0)

    def test_blank_page_returns_one_explicitly_uncertain_region(self) -> None:
        suggestions = suggest_question_regions(white_page())

        self.assertEqual(len(suggestions), 1)
        self.assertLess(suggestions[0].confidence, 0.75)
        self.assertEqual(suggestions[0].uncertain_reason, "insufficient_foreground")

    def test_analysis_reads_unicode_paths_and_records_runtime_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "数学题.png"
            image = white_page(600, 500)
            draw_question(image, 40, 460, 100, 260)
            encoded, bytes_ = cv2.imencode(".png", image)
            self.assertTrue(encoded)
            path.write_bytes(bytes_.tobytes())

            result = analyze_image(path)

            self.assertEqual(result.engine, ENGINE_NAME)
            self.assertEqual(result.engine_version, ENGINE_VERSION)
            self.assertGreaterEqual(result.runtime_ms, 0.0)
            self.assertGreaterEqual(len(result.suggestions), 1)
            self.assertEqual(result.original_width, 500)
            self.assertEqual(result.original_height, 600)

    def test_analysis_applies_exif_orientation_before_measuring_and_suggesting(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "portrait-after-exif.jpg"
            image = Image.new("RGB", (80, 40), "white")
            exif = Image.Exif()
            exif[274] = 6
            image.save(path, format="JPEG", exif=exif)

            result = analyze_image(path)

            self.assertEqual(result.original_width, 40)
            self.assertEqual(result.original_height, 80)


if __name__ == "__main__":
    unittest.main()
