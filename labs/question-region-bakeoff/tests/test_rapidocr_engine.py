import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

import numpy as np
from rapidocr import ModelType, OCRVersion

from question_bakeoff.rapidocr_engine import (
    OcrBox,
    engine_name_for,
    is_question_anchor,
    make_analyzer,
    question_anchor_number,
    suggest_from_ocr_boxes,
)


def box(
    left: float,
    top: float,
    right: float,
    bottom: float,
    text: str,
    confidence: float,
    *,
    kind: str = "text",
) -> OcrBox:
    return OcrBox(left, top, right, bottom, text, confidence, kind)


class QuestionAnchorTests(unittest.TestCase):
    def test_accepts_chinese_english_and_parenthesized_question_numbers(self) -> None:
        for text in ("1. 已知函数", "12、如图", "（3）求证", "(4) Calculate", "Q5. Choose"):
            with self.subTest(text=text):
                self.assertTrue(is_question_anchor(text))

    def test_rejects_options_section_headings_and_number_like_content(self) -> None:
        for text in ("A. 选项", "H、选项", "一、选择题", "III. Reading", "3.14", "2026 年"):
            with self.subTest(text=text):
                self.assertFalse(is_question_anchor(text))

    def test_relaxed_numbers_are_available_only_for_aligned_sequence_recovery(self) -> None:
        self.assertEqual(question_anchor_number("2", allow_relaxed=True), 2)
        self.assertEqual(question_anchor_number("1 (1+5i)i 的虚部"), None)
        self.assertEqual(
            question_anchor_number("1 (1+5i)i 的虚部", allow_relaxed=True),
            1,
        )
        self.assertIsNone(question_anchor_number("2026 年", allow_relaxed=True))


class AnalyzerConfigurationTests(unittest.TestCase):
    @staticmethod
    def _result() -> SimpleNamespace:
        return SimpleNamespace(
            boxes=np.asarray(
                [
                    [[10, 10], [90, 10], [90, 20], [10, 20]],
                    [[10, 50], [90, 50], [90, 60], [10, 60]],
                ],
                dtype=np.float64,
            ),
            txts=["1. First", "2. Second"],
            scores=np.asarray([0.99, 0.98], dtype=np.float64),
        )

    def test_small_and_medium_use_explicit_ppocrv6_models(self) -> None:
        image = np.full((100, 100, 3), 255, dtype=np.uint8)
        for model_type in ("small", "medium"):
            with self.subTest(model_type=model_type):
                runtime = Mock(return_value=self._result())
                factory = Mock(return_value=runtime)
                analyzer = make_analyzer(model_type, runtime_factory=factory)

                with patch(
                    "question_bakeoff.rapidocr_engine._decode_path",
                    return_value=image,
                ):
                    result = analyzer("fixture.png")

                factory.assert_called_once_with(
                    params={
                        "Det.ocr_version": OCRVersion.PPOCRV6,
                        "Det.model_type": (
                            ModelType.SMALL
                            if model_type == "small"
                            else ModelType.MEDIUM
                        ),
                        "Rec.ocr_version": OCRVersion.PPOCRV6,
                        "Rec.model_type": (
                            ModelType.SMALL
                            if model_type == "small"
                            else ModelType.MEDIUM
                        ),
                    }
                )
                self.assertEqual(result.engine, engine_name_for(model_type))
                self.assertIn(f"ppocrv6-{model_type}", result.engine_version)
                self.assertEqual(
                    {item.engine for item in result.suggestions},
                    {engine_name_for(model_type)},
                )

    def test_analyzer_reuses_one_runtime_between_images(self) -> None:
        runtime = Mock(return_value=self._result())
        factory = Mock(return_value=runtime)
        analyzer = make_analyzer("small", runtime_factory=factory)
        image = np.full((100, 100, 3), 255, dtype=np.uint8)

        with patch(
            "question_bakeoff.rapidocr_engine._decode_path",
            return_value=image,
        ):
            analyzer("first.png")
            analyzer("second.png")

        self.assertEqual(factory.call_count, 1)
        self.assertEqual(runtime.call_count, 2)

    def test_rejects_an_unknown_model_tier_before_loading_a_runtime(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported PP-OCRv6 model tier"):
            make_analyzer("server")


class AnchorAssemblyTests(unittest.TestCase):
    def test_discards_numbered_instructions_before_questions_restart(self) -> None:
        boxes = (
            box(30, 40, 800, 70, "1. 答题前填写姓名", 0.99),
            box(30, 80, 800, 110, "2. 核对条形码", 0.99),
            box(30, 120, 800, 150, "3. 正确填涂", 0.99),
            box(30, 240, 700, 280, "1 第一题缺少标点", 0.98),
            box(30, 360, 45, 390, "2", 0.99),
            box(30, 500, 700, 540, "3. 第三题", 0.98),
            box(30, 650, 60, 675, "40", 0.99),
        )

        regions = suggest_from_ocr_boxes(1_000, 700, boxes)

        self.assertEqual(len(regions), 3)
        self.assertAlmostEqual(regions[0].rect.y, (240 - 10.5) / 700)
        self.assertAlmostEqual(regions[1].rect.y, (360 - 10.5) / 700)
        self.assertAlmostEqual(regions[2].rect.y, (500 - 10.5) / 700)

    def test_wide_header_does_not_cancel_a_strong_two_column_question_sequence(self) -> None:
        boxes = (
            box(40, 100, 420, 140, "1. left", 0.98),
            box(40, 420, 430, 460, "2. left", 0.97),
            box(560, 100, 920, 140, "3. right", 0.98),
            box(560, 420, 930, 460, "4. right", 0.97),
            box(100, 20, 900, 60, "2026 全国统一考试", 0.99),
        )

        regions = suggest_from_ocr_boxes(1_000, 700, boxes)

        self.assertEqual(len(regions), 4)
        self.assertGreater(regions[0].rect.right, regions[2].rect.x)
        self.assertLess(regions[0].rect.width, 0.6)
        self.assertLess(regions[2].rect.width, 0.6)

    def test_regions_do_not_cut_detected_formula_or_figure_blocks(self) -> None:
        boxes = (
            box(40, 100, 300, 145, "1. 已知", 0.98),
            box(60, 150, 900, 360, "formula-or-figure", 0.90, kind="non_text"),
            box(40, 400, 300, 445, "2. 求", 0.98),
            box(60, 455, 700, 560, "第二题内容", 0.94),
        )

        regions = suggest_from_ocr_boxes(1000, 800, boxes)

        self.assertEqual(len(regions), 2)
        self.assertGreaterEqual(regions[0].rect.bottom, 360 / 800)
        self.assertLessEqual(regions[0].rect.bottom - regions[1].rect.y, 0.031)
        self.assertEqual(regions[0].rect.x, 0.0)
        self.assertEqual(regions[0].rect.right, 1.0)

    def test_two_columns_use_column_major_reading_order(self) -> None:
        boxes = (
            box(40, 80, 300, 120, "1. 左上", 0.98),
            box(40, 380, 300, 420, "2. 左下", 0.97),
            box(560, 90, 850, 130, "3. 右上", 0.98),
            box(560, 410, 850, 450, "4. 右下", 0.97),
        )

        regions = suggest_from_ocr_boxes(1000, 700, boxes)

        self.assertEqual(len(regions), 4)
        self.assertLess(regions[0].rect.x, regions[2].rect.x)
        self.assertLess(regions[0].rect.y, regions[1].rect.y)
        self.assertLess(regions[2].rect.y, regions[3].rect.y)
        self.assertGreater(regions[0].rect.right, regions[2].rect.x)

    def test_low_confidence_or_missing_anchors_return_one_uncertain_content_region(self) -> None:
        for boxes in (
            (
                box(40, 100, 500, 150, "答案与解析", 0.99),
                box(60, 180, 800, 500, "answer-figure", 0.90, kind="non_text"),
            ),
            (
                box(40, 100, 300, 145, "1. 模糊题号", 0.44),
                box(60, 160, 800, 500, "正文", 0.96),
            ),
        ):
            with self.subTest(boxes=boxes):
                regions = suggest_from_ocr_boxes(1000, 700, boxes)
                self.assertEqual(len(regions), 1)
                self.assertLess(regions[0].confidence, 0.75)
                self.assertEqual(
                    regions[0].uncertain_reason,
                    "insufficient_question_anchors",
                )
                self.assertEqual(regions[0].rect.x, 0.0)
                self.assertEqual(regions[0].rect.y, 0.0)
                self.assertEqual(regions[0].rect.width, 1.0)
                self.assertEqual(regions[0].rect.height, 1.0)

    def test_option_labels_do_not_split_a_long_question(self) -> None:
        boxes = (
            box(40, 80, 300, 120, "1. 选择正确答案", 0.98),
            box(70, 160, 600, 200, "A. 第一个选项", 0.99),
            box(70, 220, 600, 260, "B. 第二个选项", 0.99),
            box(60, 290, 900, 440, "question-figure", 0.90, kind="non_text"),
            box(40, 500, 300, 540, "2. 下一题", 0.98),
        )

        regions = suggest_from_ocr_boxes(1000, 700, boxes)

        self.assertEqual(len(regions), 2)
        self.assertGreaterEqual(regions[0].rect.bottom, 440 / 700)

    def test_rejects_invalid_dimensions_and_out_of_bounds_boxes(self) -> None:
        with self.assertRaisesRegex(ValueError, "source dimensions"):
            suggest_from_ocr_boxes(0, 700, ())
        with self.assertRaisesRegex(ValueError, "OCR box"):
            suggest_from_ocr_boxes(
                1000,
                700,
                (box(-1, 10, 100, 30, "1. invalid", 0.9),),
            )


if __name__ == "__main__":
    unittest.main()
