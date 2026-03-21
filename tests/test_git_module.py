"""Tests verifying git module extraction and session_hours bug fix."""

from __future__ import annotations

import os
from unittest.mock import MagicMock, patch

from checks.git import get_modified_files, normalize_path
from checks.runner import _get_modified_files, _normalize_path


class TestGitModuleImports:
    def test_normalize_path_relative(self):
        result = normalize_path("src/app.py", "/home/user/project")
        assert result == os.path.normpath("/home/user/project/src/app.py")

    def test_normalize_path_absolute(self):
        result = normalize_path("/abs/path.py", "/home/user/project")
        assert result == os.path.normpath("/abs/path.py")

    def test_reexport_normalize_path(self):
        assert _normalize_path is normalize_path

    def test_reexport_get_modified_files(self):
        assert _get_modified_files is get_modified_files

    def test_get_modified_files_accepts_session_hours(self, tmp_path):
        """get_modified_files accepts session_hours parameter without error."""
        result = get_modified_files(str(tmp_path), session_hours=2.0)
        assert result == []


class TestSessionHoursBugFix:
    @patch("checks.git.subprocess.run")
    def test_uses_session_hours_in_git_log(self, mock_run):
        mock_run.return_value = MagicMock(returncode=0, stdout="")
        get_modified_files("/tmp/fake", session_hours=6.0)
        # Find the git log call (4th call)
        git_log_calls = [
            c for c in mock_run.call_args_list if "log" in str(c)
        ]
        assert len(git_log_calls) == 1
        args = git_log_calls[0][0][0]
        assert "--since=360m" in args  # 6 * 60 = 360 minutes

    @patch("checks.git.subprocess.run")
    def test_default_session_hours_is_4h(self, mock_run):
        mock_run.return_value = MagicMock(returncode=0, stdout="")
        get_modified_files("/tmp/fake")
        git_log_calls = [
            c for c in mock_run.call_args_list if "log" in str(c)
        ]
        args = git_log_calls[0][0][0]
        assert "--since=240m" in args  # 4 * 60 = 240 minutes

    @patch("checks.git.subprocess.run")
    def test_fractional_session_hours(self, mock_run):
        mock_run.return_value = MagicMock(returncode=0, stdout="")
        get_modified_files("/tmp/fake", session_hours=1.5)
        git_log_calls = [
            c for c in mock_run.call_args_list if "log" in str(c)
        ]
        args = git_log_calls[0][0][0]
        assert "--since=90m" in args  # 1.5 * 60 = 90 minutes
