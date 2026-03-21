"""Tests for ruff_use_project_config feature."""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

from checks.config import get_ruff_use_project_config
from checks.tools.ruff_adapter import run_ruff


class TestConfigGetter:
    def test_default_false(self):
        assert get_ruff_use_project_config({}) is False

    def test_explicit_true(self):
        assert get_ruff_use_project_config({"ruff_use_project_config": True}) is True

    def test_explicit_false(self):
        assert get_ruff_use_project_config({"ruff_use_project_config": False}) is False

    def test_non_bool_ignored(self):
        assert get_ruff_use_project_config({"ruff_use_project_config": "yes"}) is False

    def test_non_bool_int_ignored(self):
        assert get_ruff_use_project_config({"ruff_use_project_config": 1}) is False


class TestRuffProjectConfig:
    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_project_config_omits_select(self, mock_run, mock_resolve):
        """When use_project_config=True, --select should not appear in the command."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", use_project_config=True)
        call_args = mock_run.call_args[0][0]
        assert "--select" not in call_args

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_project_config_always_has_no_fix(self, mock_run, mock_resolve):
        """--no-fix must always be present when using project config."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", use_project_config=True)
        call_args = mock_run.call_args[0][0]
        assert "--no-fix" in call_args

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_default_mode_has_select(self, mock_run, mock_resolve):
        """Without use_project_config, --select should be present (regression)."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", use_project_config=False)
        call_args = mock_run.call_args[0][0]
        assert "--select" in call_args

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_default_mode_always_has_no_fix(self, mock_run, mock_resolve):
        """--no-fix must always be present in default mode too."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", use_project_config=False)
        call_args = mock_run.call_args[0][0]
        assert "--no-fix" in call_args

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_project_config_parses_echoes(self, mock_run, mock_resolve):
        """Project config mode should still parse ruff JSON output into echoes."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps(
                [
                    {
                        "code": "F401",
                        "location": {"row": 3, "column": 1},
                        "message": "`os` imported but unused",
                    }
                ]
            ),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py", use_project_config=True)
        assert len(echoes) == 1
        assert echoes[0].check == "unused-imports"
        assert echoes[0].line == 3

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_project_config_unmapped_code_lowercased(self, mock_run, mock_resolve):
        """Unmapped ruff codes should use lowercased code as check name."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps(
                [
                    {
                        "code": "C901",
                        "location": {"row": 10, "column": 1},
                        "message": "Function is too complex",
                    }
                ]
            ),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py", use_project_config=True)
        assert len(echoes) == 1
        assert echoes[0].check == "c901"

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_project_config_has_output_format_json(self, mock_run, mock_resolve):
        """--output-format json must be present in project config mode."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", use_project_config=True)
        call_args = mock_run.call_args[0][0]
        assert "--output-format" in call_args
        idx = call_args.index("--output-format")
        assert call_args[idx + 1] == "json"
