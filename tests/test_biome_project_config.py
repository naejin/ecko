"""Tests for biome_use_project_config feature."""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

from checks.config import get_biome_use_project_config
from checks.tools.biome_adapter import _to_kebab, run_biome


class TestToKebab:
    def test_camel_case(self):
        assert _to_kebab("noUnusedImports") == "no-unused-imports"

    def test_short_camel(self):
        assert _to_kebab("noVar") == "no-var"

    def test_already_kebab(self):
        assert _to_kebab("no-var") == "no-var"

    def test_single_word(self):
        assert _to_kebab("debugger") == "debugger"

    def test_multi_upper(self):
        assert _to_kebab("noDoubleEquals") == "no-double-equals"

    def test_empty(self):
        assert _to_kebab("") == ""


class TestConfigGetter:
    def test_default_false(self):
        assert get_biome_use_project_config({}) is False

    def test_explicit_true(self):
        assert get_biome_use_project_config({"biome_use_project_config": True}) is True

    def test_explicit_false(self):
        assert get_biome_use_project_config({"biome_use_project_config": False}) is False

    def test_non_bool_ignored(self):
        assert get_biome_use_project_config({"biome_use_project_config": "yes"}) is False


class TestBiomeProjectConfig:
    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_default_has_config_path(self, mock_run, mock_resolve):
        """Without use_project_config, --config-path should be present."""
        mock_resolve.return_value = ["biome"]
        mock_run.return_value = MagicMock(stdout="{}", returncode=0)
        run_biome("/tmp/test.ts", "/path/to/ecko", use_project_config=False)
        call_args = mock_run.call_args[0][0]
        assert "--config-path" in call_args

    @patch("checks.tools.biome_adapter._find_project_biome_config")
    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_project_config_omits_config_path(self, mock_run, mock_resolve, mock_find):
        """When use_project_config=True and biome.json exists, --config-path should be absent."""
        mock_resolve.return_value = ["biome"]
        mock_find.return_value = "/tmp/biome.json"
        mock_run.return_value = MagicMock(stdout="{}", returncode=0)
        run_biome("/tmp/test.ts", "/path/to/ecko", use_project_config=True)
        call_args = mock_run.call_args[0][0]
        assert "--config-path" not in call_args

    @patch("checks.tools.biome_adapter._find_project_biome_config")
    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_project_config_fallback_when_no_config(
        self, mock_run, mock_resolve, mock_find
    ):
        """When use_project_config=True but no biome.json, fall back to ecko config."""
        mock_resolve.return_value = ["biome"]
        mock_find.return_value = None
        mock_run.return_value = MagicMock(stdout="{}", returncode=0)
        import io
        import sys

        captured = io.StringIO()
        old = sys.stderr
        sys.stderr = captured
        try:
            run_biome("/tmp/test.ts", "/path/to/ecko", use_project_config=True)
        finally:
            sys.stderr = old
        call_args = mock_run.call_args[0][0]
        assert "--config-path" in call_args
        assert "no biome.json" in captured.getvalue()

    @patch("checks.tools.biome_adapter._find_project_biome_config")
    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_unknown_rule_gets_kebab_name_with_project_config(
        self, mock_run, mock_resolve, mock_find
    ):
        """Unknown biome rules get kebab-case check names only with project config."""
        mock_resolve.return_value = ["biome"]
        mock_find.return_value = "/tmp/biome.json"
        mock_run.return_value = MagicMock(
            stdout=json.dumps(
                {
                    "diagnostics": [
                        {
                            "category": "lint/style/noDoubleEquals",
                            "description": "Use === instead of ==",
                            "location": {"start": {"line": 5}},
                        }
                    ]
                }
            ),
            returncode=1,
        )
        echoes = run_biome("/tmp/test.ts", "/path/to/ecko", use_project_config=True)
        assert len(echoes) == 1
        assert echoes[0].check == "no-double-equals"
        assert echoes[0].line == 5

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_unknown_rule_skipped_without_project_config(self, mock_run, mock_resolve):
        """Unknown biome rules are skipped when using ecko's config (default)."""
        mock_resolve.return_value = ["biome"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps(
                {
                    "diagnostics": [
                        {
                            "category": "lint/style/noDoubleEquals",
                            "description": "Use === instead of ==",
                            "location": {"start": {"line": 5}},
                        }
                    ]
                }
            ),
            returncode=1,
        )
        echoes = run_biome("/tmp/test.ts", "/path/to/ecko", use_project_config=False)
        assert echoes == []

    @patch("checks.tools.biome_adapter.resolve_node_tool")
    @patch("subprocess.run")
    def test_known_rule_uses_rule_map(self, mock_run, mock_resolve):
        """Known rules should still use RULE_MAP names."""
        mock_resolve.return_value = ["biome"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps(
                {
                    "diagnostics": [
                        {
                            "category": "lint/correctness/noUnusedImports",
                            "description": "This import is unused",
                            "location": {"start": {"line": 1}},
                        }
                    ]
                }
            ),
            returncode=1,
        )
        echoes = run_biome("/tmp/test.ts", "/path/to/ecko")
        assert len(echoes) == 1
        assert echoes[0].check == "unused-imports"


class TestFindProjectBiomeConfig:
    def test_finds_biome_json(self, tmp_path):
        """Should find biome.json in the directory."""
        biome_config = tmp_path / "biome.json"
        biome_config.write_text("{}")
        from checks.tools.biome_adapter import _find_project_biome_config

        result = _find_project_biome_config(str(tmp_path))
        assert result == str(biome_config)

    def test_finds_biome_jsonc(self, tmp_path):
        """Should find biome.jsonc in the directory."""
        biome_config = tmp_path / "biome.jsonc"
        biome_config.write_text("{}")
        from checks.tools.biome_adapter import _find_project_biome_config

        result = _find_project_biome_config(str(tmp_path))
        assert result == str(biome_config)

    def test_none_when_missing(self, tmp_path):
        """Should return None when no biome config exists."""
        from checks.tools.biome_adapter import _find_project_biome_config

        result = _find_project_biome_config(str(tmp_path))
        assert result is None
