"""Tests verifying bash guard module extraction preserves behavior."""

from __future__ import annotations

from checks.bash_guard import check_bash_command as direct_check
from checks.bash_guard import run_pre_tool_use_bash
from checks.runner import check_bash_command as reexported_check


class TestBashGuardModuleImports:
    def test_direct_import_works(self):
        assert direct_check("git status", []) is None

    def test_reexport_works(self):
        assert reexported_check("git status", []) is None

    def test_same_function(self):
        assert direct_check is reexported_check

    def test_blocks_via_direct(self):
        assert direct_check("git commit --no-verify", []) is not None

    def test_run_pre_tool_use_bash_importable(self):
        assert callable(run_pre_tool_use_bash)
