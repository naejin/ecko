"""Shared file classification utilities."""

from __future__ import annotations

import os


def is_test_file(file_path: str) -> bool:
    """Check if a file is a Python test file (by filename convention).

    Matches: test_*.py, *_test.py, conftest.py, conftest.pyi
    """
    name = os.path.basename(file_path)
    return (
        name.startswith("test_")
        or name.endswith("_test.py")
        or name in ("conftest.py", "conftest.pyi")
    )
