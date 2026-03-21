"""Tests for echo severity feature."""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

from checks.result import Echo, format_file_echoes, format_stop_echoes, has_errors


class TestEchoSeverity:
    def test_default_severity_is_warn(self):
        echo = Echo(check="unused-imports", line=1, message="test")
        assert echo.severity == "warn"

    def test_explicit_error(self):
        echo = Echo(check="bare-except", line=1, message="test", severity="error")
        assert echo.severity == "error"

    def test_explicit_warn(self):
        echo = Echo(check="unused-imports", line=1, message="test", severity="warn")
        assert echo.severity == "warn"


class TestHasErrors:
    def test_no_echoes(self):
        assert has_errors([]) is False

    def test_all_warn(self):
        echoes = [
            Echo(check="unused-imports", line=1, message="a"),
            Echo(check="dead-code", line=2, message="b"),
        ]
        assert has_errors(echoes) is False

    def test_one_error(self):
        echoes = [
            Echo(check="unused-imports", line=1, message="a"),
            Echo(check="bare-except", line=2, message="b", severity="error"),
        ]
        assert has_errors(echoes) is True

    def test_all_errors(self):
        echoes = [
            Echo(check="bare-except", line=1, message="a", severity="error"),
            Echo(check="type-error", line=2, message="b", severity="error"),
        ]
        assert has_errors(echoes) is True


class TestFormatWithSeverity:
    def test_error_prefix_in_file_echoes(self):
        echoes = [
            Echo(check="bare-except", line=5, message="Bare except", severity="error"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "[error] bare-except (L5)" in output

    def test_warn_no_prefix_in_file_echoes(self):
        echoes = [
            Echo(check="unused-imports", line=3, message="Unused import"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "unused-imports (L3)" in output
        assert "[error]" not in output

    def test_error_prefix_in_stop_echoes(self):
        file_echoes = {
            "test.py": [
                Echo(check="bare-except", line=5, message="Bare except", severity="error"),
            ]
        }
        output = format_stop_echoes(file_echoes)
        assert "[error] bare-except" in output

    def test_warn_no_prefix_in_stop_echoes(self):
        file_echoes = {
            "test.py": [
                Echo(check="unused-imports", line=3, message="Unused import"),
            ]
        }
        output = format_stop_echoes(file_echoes)
        assert "[error]" not in output
        assert "unused-imports" in output

    def test_mixed_severity_formatting(self):
        echoes = [
            Echo(check="bare-except", line=1, message="Bare", severity="error"),
            Echo(check="unused-imports", line=2, message="Unused"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "[error] bare-except" in output
        assert "unused-imports" in output


class TestAdapterSeverity:
    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_ruff_bare_except_is_error(self, mock_run, mock_resolve):
        from checks.tools.ruff_adapter import run_ruff

        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps([
                {"code": "E722", "location": {"row": 5}, "message": "Do not use bare `except`"},
            ]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py")
        assert echoes[0].severity == "error"

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_ruff_star_imports_is_error(self, mock_run, mock_resolve):
        from checks.tools.ruff_adapter import run_ruff

        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps([
                {"code": "F403", "location": {"row": 1}, "message": "`from x import *` used"},
            ]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py")
        assert echoes[0].severity == "error"

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_ruff_singleton_comparison_is_warn(self, mock_run, mock_resolve):
        from checks.tools.ruff_adapter import run_ruff

        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps([
                {"code": "E711", "location": {"row": 3}, "message": "Comparison to None"},
            ]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py")
        assert echoes[0].severity == "warn"

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_pyright_is_error(self, mock_run, mock_resolve):
        from checks.tools.pyright_adapter import run_pyright

        mock_resolve.return_value = ["pyright"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps({
                "generalDiagnostics": [
                    {
                        "file": "/tmp/test.py",
                        "severity": "error",
                        "message": "Type mismatch",
                        "range": {"start": {"line": 10, "character": 0}},
                    }
                ]
            }),
            returncode=1,
        )
        result = run_pyright(["/tmp/test.py"], "/tmp")
        echoes = result.get("/tmp/test.py", [])
        assert len(echoes) == 1
        assert echoes[0].severity == "error"
