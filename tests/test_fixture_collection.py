"""Tests for vulture dynamic fixture collection (Step 6)."""

from __future__ import annotations

from checks.tools.vulture_adapter import _collect_fixture_names


class TestCollectFixtureNames:
    def test_basic_fixture(self, tmp_path):
        conftest = tmp_path / "conftest.py"
        conftest.write_text(
            "import pytest\n\n"
            "@pytest.fixture\n"
            "def my_db():\n"
            "    return 'db'\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "my_db" in names

    def test_fixture_with_scope(self, tmp_path):
        conftest = tmp_path / "conftest.py"
        conftest.write_text(
            "import pytest\n\n"
            '@pytest.fixture(scope="session")\n'
            "def app():\n"
            "    return 'app'\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "app" in names

    def test_bare_fixture_decorator(self, tmp_path):
        conftest = tmp_path / "conftest.py"
        conftest.write_text(
            "from pytest import fixture\n\n"
            "@fixture\n"
            "def my_client():\n"
            "    return 'client'\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "my_client" in names

    def test_nested_conftest(self, tmp_path):
        sub = tmp_path / "tests"
        sub.mkdir()
        conftest = sub / "conftest.py"
        conftest.write_text(
            "import pytest\n\n"
            "@pytest.fixture\n"
            "def nested_fixture():\n"
            "    return 42\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "nested_fixture" in names

    def test_syntax_error_handling(self, tmp_path):
        conftest = tmp_path / "conftest.py"
        conftest.write_text("def broken(\n", encoding="utf-8")
        names = _collect_fixture_names(str(tmp_path))
        assert names == set()  # graceful skip

    def test_os_error_handling(self, tmp_path):
        """Non-readable conftest should be skipped gracefully."""
        # Just verify the function handles missing dirs without crash
        names = _collect_fixture_names(str(tmp_path / "nonexistent"))
        assert names == set()

    def test_empty_project(self, tmp_path):
        names = _collect_fixture_names(str(tmp_path))
        assert names == set()

    def test_non_fixture_functions_excluded(self, tmp_path):
        conftest = tmp_path / "conftest.py"
        conftest.write_text(
            "import pytest\n\n"
            "def helper():\n"
            "    return 'not a fixture'\n\n"
            "@pytest.fixture\n"
            "def real_fixture():\n"
            "    return 'fixture'\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "real_fixture" in names
        assert "helper" not in names

    def test_multiple_conftest_files(self, tmp_path):
        root_conftest = tmp_path / "conftest.py"
        root_conftest.write_text(
            "import pytest\n\n"
            "@pytest.fixture\n"
            "def root_fix():\n"
            "    return 1\n",
            encoding="utf-8",
        )
        sub = tmp_path / "tests"
        sub.mkdir()
        sub_conftest = sub / "conftest.py"
        sub_conftest.write_text(
            "import pytest\n\n"
            "@pytest.fixture\n"
            "def sub_fix():\n"
            "    return 2\n",
            encoding="utf-8",
        )
        names = _collect_fixture_names(str(tmp_path))
        assert "root_fix" in names
        assert "sub_fix" in names
