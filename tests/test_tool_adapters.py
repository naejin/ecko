"""Unit tests for tool adapter output parsing.

Each adapter's parsing logic is tested with realistic fixture data,
without needing the actual tool installed.
"""

from __future__ import annotations

import io
import json
import subprocess
import sys
from contextlib import contextmanager
from unittest.mock import MagicMock, patch

from checks.tools.biome_adapter import run_biome
from checks.tools.knip_adapter import run_knip
from checks.tools.pyright_adapter import run_pyright
from checks.tools.ruff_adapter import run_ruff
from checks.tools.tsc_adapter import TSC_PATTERN, run_tsc
from checks.tools.vulture_adapter import VULTURE_PATTERN, run_vulture


@contextmanager
def capture_stderr():
    """Context manager that captures stderr output (used by emit())."""
    captured = io.StringIO()
    old = sys.stderr
    sys.stderr = captured
    try:
        yield captured
    finally:
        sys.stderr = old


# ---------------------------------------------------------------------------
# Ruff adapter
# ---------------------------------------------------------------------------


class TestRuffAdapterParsing:
    """Test ruff JSON output parsing."""

    def _make_ruff_output(self, violations: list[dict]) -> str:
        return json.dumps(violations)

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_parses_unused_import(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=self._make_ruff_output([
                {
                    "code": "F401",
                    "location": {"row": 3, "column": 1},
                    "message": "`os` imported but unused",
                }
            ]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py")
        assert len(echoes) == 1
        assert echoes[0].check == "unused-imports"
        assert echoes[0].line == 3
        assert "os" in echoes[0].message

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_allowlist_filters_shadow(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=self._make_ruff_output([
                {
                    "code": "A002",
                    "location": {"row": 5, "column": 1},
                    "message": "Argument `type` is shadowing a Python builtin",
                },
                {
                    "code": "A002",
                    "location": {"row": 10, "column": 1},
                    "message": "Argument `data` is shadowing a Python builtin",
                },
            ]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py", builtin_shadow_allowlist=frozenset({"type"}))
        assert len(echoes) == 1
        assert echoes[0].line == 10

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="ruff", timeout=30)
        with capture_stderr() as captured:
            echoes = run_ruff("/tmp/test.py")
        assert echoes == []
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_crash_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.side_effect = OSError("No such file")
        with capture_stderr() as captured:
            echoes = run_ruff("/tmp/test.py")
        assert echoes == []
        assert "failed" in captured.getvalue()

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_empty_output(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="", returncode=0)
        assert run_ruff("/tmp/test.py") == []

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_invalid_json(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="not json", returncode=1)
        assert run_ruff("/tmp/test.py") == []

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    def test_unresolved_returns_empty(self, mock_resolve):
        mock_resolve.return_value = None
        assert run_ruff("/tmp/test.py") == []


# ---------------------------------------------------------------------------
# Biome adapter
# ---------------------------------------------------------------------------


class TestBiomeAdapterParsing:
    """Test biome JSON output parsing."""

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_parses_v2_format(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["biome"]
        data = {
            "diagnostics": [
                {
                    "category": "lint/correctness/noUnusedImports",
                    "description": "This import is unused.",
                    "location": {
                        "start": {"line": 2, "column": 0},
                    },
                }
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=1)
        echoes = run_biome("/tmp/test.ts", "/plugin")
        assert len(echoes) == 1
        assert echoes[0].check == "unused-imports"
        assert echoes[0].line == 2

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_v1_span_format(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["biome"]
        source = "import os\nlet x = 1;\nlet y = 2;\n"
        data = {
            "diagnostics": [
                {
                    "category": "lint/style/noVar",
                    "description": "Use let or const instead of var.",
                    "location": {
                        "span": [11, 22],
                        "sourceCode": source,
                    },
                }
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=1)
        echoes = run_biome("/tmp/test.ts", "/plugin")
        assert len(echoes) == 1
        assert echoes[0].check == "var-declarations"
        assert echoes[0].line == 2  # offset 11 is on line 2

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_unknown_rule_skipped(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["biome"]
        data = {
            "diagnostics": [
                {
                    "category": "lint/unknown/unknownRule",
                    "description": "something",
                    "location": {"start": {"line": 1, "column": 0}},
                }
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=1)
        echoes = run_biome("/tmp/test.ts", "/plugin")
        assert echoes == []

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["biome"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="biome", timeout=30)
        with capture_stderr() as captured:
            echoes = run_biome("/tmp/test.ts", "/plugin")
        assert echoes == []
        assert "timed out" in captured.getvalue()


# ---------------------------------------------------------------------------
# Pyright adapter
# ---------------------------------------------------------------------------


class TestPyrightAdapterParsing:
    """Test pyright JSON output parsing."""

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_parses_type_error(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["pyright"]
        data = {
            "generalDiagnostics": [
                {
                    "severity": "error",
                    "file": "/tmp/test.py",
                    "message": "Cannot assign type 'str' to 'int'",
                    "range": {"start": {"line": 9, "character": 0}},
                }
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=1)
        result = run_pyright(["/tmp/test.py"], "/tmp")
        assert "/tmp/test.py" in result
        assert len(result["/tmp/test.py"]) == 1
        assert result["/tmp/test.py"][0].line == 10  # 0-indexed → 1-indexed
        assert result["/tmp/test.py"][0].check == "type-error"

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_filters_unresolved_imports(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["pyright"]
        data = {
            "generalDiagnostics": [
                {
                    "severity": "error",
                    "file": "/tmp/test.py",
                    "message": "Import \"flask\" could not be resolved",
                    "range": {"start": {"line": 0, "character": 0}},
                },
                {
                    "severity": "error",
                    "file": "/tmp/test.py",
                    "message": "Cannot assign type 'str' to 'int'",
                    "range": {"start": {"line": 5, "character": 0}},
                },
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=1)
        result = run_pyright(["/tmp/test.py"], "/tmp")
        assert len(result["/tmp/test.py"]) == 1
        assert "assign" in result["/tmp/test.py"][0].message

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_skips_warnings(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["pyright"]
        data = {
            "generalDiagnostics": [
                {
                    "severity": "warning",
                    "file": "/tmp/test.py",
                    "message": "Some warning",
                    "range": {"start": {"line": 0, "character": 0}},
                }
            ]
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=0)
        result = run_pyright(["/tmp/test.py"], "/tmp")
        assert result == {}

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["pyright"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="pyright", timeout=60)
        with capture_stderr() as captured:
            result = run_pyright(["/tmp/test.py"], "/tmp")
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.pyright_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_crash_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["pyright"]
        mock_run.side_effect = OSError("No such file")
        with capture_stderr() as captured:
            result = run_pyright(["/tmp/test.py"], "/tmp")
        assert result == {}
        assert "failed" in captured.getvalue()


# ---------------------------------------------------------------------------
# tsc adapter
# ---------------------------------------------------------------------------


class TestTscAdapterParsing:
    """Test tsc regex-based output parsing."""

    def test_pattern_matches_standard_error(self):
        line = "src/index.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'."
        match = TSC_PATTERN.match(line)
        assert match is not None
        assert match.group(1) == "src/index.ts"
        assert match.group(2) == "10"
        assert "Type 'string'" in match.group(4)

    def test_pattern_matches_windows_path(self):
        line = r"C:\Users\dev\src\index.ts(3,1): error TS1005: ';' expected."
        match = TSC_PATTERN.match(line)
        assert match is not None
        assert match.group(2) == "3"

    @patch("checks.tools.tsc_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_parses_multiple_errors(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["tsc"]
        output = (
            "src/a.ts(1,1): error TS2322: Bad type\n"
            "src/b.ts(5,3): error TS1005: Missing semi\n"
        )
        mock_run.return_value = MagicMock(stdout=output, stderr="", returncode=1)
        result = run_tsc("/tmp")
        assert "src/a.ts" in result
        assert "src/b.ts" in result
        assert result["src/a.ts"][0].line == 1
        assert result["src/b.ts"][0].line == 5

    @patch("checks.tools.tsc_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["tsc"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="tsc", timeout=60)
        with capture_stderr() as captured:
            result = run_tsc("/tmp")
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.tsc_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_crash_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["tsc"]
        mock_run.side_effect = OSError("No such file")
        with capture_stderr() as captured:
            result = run_tsc("/tmp")
        assert result == {}
        assert "failed" in captured.getvalue()


# ---------------------------------------------------------------------------
# Knip adapter
# ---------------------------------------------------------------------------


class TestKnipAdapterParsing:
    """Test knip JSON output parsing."""

    @patch("checks.tools.knip_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_parses_unused_files(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["knip"]
        data = {"files": ["src/unused.ts"], "exports": [], "types": []}
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=0)
        result = run_knip("/tmp")
        assert "src/unused.ts" in result
        assert result["src/unused.ts"][0].check == "unused-file"

    @patch("checks.tools.knip_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_parses_unused_exports(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["knip"]
        data = {
            "files": [],
            "exports": [
                {"file": "src/utils.ts", "name": "helper", "line": 10}
            ],
            "types": [],
        }
        mock_run.return_value = MagicMock(stdout=json.dumps(data), returncode=0)
        result = run_knip("/tmp")
        assert "src/utils.ts" in result
        assert result["src/utils.ts"][0].check == "unused-export"
        assert result["src/utils.ts"][0].line == 10

    @patch("checks.tools.knip_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_empty_json(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["knip"]
        mock_run.return_value = MagicMock(stdout="{}", returncode=0)
        assert run_knip("/tmp") == {}

    @patch("checks.tools.knip_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["knip"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="knip", timeout=60)
        with capture_stderr() as captured:
            result = run_knip("/tmp")
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.knip_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_crash_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["knip"]
        mock_run.side_effect = OSError("No such file")
        with capture_stderr() as captured:
            result = run_knip("/tmp")
        assert result == {}
        assert "failed" in captured.getvalue()


# ---------------------------------------------------------------------------
# Vulture adapter
# ---------------------------------------------------------------------------


class TestVultureAdapterParsing:
    """Test vulture regex-based output parsing."""

    def test_pattern_matches_standard_output(self):
        line = "src/app.py:42: unused function 'helper' (80% confidence)"
        match = VULTURE_PATTERN.match(line)
        assert match is not None
        assert match.group(1) == "src/app.py"
        assert match.group(2) == "42"
        assert "unused function" in match.group(3)

    def test_pattern_matches_various_formats(self):
        lines = [
            "app.py:1: unused variable 'x' (90% confidence)",
            "tests/test_app.py:10: unused argument 'tmp_path' (80% confidence)",
            "mod.py:5: unreachable code after 'return' (100% confidence)",
        ]
        for line in lines:
            assert VULTURE_PATTERN.match(line) is not None

    @patch("checks.tools.vulture_adapter.resolve_python_tool")
    @patch("subprocess.run")
    @patch("checks.tools.vulture_adapter._collect_fixture_names")
    def test_filters_always_skip(self, mock_fixtures, mock_run, mock_resolve):
        mock_resolve.return_value = ["vulture"]
        mock_fixtures.return_value = set()
        output = (
            "app.py:1: unused argument 'exc_type' (80% confidence)\n"
            "app.py:5: unused variable 'real_var' (80% confidence)\n"
        )
        mock_run.return_value = MagicMock(stdout=output, returncode=1)
        result = run_vulture("/tmp")
        assert "app.py" in result
        echoes = result["app.py"]
        assert len(echoes) == 1
        assert "real_var" in echoes[0].message

    @patch("checks.tools.vulture_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_timeout_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["vulture"]
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="vulture", timeout=60)
        with capture_stderr() as captured:
            result = run_vulture("/tmp")
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.vulture_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_crash_emits_warning(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["vulture"]
        mock_run.side_effect = OSError("No such file")
        with capture_stderr() as captured:
            result = run_vulture("/tmp")
        assert result == {}
        assert "failed" in captured.getvalue()
