"""Tests for clippy adapter."""

from __future__ import annotations

import io
import json
import subprocess
import sys
from unittest.mock import MagicMock, patch

from checks.tools.clippy_adapter import run_clippy


def _make_clippy_output(messages):
    """Build streaming JSON output (one object per line)."""
    lines = []
    for msg in messages:
        lines.append(json.dumps(msg))
    return "\n".join(lines)


class TestClippyAdapter:
    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_parses_streaming_json(self, mock_run, mock_isfile, mock_which):
        output = _make_clippy_output([
            {
                "reason": "compiler-message",
                "message": {
                    "code": {"code": "clippy::needless_return"},
                    "message": "unneeded `return` statement",
                    "spans": [
                        {"file_name": "src/main.rs", "line_start": 15}
                    ],
                },
            },
            {
                "reason": "compiler-artifact",
                "target": {"name": "test"},
            },
        ])
        mock_run.return_value = MagicMock(stdout=output, returncode=0)
        result = run_clippy("/tmp/project")
        values = list(result.values())
        assert len(values) == 1
        echoes = values[0]
        assert len(echoes) == 1
        assert echoes[0].check == "rust-clippy::needless_return"
        assert echoes[0].line == 15

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_filters_non_compiler_messages(self, mock_run, mock_isfile, mock_which):
        output = _make_clippy_output([
            {"reason": "compiler-artifact", "target": {"name": "test"}},
            {"reason": "build-script-executed"},
        ])
        mock_run.return_value = MagicMock(stdout=output, returncode=0)
        result = run_clippy("/tmp/project")
        assert result == {}

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=None)
    def test_tool_not_found(self, mock_which):
        result = run_clippy("/tmp/project")
        assert result == {}

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=False)
    def test_no_cargo_toml(self, mock_isfile, mock_which):
        result = run_clippy("/tmp/project")
        assert result == {}

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_timeout_handling(self, mock_run, mock_isfile, mock_which):
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="cargo", timeout=120)
        captured = io.StringIO()
        old = sys.stderr
        sys.stderr = captured
        try:
            result = run_clippy("/tmp/project")
        finally:
            sys.stderr = old
        assert result == {}
        assert "timed out" in captured.getvalue()

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_oserror_handling(self, mock_run, mock_isfile, mock_which):
        mock_run.side_effect = OSError("No such file")
        captured = io.StringIO()
        old = sys.stderr
        sys.stderr = captured
        try:
            result = run_clippy("/tmp/project")
        finally:
            sys.stderr = old
        assert result == {}
        assert "failed" in captured.getvalue()

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_post_filter_to_modified_files(self, mock_run, mock_isfile, mock_which):
        output = _make_clippy_output([
            {
                "reason": "compiler-message",
                "message": {
                    "code": {"code": "clippy::needless_return"},
                    "message": "unneeded return",
                    "spans": [{"file_name": "src/main.rs", "line_start": 10}],
                },
            },
            {
                "reason": "compiler-message",
                "message": {
                    "code": {"code": "clippy::needless_return"},
                    "message": "unneeded return 2",
                    "spans": [{"file_name": "src/other.rs", "line_start": 20}],
                },
            },
        ])
        mock_run.return_value = MagicMock(stdout=output, returncode=0)
        import os
        cwd = os.path.normpath("/tmp/project")
        result = run_clippy(
            cwd, modified_files=[os.path.normpath(os.path.join(cwd, "src/main.rs"))]
        )
        assert len(result) == 1

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_empty_output(self, mock_run, mock_isfile, mock_which):
        mock_run.return_value = MagicMock(stdout="", returncode=0)
        result = run_clippy("/tmp/project")
        assert result == {}

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_skips_messages_without_code(self, mock_run, mock_isfile, mock_which):
        output = _make_clippy_output([
            {
                "reason": "compiler-message",
                "message": {
                    "code": None,
                    "message": "warning without code",
                    "spans": [{"file_name": "src/main.rs", "line_start": 1}],
                },
            },
        ])
        mock_run.return_value = MagicMock(stdout=output, returncode=0)
        result = run_clippy("/tmp/project")
        assert result == {}

    @patch("checks.tools.clippy_adapter.resolve_binary_tool", return_value=["cargo"])
    @patch("os.path.isfile", return_value=True)
    @patch("subprocess.run")
    def test_multiple_issues_same_file(self, mock_run, mock_isfile, mock_which):
        output = _make_clippy_output([
            {
                "reason": "compiler-message",
                "message": {
                    "code": {"code": "clippy::needless_return"},
                    "message": "issue 1",
                    "spans": [{"file_name": "src/main.rs", "line_start": 10}],
                },
            },
            {
                "reason": "compiler-message",
                "message": {
                    "code": {"code": "clippy::unused_variable"},
                    "message": "issue 2",
                    "spans": [{"file_name": "src/main.rs", "line_start": 20}],
                },
            },
        ])
        mock_run.return_value = MagicMock(stdout=output, returncode=0)
        result = run_clippy("/tmp/project")
        values = list(result.values())
        assert len(values) == 1
        assert len(values[0]) == 2
