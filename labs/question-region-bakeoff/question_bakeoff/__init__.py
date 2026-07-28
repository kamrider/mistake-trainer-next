"""Offline question-region benchmark lab.

This package is intentionally isolated from the Tauri application. It reads only
explicit benchmark manifests and never opens the product database or asset store.
"""

from .schema import ENGINE_CONTRACT_VERSION

__all__ = ["ENGINE_CONTRACT_VERSION"]
