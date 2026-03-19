"""Shared fixtures for ecko tests."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

FIXTURES_DIR = Path(__file__).parent / "fixtures"


@pytest.fixture
def fixtures_dir() -> Path:
    return FIXTURES_DIR


@pytest.fixture
def tmp_copy(tmp_path: Path):
    """Copy a fixture file to a temp directory so tests can modify it."""

    def _copy(fixture_name: str) -> str:
        src = FIXTURES_DIR / fixture_name
        dst = tmp_path / fixture_name
        shutil.copy2(src, dst)
        return str(dst)

    return _copy


@pytest.fixture
def plugin_root() -> str:
    return str(Path(__file__).parent.parent)
