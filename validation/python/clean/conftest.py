# Expected: exit 0
# Pytest fixtures.
import pytest


@pytest.fixture
def sample_data():
    return {"key": "value"}


@pytest.fixture
def temp_dir(tmp_path):
    d = tmp_path / "test"
    d.mkdir()
    return d
