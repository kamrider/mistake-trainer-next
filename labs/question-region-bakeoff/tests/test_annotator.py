import json
import tempfile
import threading
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from question_bakeoff.annotator import (
    DRAFT_FILENAME,
    MANIFEST_FILENAME,
    _AnnotationServer,
    AnnotationError,
    build_annotation_state,
    save_annotation_state,
)


def write_image(root: Path, name: str, payload: bytes = b"image") -> Path:
    path = root / "images" / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return path


class AnnotationStateTests(unittest.TestCase):
    def test_discovers_supported_images_with_stable_safe_ids(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "01 math page.PNG")
            write_image(root, "nested/01 math page.jpg")
            write_image(root, "ignore.txt")

            state = build_annotation_state(root)

            self.assertEqual(
                [sample["image"] for sample in state["samples"]],
                ["images/01 math page.PNG", "images/nested/01 math page.jpg"],
            )
            self.assertEqual(len({sample["id"] for sample in state["samples"]}), 2)
            for sample in state["samples"]:
                self.assertRegex(sample["id"], r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")
                self.assertEqual(sample["regions"], [])
                self.assertEqual(sample["anchors"], [])
            self.assertNotIn(str(root), json.dumps(state))

    def test_preserves_existing_progress_and_adds_new_images(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "first.png")
            draft = {
                "schemaVersion": 1,
                "consent": {
                    "anonymized": True,
                    "authorizedForLocalEvaluation": True,
                    "recordedAt": "2026-07-25",
                },
                "samples": [
                    {
                        "id": "kept-id",
                        "image": "images/first.png",
                        "layout": "single-column",
                        "tags": ["math"],
                        "regions": [{"x": 0.1, "y": 0.2, "width": 0.8, "height": 0.3}],
                        "anchors": [{"x": 0.12, "y": 0.21}],
                    }
                ],
            }
            (root / DRAFT_FILENAME).write_text(json.dumps(draft), encoding="utf-8")
            write_image(root, "second.jpg")

            state = build_annotation_state(root)

            first, second = state["samples"]
            self.assertEqual(first, draft["samples"][0])
            self.assertEqual(second["image"], "images/second.jpg")
            self.assertEqual(second["regions"], [])

    def test_incomplete_save_is_resumable_but_does_not_create_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "first.png")
            state = build_annotation_state(root)
            state["consent"] = {
                "anonymized": True,
                "authorizedForLocalEvaluation": True,
                "recordedAt": "2026-07-25",
            }

            result = save_annotation_state(root, state)

            self.assertFalse(result["complete"])
            self.assertEqual(result["labeledCount"], 0)
            self.assertTrue((root / DRAFT_FILENAME).is_file())
            self.assertFalse((root / MANIFEST_FILENAME).exists())

    def test_complete_authorized_save_atomically_writes_valid_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "first.png")
            state = build_annotation_state(root)
            state["consent"] = {
                "anonymized": True,
                "authorizedForLocalEvaluation": True,
                "recordedAt": "2026-07-25",
            }
            state["samples"][0]["layout"] = "single-column"
            state["samples"][0]["tags"] = ["math", "formula"]
            state["samples"][0]["regions"] = [
                {"x": 0.1, "y": 0.1, "width": 0.8, "height": 0.7}
            ]
            state["samples"][0]["anchors"] = [{"x": 0.12, "y": 0.12}]

            result = save_annotation_state(root, state)

            self.assertTrue(result["complete"])
            self.assertEqual(result["labeledCount"], 1)
            manifest = json.loads((root / MANIFEST_FILENAME).read_text(encoding="utf-8"))
            self.assertEqual(manifest, state)
            self.assertFalse(list(root.glob("*.tmp")))

    def test_rejects_unknown_paths_invalid_geometry_and_false_complete_consent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "first.png")
            state = build_annotation_state(root)
            state["samples"][0]["image"] = "../private.png"
            with self.assertRaisesRegex(AnnotationError, "image"):
                save_annotation_state(root, state)

            state = build_annotation_state(root)
            state["samples"][0]["regions"] = [
                {"x": 0.9, "y": 0.1, "width": 0.2, "height": 0.3}
            ]
            with self.assertRaisesRegex(AnnotationError, "geometry"):
                save_annotation_state(root, state)

            state = build_annotation_state(root)
            state["samples"][0]["regions"] = [
                {"x": 0.1, "y": 0.1, "width": 0.8, "height": 0.7}
            ]
            result = save_annotation_state(root, state)
            self.assertFalse(result["complete"])
            self.assertFalse((root / MANIFEST_FILENAME).exists())


class AnnotationServerTests(unittest.TestCase):
    def test_local_server_requires_token_for_state_images_and_writes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root, "first.png", b"not-a-real-image")
            server = _AnnotationServer(root, "test-secret")
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.server_close)
            self.addCleanup(server.shutdown)
            base = f"http://127.0.0.1:{server.server_port}"

            with urlopen(f"{base}/", timeout=2) as response:
                html = response.read().decode("utf-8")
                self.assertIn("真实题图标注器", html)
                self.assertIn("default-src 'none'", response.headers["Content-Security-Policy"])
                self.assertNotIn("https://", html)

            with self.assertRaises(HTTPError) as rejected:
                urlopen(f"{base}/api/state", timeout=2)
            self.assertEqual(rejected.exception.code, 401)

            request = Request(
                f"{base}/api/state",
                headers={"X-Question-Bakeoff-Token": "test-secret"},
            )
            with urlopen(request, timeout=2) as response:
                state = json.load(response)
            self.assertEqual(state["samples"][0]["image"], "images/first.png")
            self.assertNotIn(str(root), json.dumps(state))

            image_request = Request(
                f"{base}/api/image?path=images%2Ffirst.png",
                headers={"X-Question-Bakeoff-Token": "test-secret"},
            )
            with urlopen(image_request, timeout=2) as response:
                self.assertEqual(response.read(), b"not-a-real-image")

            state["consent"] = {
                "anonymized": True,
                "authorizedForLocalEvaluation": True,
                "recordedAt": "2026-07-25",
            }
            state["samples"][0]["regions"] = [
                {"x": 0.1, "y": 0.1, "width": 0.8, "height": 0.8}
            ]
            save_request = Request(
                f"{base}/api/state",
                method="PUT",
                data=json.dumps(state).encode("utf-8"),
                headers={
                    "Content-Type": "application/json",
                    "X-Question-Bakeoff-Token": "test-secret",
                },
            )
            with urlopen(save_request, timeout=2) as response:
                result = json.load(response)
            self.assertTrue(result["complete"])
            self.assertTrue((root / MANIFEST_FILENAME).is_file())


if __name__ == "__main__":
    unittest.main()
