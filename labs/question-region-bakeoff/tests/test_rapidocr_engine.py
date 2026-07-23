import unittest

from question_bakeoff.rapidocr_engine import (
    OcrBox,
    is_question_anchor,
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


class AnchorAssemblyTests(unittest.TestCase):
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
        self.assertLessEqual(regions[0].rect.bottom, regions[1].rect.y)
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
        self.assertLessEqual(regions[0].rect.right, regions[2].rect.x)

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
