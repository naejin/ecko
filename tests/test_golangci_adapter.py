"""Tests for golangci-lint adapter."""

from __future__ import annotations

import io
import json
import subprocess
import sys
from unittest.mock import MagicMock, patch

from checks.tools.golangci_adapter import run_golangci


class TestGolangciAdapter:
    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_parses_json_issues(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(
            stdout=json.dumps({
                "Issues": [
                    {
                        "FromLinter": "errcheck",
                        "Text": "Error return value not checked",
                        "Pos": {"Filename": "main.go", "Line": 10, "Column": 5},
                    }
                ]
            }),
            returncode=1,
        )
        result = run_golangci("/tmp/project")
        # Find the file — key is absolute path
        values = list(result.values())
        assert len(values) == 1
        echoes = values[0]
        assert len(echoes) == 1
        assert echoes[0].check == "go-errcheck"
        assert echoes[0].line == 10
        assert echoes[0].message == "Error return value not checked"

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_extracts_linter_name(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(
            stdout=json.dumps({
                "Issues": [
                    {
                        "FromLinter": "staticcheck",
                        "Text": "SA1000: problem",
                        "Pos": {"Filename": "cmd/server.go", "Line": 25},
                    }
                ]
            }),
            returncode=1,
        )
        result = run_golangci("/tmp/project")
        values = list(result.values())
        assert values[0][0].check == "go-staticcheck"

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_empty_issues(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(
            stdout=json.dumps({"Issues": []}),
            returncode=0,
        )
        result = run_golangci("/tmp/project")
        assert result == {}

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_null_issues(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(
            stdout=json.dumps({"Issues": None}),
            returncode=0,
        )
        result = run_golangci("/tmp/project")
        assert result == {}

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=None)
    def test_tool_not_found(self, mock_which):
        result = run_golangci("/tmp/project")
        assert result == {}

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_timeout_handling(self, mock_run, mock_which):
        mock_run.side_effect = subprocess.TimeoutExpired(
            cmd="golangci-lint", timeout=120
        )
        captured = io.StringIO()
        old = sys.stderr
        sys.stderr = captured
        try:
            result = run_golangci("/tmp/project")
        finally:
            sys.stderr = old
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_oserror_handling(self, mock_run, mock_which):
        mock_run.side_effect = OSError("No such file")
        captured = io.StringIO()
        old = sys.stderr
        sys.stderr = captured
        try:
            result = run_golangci("/tmp/project")
        finally:
            sys.stderr = old
        assert result == {}
        assert "failed" in captured.getvalue()

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_post_filter_to_modified_files(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(
            stdout=json.dumps({
                "Issues": [
                    {
                        "FromLinter": "errcheck",
                        "Text": "Error not checked",
                        "Pos": {"Filename": "main.go", "Line": 10},
                    },
                    {
                        "FromLinter": "errcheck",
                        "Text": "Error not checked 2",
                        "Pos": {"Filename": "other.go", "Line": 20},
                    },
                ]
            }),
            returncode=1,
        )
        import os
        cwd = os.path.normpath("/tmp/project")
        result = run_golangci(
            cwd, modified_files=[os.path.join(cwd, "main.go")]
        )
        # Only main.go should be in results
        assert len(result) == 1

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_empty_output(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(stdout="", returncode=0)
        result = run_golangci("/tmp/project")
        assert result == {}

    @patch("checks.tools.golangci_adapter.resolve_binary_tool", return_value=["golangci-lint"])
    @patch("subprocess.run")
    def test_invalid_json(self, mock_run, mock_which):
        mock_run.return_value = MagicMock(stdout="not json", returncode=1)
        result = run_golangci("/tmp/project")
        assert result == {}


class TestLangMap:
    def test_go_language_detection(self):
        from checks.runner import detect_language

        assert detect_language("main.go") == "go"

    def test_rust_language_detection(self):
        from checks.runner import detect_language

        assert detect_language("lib.rs") == "rust"
