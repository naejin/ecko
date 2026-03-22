# Expected: exit 0
# TYPE_CHECKING imports used only in annotations.
from __future__ import annotations
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pathlib import Path


def load_config(path: Path) -> dict:
    return {}
