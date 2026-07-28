from __future__ import annotations

import hashlib
import json
import mimetypes
import os
import re
import secrets
import threading
import webbrowser
from datetime import date
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Mapping, Sequence
from urllib.parse import parse_qs, quote, urlsplit

from .schema import NormalizedPoint, NormalizedRect


DRAFT_FILENAME = "annotations.draft.json"
MANIFEST_FILENAME = "manifest.json"
SUPPORTED_IMAGE_SUFFIXES = frozenset({".jpg", ".jpeg", ".png", ".webp"})
MAX_REQUEST_BYTES = 8 * 1024 * 1024
_SAMPLE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")


class AnnotationError(ValueError):
    """Raised when annotation state could expose files or create invalid evidence."""


def _mapping(value: Any, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise AnnotationError(f"{field} must be an object")
    return value


def _sequence(value: Any, field: str) -> Sequence[Any]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise AnnotationError(f"{field} must be an array")
    return value


def _image_paths(root: Path) -> tuple[str, ...]:
    images_root = (root / "images").resolve()
    if not images_root.is_dir():
        raise AnnotationError("create an images directory containing authorized copies first")
    discovered = []
    for path in images_root.rglob("*"):
        if path.is_file() and path.suffix.lower() in SUPPORTED_IMAGE_SUFFIXES:
            if not path.resolve().is_relative_to(images_root):
                raise AnnotationError("image links must remain inside the images directory")
            discovered.append(path.relative_to(root).as_posix())
    discovered.sort(key=str.casefold)
    if not discovered:
        raise AnnotationError("the images directory contains no supported PNG, JPEG, or WebP files")
    return tuple(discovered)


def _read_json(path: Path) -> Mapping[str, Any] | None:
    if not path.is_file():
        return None
    try:
        return _mapping(json.loads(path.read_text(encoding="utf-8")), path.name)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise AnnotationError(f"{path.name} must be readable UTF-8 JSON") from error


def _safe_id(image_path: str, used: set[str]) -> str:
    stem = Path(image_path).stem
    normalized = re.sub(r"[^A-Za-z0-9._-]+", "-", stem).strip("._-")
    if not normalized or not normalized[0].isalnum():
        normalized = "sample"
    normalized = normalized[:64]
    candidate = normalized
    if candidate in used:
        digest = hashlib.sha256(image_path.encode("utf-8")).hexdigest()[:10]
        candidate = f"{normalized[:68]}-{digest}"[:80]
    suffix = 2
    while candidate in used:
        marker = f"-{suffix}"
        candidate = f"{normalized[:80 - len(marker)]}{marker}"
        suffix += 1
    used.add(candidate)
    return candidate


def _default_consent() -> dict[str, Any]:
    return {
        "anonymized": False,
        "authorizedForLocalEvaluation": False,
        "recordedAt": date.today().isoformat(),
    }


def build_annotation_state(root: str | Path) -> dict[str, Any]:
    data_root = Path(root).resolve()
    image_paths = _image_paths(data_root)
    persisted = _read_json(data_root / DRAFT_FILENAME) or _read_json(data_root / MANIFEST_FILENAME)
    persisted_samples: dict[str, Mapping[str, Any]] = {}
    consent: Mapping[str, Any] = _default_consent()
    if persisted is not None:
        if persisted.get("schemaVersion") != 1:
            raise AnnotationError("annotation schemaVersion must be 1")
        consent = _mapping(persisted.get("consent"), "consent")
        for value in _sequence(persisted.get("samples"), "samples"):
            sample = _mapping(value, "sample")
            image = sample.get("image")
            if isinstance(image, str) and image in image_paths:
                persisted_samples[image] = sample

    used_ids: set[str] = set()
    samples: list[dict[str, Any]] = []
    for image_path in image_paths:
        previous = persisted_samples.get(image_path)
        previous_id = previous.get("id") if previous else None
        if isinstance(previous_id, str) and _SAMPLE_ID.fullmatch(previous_id) and previous_id not in used_ids:
            sample_id = previous_id
            used_ids.add(sample_id)
        else:
            sample_id = _safe_id(image_path, used_ids)
        samples.append(
            {
                "id": sample_id,
                "image": image_path,
                "layout": previous.get("layout", "unknown") if previous else "unknown",
                "tags": list(previous.get("tags", [])) if previous else [],
                "regions": list(previous.get("regions", [])) if previous else [],
                "anchors": list(previous.get("anchors", [])) if previous else [],
            }
        )
    state = {
        "schemaVersion": 1,
        "consent": dict(consent),
        "samples": samples,
    }
    return _validated_state(data_root, state)


def _validated_consent(value: Any) -> dict[str, Any]:
    consent = _mapping(value, "consent")
    anonymized = consent.get("anonymized") is True
    authorized = consent.get("authorizedForLocalEvaluation") is True
    recorded_at = consent.get("recordedAt")
    if not isinstance(recorded_at, str):
        raise AnnotationError("consent recordedAt must be an ISO date")
    try:
        date.fromisoformat(recorded_at)
    except ValueError as error:
        raise AnnotationError("consent recordedAt must be an ISO date") from error
    return {
        "anonymized": anonymized,
        "authorizedForLocalEvaluation": authorized,
        "recordedAt": recorded_at,
    }


def _validated_text_list(value: Any, field: str) -> list[str]:
    result: list[str] = []
    for entry in _sequence(value, field):
        if not isinstance(entry, str) or not entry.strip():
            raise AnnotationError(f"{field} entries must be non-empty strings")
        normalized = entry.strip()
        if normalized not in result:
            result.append(normalized)
    return result


def _validated_geometry(value: Any, field: str, rectangle: bool) -> list[dict[str, float]]:
    result: list[dict[str, float]] = []
    try:
        for entry in _sequence(value, field):
            item = _mapping(entry, field)
            geometry = NormalizedRect.from_mapping(item) if rectangle else NormalizedPoint.from_mapping(item)
            result.append(geometry.as_dict())
    except (ValueError, KeyError) as error:
        raise AnnotationError(f"{field} contains invalid geometry: {error}") from error
    return result


def _validated_state(root: Path, value: Any) -> dict[str, Any]:
    state = _mapping(value, "annotation state")
    if state.get("schemaVersion") != 1:
        raise AnnotationError("annotation schemaVersion must be 1")
    expected_images = _image_paths(root)
    samples = _sequence(state.get("samples"), "samples")
    if len(samples) != len(expected_images):
        raise AnnotationError("annotation samples must match the current images directory")
    validated_samples: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    seen_images: set[str] = set()
    for index, value in enumerate(samples):
        sample = _mapping(value, f"samples[{index}]")
        sample_id = sample.get("id")
        image = sample.get("image")
        layout = sample.get("layout")
        if not isinstance(sample_id, str) or not _SAMPLE_ID.fullmatch(sample_id):
            raise AnnotationError(f"samples[{index}].id is not a safe stable identifier")
        if sample_id in seen_ids:
            raise AnnotationError(f"duplicate sample id: {sample_id}")
        if not isinstance(image, str) or image not in expected_images:
            raise AnnotationError(f"samples[{index}].image is not a discovered image")
        if image in seen_images:
            raise AnnotationError(f"duplicate sample image: {image}")
        if not isinstance(layout, str) or not layout.strip():
            raise AnnotationError(f"samples[{index}].layout must be non-empty")
        seen_ids.add(sample_id)
        seen_images.add(image)
        validated_samples.append(
            {
                "id": sample_id,
                "image": image,
                "layout": layout.strip(),
                "tags": _validated_text_list(sample.get("tags", []), f"samples[{index}].tags"),
                "regions": _validated_geometry(
                    sample.get("regions", []),
                    f"samples[{index}].regions",
                    True,
                ),
                "anchors": _validated_geometry(
                    sample.get("anchors", []),
                    f"samples[{index}].anchors",
                    False,
                ),
            }
        )
    if seen_images != set(expected_images):
        raise AnnotationError("annotation samples must match the current images directory")
    return {
        "schemaVersion": 1,
        "consent": _validated_consent(state.get("consent")),
        "samples": validated_samples,
    }


def _atomic_json(path: Path, payload: Mapping[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.{secrets.token_hex(8)}.tmp")
    try:
        temporary.write_text(
            json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def save_annotation_state(root: str | Path, value: Any) -> dict[str, Any]:
    data_root = Path(root).resolve()
    state = _validated_state(data_root, value)
    labeled_count = sum(1 for sample in state["samples"] if sample["regions"])
    consent = state["consent"]
    complete = (
        labeled_count == len(state["samples"])
        and consent["anonymized"]
        and consent["authorizedForLocalEvaluation"]
    )
    _atomic_json(data_root / DRAFT_FILENAME, state)
    manifest_path = data_root / MANIFEST_FILENAME
    if complete:
        _atomic_json(manifest_path, state)
    else:
        manifest_path.unlink(missing_ok=True)
    return {
        "complete": complete,
        "labeledCount": labeled_count,
        "sampleCount": len(state["samples"]),
        "manifest": MANIFEST_FILENAME if complete else None,
    }


class _AnnotationServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, root: Path, token: str):
        super().__init__(("127.0.0.1", 0), _AnnotationHandler)
        self.data_root = root
        self.token = token


class _AnnotationHandler(BaseHTTPRequestHandler):
    server: _AnnotationServer

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _authorized(self) -> bool:
        return secrets.compare_digest(
            self.headers.get("X-Question-Bakeoff-Token", ""),
            self.server.token,
        )

    def _send(self, status: HTTPStatus, payload: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'none'; img-src 'self' blob:; style-src 'unsafe-inline'; "
            "script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'",
        )
        self.end_headers()
        self.wfile.write(payload)

    def _json(self, status: HTTPStatus, value: Any) -> None:
        self._send(
            status,
            json.dumps(value, ensure_ascii=False).encode("utf-8"),
            "application/json; charset=utf-8",
        )

    def _reject(self, status: HTTPStatus, message: str) -> None:
        self._json(status, {"error": message})

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request = urlsplit(self.path)
        if request.path == "/":
            html_path = Path(__file__).with_name("annotator.html")
            self._send(
                HTTPStatus.OK,
                html_path.read_bytes(),
                "text/html; charset=utf-8",
            )
            return
        if not self._authorized():
            self._reject(HTTPStatus.UNAUTHORIZED, "invalid local annotation token")
            return
        if request.path == "/api/state":
            try:
                state = build_annotation_state(self.server.data_root)
            except AnnotationError as error:
                self._reject(HTTPStatus.BAD_REQUEST, str(error))
                return
            self._json(HTTPStatus.OK, state)
            return
        if request.path == "/api/image":
            raw = parse_qs(request.query).get("path", [""])[0]
            allowed = set(_image_paths(self.server.data_root))
            if raw not in allowed:
                self._reject(HTTPStatus.NOT_FOUND, "image is not part of this annotation set")
                return
            image_path = (self.server.data_root / raw).resolve()
            content_type = mimetypes.guess_type(image_path.name)[0] or "application/octet-stream"
            self._send(HTTPStatus.OK, image_path.read_bytes(), content_type)
            return
        self._reject(HTTPStatus.NOT_FOUND, "not found")

    def do_PUT(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if urlsplit(self.path).path != "/api/state":
            self._reject(HTTPStatus.NOT_FOUND, "not found")
            return
        if not self._authorized():
            self._reject(HTTPStatus.UNAUTHORIZED, "invalid local annotation token")
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self._reject(HTTPStatus.BAD_REQUEST, "invalid content length")
            return
        if length <= 0 or length > MAX_REQUEST_BYTES:
            self._reject(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, "annotation payload is too large")
            return
        try:
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            result = save_annotation_state(self.server.data_root, payload)
        except (UnicodeError, json.JSONDecodeError, AnnotationError, OSError) as error:
            self._reject(HTTPStatus.BAD_REQUEST, str(error))
            return
        self._json(HTTPStatus.OK, result)

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if urlsplit(self.path).path != "/api/shutdown":
            self._reject(HTTPStatus.NOT_FOUND, "not found")
            return
        if not self._authorized():
            self._reject(HTTPStatus.UNAUTHORIZED, "invalid local annotation token")
            return
        self._json(HTTPStatus.OK, {"stopping": True})
        threading.Thread(target=self.server.shutdown, daemon=True).start()


def serve_annotator(root: str | Path, *, open_browser: bool = True) -> str:
    data_root = Path(root).resolve()
    build_annotation_state(data_root)
    token = secrets.token_urlsafe(32)
    server = _AnnotationServer(data_root, token)
    url = f"http://127.0.0.1:{server.server_port}/#{quote(token)}"
    print(f"Question annotation is available at {url}")
    print("It is bound to this computer only. Close the page or press Ctrl+C when finished.")
    if open_browser:
        webbrowser.open(url)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return url
