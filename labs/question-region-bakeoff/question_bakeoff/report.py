from __future__ import annotations

import hashlib
import html
import io
import json
import shutil
import time
import uuid
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence

import cv2
import numpy
import PIL
from PIL import Image, ImageDraw, ImageOps

from .metrics import AggregateMetrics, SampleMetrics, aggregate_metrics, evaluate_sample
from .opencv_baseline import Analysis, analyze_image
from .schema import BenchmarkManifest, BenchmarkSample, NormalizedPoint, NormalizedRect, Suggestion


OUTPUT_MARKER = ".question-bakeoff-output"
OUTPUT_MARKER_VALUE = "question-region-bakeoff-output-v1\n"
REPORT_SCHEMA_VERSION = 1


class ReportError(ValueError):
    """Raised when report output would be unsafe or ambiguous."""


def _camel_metrics(value: SampleMetrics | AggregateMetrics) -> dict[str, Any]:
    raw = asdict(value)
    rename = {
        "sample_id": "sampleId",
        "sample_count": "sampleCount",
        "truth_count": "truthCount",
        "prediction_count": "predictionCount",
        "matched_count": "matchedCount",
        "matched_pairs": "matchedPairs",
        "region_recall": "regionRecall",
        "mean_matched_iou": "meanMatchedIou",
        "content_cut_rate": "contentCutRate",
        "false_split_rate": "falseSplitRate",
        "question_start_recall": "questionStartRecall",
        "uncertain_count": "uncertainCount",
        "truth_area": "truthArea",
        "cut_area": "cutArea",
        "iou_sum": "iouSum",
        "false_split_count": "falseSplitCount",
        "anchor_count": "anchorCount",
        "anchor_hit_count": "anchorHitCount",
        "passes_60_image_gate": "passes60ImageGate",
        "passes_300_image_gate": "passes300ImageGate",
    }
    return {rename.get(key, key): value for key, value in raw.items()}


def _point(value: NormalizedPoint) -> dict[str, float]:
    return value.as_dict()


def _rect(value: NormalizedRect) -> dict[str, float]:
    return value.as_dict()


def _suggestion(value: Suggestion) -> dict[str, Any]:
    return {
        "rect": _rect(value.rect),
        "confidence": value.confidence,
        "anchor": _point(value.anchor) if value.anchor else None,
        "engine": value.engine,
        "engineVersion": value.engine_version,
        "uncertainReason": value.uncertain_reason,
    }


def _decode_image(path: Path) -> Image.Image:
    try:
        with Image.open(io.BytesIO(path.read_bytes())) as source:
            return ImageOps.exif_transpose(source).convert("RGB")
    except (OSError, ValueError) as error:
        raise ReportError("benchmark image could not be decoded for its overlay") from error


def _pixel_rect(rect: NormalizedRect, width: int, height: int) -> tuple[tuple[int, int], tuple[int, int]]:
    left = max(0, min(width - 1, round(rect.x * width)))
    top = max(0, min(height - 1, round(rect.y * height)))
    right = max(left + 1, min(width - 1, round(rect.right * width) - 1))
    bottom = max(top + 1, min(height - 1, round(rect.bottom * height) - 1))
    return (left, top), (right, bottom)


def _write_overlay(
    destination: Path,
    sample: BenchmarkSample,
    analysis: Analysis,
) -> None:
    overlay = _decode_image(sample.image_path)
    width, height = overlay.size
    draw = ImageDraw.Draw(overlay)
    for index, truth in enumerate(sample.regions, start=1):
        top_left, bottom_right = _pixel_rect(truth, width, height)
        draw.rectangle((*top_left, *bottom_right), outline=(40, 170, 70), width=4)
        draw.text(
            (top_left[0] + 5, min(height - 16, top_left[1] + 7)),
            f"T{index}",
            fill=(30, 130, 50),
            stroke_width=1,
            stroke_fill=(255, 255, 255),
        )
    for index, suggestion in enumerate(analysis.suggestions, start=1):
        color = (245, 175, 0) if suggestion.confidence >= 0.75 else (220, 55, 40)
        top_left, bottom_right = _pixel_rect(suggestion.rect, width, height)
        draw.rectangle((*top_left, *bottom_right), outline=color, width=3)
        draw.text(
            (top_left[0] + 5, max(2, top_left[1] - 14)),
            f"P{index} {suggestion.confidence:.2f}",
            fill=color,
            stroke_width=1,
            stroke_fill=(255, 255, 255),
        )
        if suggestion.anchor:
            center = (
                max(0, min(width - 1, round(suggestion.anchor.x * width))),
                max(0, min(height - 1, round(suggestion.anchor.y * height))),
            )
            draw.ellipse(
                (center[0] - 7, center[1] - 7, center[0] + 7, center[1] + 7),
                fill=color,
            )
    if analysis.page_quad:
        points = [(round(point.x * width), round(point.y * height)) for point in analysis.page_quad]
        draw.line((*points, points[0]), fill=(30, 120, 220), width=3, joint="curve")
    try:
        overlay.save(destination, format="PNG")
    except OSError as error:
        raise ReportError("overlay PNG encoding failed") from error
    finally:
        overlay.close()


def _report_document(
    manifest_path: Path,
    manifest: BenchmarkManifest,
    analyses: Sequence[Analysis],
    metrics: Sequence[SampleMetrics],
) -> dict[str, Any]:
    aggregate = aggregate_metrics(metrics)
    samples: list[dict[str, Any]] = []
    for sample, analysis, sample_metrics in zip(manifest.samples, analyses, metrics, strict=True):
        samples.append(
            {
                "id": sample.sample_id,
                "layout": sample.layout,
                "tags": list(sample.tags),
                "dimensions": {
                    "width": analysis.original_width,
                    "height": analysis.original_height,
                },
                "runtimeMs": round(analysis.runtime_ms, 3),
                "skewDegrees": round(analysis.skew_degrees, 4),
                "pageQuad": [_point(point) for point in analysis.page_quad],
                "truth": {
                    "regions": [_rect(rect) for rect in sample.regions],
                    "anchors": [_point(point) for point in sample.anchors],
                },
                "suggestions": [_suggestion(suggestion) for suggestion in analysis.suggestions],
                "metrics": _camel_metrics(sample_metrics),
                "overlay": f"overlays/{sample.sample_id}.png",
            }
        )
    return {
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "generatedAtUtc": datetime.now(timezone.utc).isoformat(),
        "manifestDigestSha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "engine": analyses[0].engine if analyses else "opencv-whitespace",
        "engineVersion": analyses[0].engine_version if analyses else "unknown",
        "runtimeVersions": {
            "numpy": numpy.__version__,
            "opencv": cv2.__version__,
            "pillow": PIL.__version__,
        },
        "sampleCount": len(samples),
        "thresholds": {
            "questionStartRecall": 0.95,
            "contentCutRateExclusiveMaximum": 0.005,
            "falseSplitRateExclusiveMaximum": 0.03,
            "reviewConfidence": 0.75,
            "minimumDecisionSamples": 300,
        },
        "aggregate": _camel_metrics(aggregate),
        "samples": samples,
        "notice": "Synthetic test success is not a production model decision. Automatic splitting remains disabled until the consented benchmark passes.",
    }


def _percent(value: float) -> str:
    return f"{value * 100:.2f}%"


def _html_document(report: dict[str, Any]) -> str:
    aggregate = report["aggregate"]
    verdict = "达到 300 张发布阈值" if aggregate["passes300ImageGate"] else "未达到发布阈值"
    verdict_class = "pass" if aggregate["passes300ImageGate"] else "fail"
    cards: list[str] = []
    for sample in report["samples"]:
        sample_metrics = sample["metrics"]
        layout = html.escape(sample["layout"])
        tags = " · ".join(html.escape(tag) for tag in sample["tags"]) or "未标记"
        cards.append(
            f"""
            <article class="sample">
              <img src="{html.escape(sample['overlay'], quote=True)}" alt="{html.escape(sample['id'])} 的真值与建议叠加图" loading="lazy" />
              <div><p class="sample-id">{html.escape(sample['id'])}</p><h2>{layout}</h2><p>{tags}</p>
              <dl><div><dt>区域召回</dt><dd>{_percent(sample_metrics['regionRecall'])}</dd></div>
              <div><dt>内容切断</dt><dd>{_percent(sample_metrics['contentCutRate'])}</dd></div>
              <div><dt>错误分割</dt><dd>{_percent(sample_metrics['falseSplitRate'])}</dd></div>
              <div><dt>耗时</dt><dd>{sample['runtimeMs']:.1f} ms</dd></div></dl></div>
            </article>"""
        )
    return f"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8" /><meta name="viewport" content="width=device-width,initial-scale=1" />
<title>Question Region Bake-off</title><style>
:root{{color:#22302b;background:#f6f1e7;font-family:"Microsoft YaHei UI",system-ui,sans-serif}}*{{box-sizing:border-box}}body{{margin:0}}main{{width:min(1080px,100%);margin:auto;padding:32px 20px 80px}}h1{{font-family:Georgia,"STSong",serif;font-size:clamp(32px,6vw,58px);margin:.15em 0}}.eyebrow,.sample-id{{color:#a24f39;font-weight:800;letter-spacing:.08em}}.summary{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:28px 0}}.metric,.sample{{border:1px solid #d9cfbd;border-radius:20px;background:#fffdf8;box-shadow:0 15px 40px rgba(50,42,30,.07)}}.metric{{padding:18px}}.metric b{{display:block;margin-top:8px;font:700 28px Georgia,serif}}.verdict{{padding:14px 16px;border-radius:14px;font-weight:800}}.verdict.fail{{color:#7e3b2d;background:#f0ddd2}}.verdict.pass{{color:#244f42;background:#dce9df}}.sample{{display:grid;grid-template-columns:minmax(0,1.3fr) minmax(240px,.7fr);gap:20px;margin-top:18px;padding:14px}}.sample img{{width:100%;max-height:680px;object-fit:contain;border-radius:13px;background:#e8ddc7}}.sample h2{{margin:.25em 0}}dl{{display:grid;grid-template-columns:1fr 1fr;gap:8px}}dl div{{padding:10px;border-radius:10px;background:#eee5d6}}dt{{font-size:12px;color:#69736e}}dd{{margin:4px 0 0;font-weight:800}}@media(max-width:700px){{.sample{{grid-template-columns:1fr}}}}
</style></head><body><main><p class="eyebrow">MISTAKE TRAINER · LOCAL LAB</p><h1>Question Region Bake-off</h1>
<p>只显示匿名评测叠加图；绿色是真值，黄色是高置信建议，红色是必须确认的低置信建议。</p>
<p class="verdict {verdict_class}">{verdict}。合成测试不能作为模型发布证据。</p>
<section class="summary"><div class="metric">样本数<b>{report['sampleCount']}</b></div><div class="metric">题号起点召回<b>{_percent(aggregate['questionStartRecall'])}</b></div><div class="metric">内容切断率<b>{_percent(aggregate['contentCutRate'])}</b></div><div class="metric">错误分割率<b>{_percent(aggregate['falseSplitRate'])}</b></div></section>
{''.join(cards)}</main></body></html>"""


def _owned_output(path: Path) -> bool:
    marker = path / OUTPUT_MARKER
    try:
        return path.is_dir() and not path.is_symlink() and marker.read_text(encoding="ascii") == OUTPUT_MARKER_VALUE
    except (OSError, UnicodeError):
        return False


def _rename_with_retry(source: Path, destination: Path) -> None:
    delays = (0.0, 0.04, 0.08, 0.16, 0.32, 0.5)
    last_error: OSError | None = None
    for delay in delays:
        if delay:
            time.sleep(delay)
        try:
            source.rename(destination)
            return
        except PermissionError as error:
            last_error = error
    if last_error is not None:
        raise last_error


def _replace_output(staging: Path, output: Path) -> None:
    backup = output.parent / f".{output.name}.question-bakeoff-{uuid.uuid4().hex}.old"
    moved_existing = False
    try:
        if output.exists():
            _rename_with_retry(output, backup)
            moved_existing = True
        _rename_with_retry(staging, output)
        if moved_existing:
            shutil.rmtree(backup)
    except Exception:
        if not output.exists() and moved_existing and backup.exists():
            _rename_with_retry(backup, output)
        raise
    finally:
        if staging.exists():
            shutil.rmtree(staging, ignore_errors=True)
        if backup.exists() and output.exists():
            shutil.rmtree(backup, ignore_errors=True)


def write_benchmark_report(
    manifest_path: Path,
    manifest: BenchmarkManifest,
    output: Path,
) -> dict[str, Any]:
    output = output.absolute()
    if output == output.parent or not output.name:
        raise ReportError("output must be a dedicated report directory")
    if output.exists() and not _owned_output(output):
        raise ReportError("output directory is not owned by the question bake-off lab")
    output.parent.mkdir(parents=True, exist_ok=True)

    analyses: list[Analysis] = []
    metrics: list[SampleMetrics] = []
    for sample in manifest.samples:
        analysis = analyze_image(sample.image_path)
        analyses.append(analysis)
        metrics.append(evaluate_sample(sample, analysis.suggestions))
    report = _report_document(manifest_path, manifest, analyses, metrics)

    staging = output.parent / f".{output.name}.question-bakeoff-{uuid.uuid4().hex}.tmp"
    staging.mkdir()
    try:
        overlays = staging / "overlays"
        overlays.mkdir()
        for sample, analysis in zip(manifest.samples, analyses, strict=True):
            _write_overlay(overlays / f"{sample.sample_id}.png", sample, analysis)
        (staging / OUTPUT_MARKER).write_text(OUTPUT_MARKER_VALUE, encoding="ascii")
        (staging / "report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (staging / "index.html").write_text(_html_document(report), encoding="utf-8")
        _replace_output(staging, output)
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    return report
