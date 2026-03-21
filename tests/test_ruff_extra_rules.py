"""Tests for ruff_extra_rules config and adapter integration."""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

from checks.config import get_ruff_extra_rules, validate_config
from checks.tools.ruff_adapter import RUFF_RULES, run_ruff


class TestGetRuffExtraRules:
    def test_empty_config(self):
        assert get_ruff_extra_rules({}) == []

    def test_valid_full_codes(self):
        config = {"ruff_extra_rules": ["C901", "N801", "SIM101"]}
        assert get_ruff_extra_rules(config) == ["C901", "N801", "SIM101"]

    def test_valid_prefix_codes(self):
        config = {"ruff_extra_rules": ["UP", "SIM", "C4"]}
        assert get_ruff_extra_rules(config) == ["UP", "SIM", "C4"]

    def test_invalid_codes_skipped(self):
        config = {"ruff_extra_rules": ["C901", "invalid!", "123", "c901"]}
        assert get_ruff_extra_rules(config) == ["C901"]

    def test_mixed_valid_invalid(self):
        config = {"ruff_extra_rules": ["UP", "bad-code", "N801"]}
        assert get_ruff_extra_rules(config) == ["UP", "N801"]

    def test_non_list_returns_empty(self):
        config = {"ruff_extra_rules": "C901"}
        assert get_ruff_extra_rules(config) == []

    def test_empty_list(self):
        config = {"ruff_extra_rules": []}
        assert get_ruff_extra_rules(config) == []

    def test_whitespace_stripped(self):
        config = {"ruff_extra_rules": [" C901 ", "  UP  "]}
        assert get_ruff_extra_rules(config) == ["C901", "UP"]

    def test_five_letter_prefix_valid(self):
        """ASYNC is a valid ruff category with 5 uppercase letters."""
        config = {"ruff_extra_rules": ["ASYNC", "ASYNC100"]}
        assert get_ruff_extra_rules(config) == ["ASYNC", "ASYNC100"]


class TestRuffExtraRulesValidation:
    def test_known_key_no_unknown_warning(self):
        config = {"ruff_extra_rules": ["C901"]}
        warnings = validate_config(config)
        assert not any("unknown" in w for w in warnings)

    def test_invalid_code_warns(self):
        config = {"ruff_extra_rules": ["invalid!"]}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "ruff_extra_rules[0]" in warnings[0]

    def test_valid_codes_no_warning(self):
        config = {"ruff_extra_rules": ["C901", "UP", "SIM101"]}
        assert validate_config(config) == []

    def test_multiple_invalid_codes(self):
        config = {"ruff_extra_rules": ["good", "C901", "bad!"]}
        warnings = validate_config(config)
        # "good" is lowercase so invalid, "bad!" has special char
        assert len(warnings) == 2


class TestRuffAdapterExtraRules:
    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_extra_rules_appended_to_select(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", extra_rules=["C901", "N801"])
        args = mock_run.call_args[0][0]
        select_idx = args.index("--select")
        select_val = args[select_idx + 1]
        assert select_val == RUFF_RULES + ",C901,N801"

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_no_extra_rules_unchanged(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py")
        args = mock_run.call_args[0][0]
        select_idx = args.index("--select")
        assert args[select_idx + 1] == RUFF_RULES

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_empty_extra_rules_same_as_none(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", extra_rules=[])
        args = mock_run.call_args[0][0]
        select_idx = args.index("--select")
        assert args[select_idx + 1] == RUFF_RULES

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_unmapped_code_lowercased(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps([{
                "code": "C901",
                "location": {"row": 10, "column": 1},
                "message": "Function is too complex (15 > 10)",
            }]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py", extra_rules=["C901"])
        assert len(echoes) == 1
        assert echoes[0].check == "c901"

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_prefix_rules_appended(self, mock_run, mock_resolve):
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", extra_rules=["UP", "SIM"])
        args = mock_run.call_args[0][0]
        select_idx = args.index("--select")
        assert args[select_idx + 1] == RUFF_RULES + ",UP,SIM"


class TestS110Removal:
    """S110 (empty-error-handlers / try-except-pass) removed from built-in rules in v0.9.1."""

    def test_s110_not_in_builtin_rules(self):
        assert "S110" not in RUFF_RULES

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_s110_re_enabled_via_extra_rules(self, mock_run, mock_resolve):
        """Users can re-enable S110 via ruff_extra_rules."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(
            stdout=json.dumps([{
                "code": "S110",
                "location": {"row": 5, "column": 1},
                "message": "try-except-pass detected",
            }]),
            returncode=1,
        )
        echoes = run_ruff("/tmp/test.py", extra_rules=["S110"])
        assert len(echoes) == 1
        assert echoes[0].check == "s110"  # unmapped, lowercased

    @patch("checks.tools.ruff_adapter.resolve_python_tool")
    @patch("subprocess.run")
    def test_s110_in_select_when_extra(self, mock_run, mock_resolve):
        """S110 appears in --select when added via extra_rules."""
        mock_resolve.return_value = ["ruff"]
        mock_run.return_value = MagicMock(stdout="[]", returncode=0)
        run_ruff("/tmp/test.py", extra_rules=["S110"])
        args = mock_run.call_args[0][0]
        select_idx = args.index("--select")
        assert "S110" in args[select_idx + 1]


class TestRuffExtraRulesYamlParsing:
    def test_yaml_parses_string_list(self):
        from checks.config import _parse_yaml_subset

        yaml_text = "ruff_extra_rules:\n  - C901\n  - N801\n  - UP"
        config = _parse_yaml_subset(yaml_text)
        assert config["ruff_extra_rules"] == ["C901", "N801", "UP"]
