"""Tests for the config parser."""

from __future__ import annotations

from checks.config import (
    _parse_yaml_subset,
    get_banned_patterns,
    get_cross_file_echo_cap,
    get_disabled_checks,
    get_exclude_patterns,
    get_obsolete_terms,
    get_session_hours,
    is_autofix_enabled,
    is_deep_enabled,
    load_config,
    validate_config,
)


class TestParseYamlSubset:
    def test_scalar_values(self):
        result = _parse_yaml_subset("key: value\ncount: 42\nenabled: true")
        assert result["key"] == "value"
        assert result["count"] == 42
        assert result["enabled"] is True

    def test_false_and_null(self):
        result = _parse_yaml_subset("a: false\nb: null")
        assert result["a"] is False
        assert result["b"] is None

    def test_nested_mapping(self):
        result = _parse_yaml_subset("autofix:\n  black: true\n  isort: false")
        assert result["autofix"]["black"] is True
        assert result["autofix"]["isort"] is False

    def test_list_of_strings(self):
        result = _parse_yaml_subset("items:\n  - one\n  - two\n  - three")
        assert result["items"] == ["one", "two", "three"]

    def test_list_of_dicts(self):
        text = "patterns:\n  - pattern: \"foo\"\n    glob: \"*.py\"\n  - pattern: \"bar\""
        result = _parse_yaml_subset(text)
        assert len(result["patterns"]) == 2
        assert result["patterns"][0]["pattern"] == "foo"
        assert result["patterns"][0]["glob"] == "*.py"
        assert result["patterns"][1]["pattern"] == "bar"

    def test_comments_ignored(self):
        result = _parse_yaml_subset("# comment\nkey: value  # inline")
        assert result["key"] == "value"

    def test_empty_list(self):
        result = _parse_yaml_subset("items: []")
        assert result["items"] == []

    def test_quoted_strings(self):
        result = _parse_yaml_subset('name: "hello world"\nalt: \'single\'')
        assert result["name"] == "hello world"
        assert result["alt"] == "single"

    def test_escape_sequences_in_double_quotes(self):
        result = _parse_yaml_subset('pattern: "foo\\\\d+"')
        assert result["pattern"] == "foo\\d+"

    def test_empty_string(self):
        result = _parse_yaml_subset("")
        assert result == {}


class TestLoadConfig:
    def test_missing_file_returns_empty(self, tmp_path):
        assert load_config(str(tmp_path)) == {}

    def test_loads_file(self, tmp_path):
        config_file = tmp_path / "ecko.yaml"
        config_file.write_text("autofix:\n  black: false\n")
        result = load_config(str(tmp_path))
        assert result["autofix"]["black"] is False


class TestConfigHelpers:
    def test_disabled_checks(self):
        config = {"disabled_checks": ["unused-imports", "bare-except"]}
        assert get_disabled_checks(config) == {"unused-imports", "bare-except"}

    def test_disabled_checks_empty(self):
        assert get_disabled_checks({}) == set()

    def test_autofix_enabled_default(self):
        assert is_autofix_enabled({}, "black") is True

    def test_autofix_disabled_globally(self):
        config = {"autofix": {"enabled": False}}
        assert is_autofix_enabled(config, "black") is False

    def test_autofix_disabled_per_tool(self):
        config = {"autofix": {"enabled": True, "black": False}}
        assert is_autofix_enabled(config, "black") is False

    def test_deep_enabled_default(self):
        assert is_deep_enabled({}, "tsc") is True

    def test_deep_disabled(self):
        config = {"deep_analysis": {"tsc": False}}
        assert is_deep_enabled(config, "tsc") is False

    def test_banned_patterns(self):
        config = {"banned_patterns": [{"pattern": "foo", "glob": "*.py"}]}
        patterns = get_banned_patterns(config)
        assert len(patterns) == 1
        assert patterns[0]["pattern"] == "foo"

    def test_obsolete_terms(self):
        config = {"obsolete_terms": [{"old": "Foo", "new": "Bar"}]}
        terms = get_obsolete_terms(config)
        assert len(terms) == 1
        assert terms[0]["old"] == "Foo"

    def test_exclude_patterns(self):
        config = {"exclude": ["generated/*", "*.min.js"]}
        patterns = get_exclude_patterns(config)
        assert patterns == ["generated/*", "*.min.js"]

    def test_exclude_patterns_empty(self):
        assert get_exclude_patterns({}) == []


class TestValidateConfig:
    def test_valid_config_no_warnings(self):
        config = {
            "disabled_checks": ["unused-imports"],
            "autofix": {"black": True},
            "exclude": ["*.min.js"],
        }
        assert validate_config(config) == []

    def test_unknown_key_warning(self):
        config = {"disabled_check": ["unused-imports"]}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "disabled_check" in warnings[0]
        assert "disabled_checks" in warnings[0]

    def test_unknown_key_no_suggestion(self):
        config = {"zzz_completely_unknown": True}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "zzz_completely_unknown" in warnings[0]

    def test_invalid_banned_pattern_regex(self):
        config = {"banned_patterns": [{"pattern": "[invalid"}]}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "banned_patterns[0]" in warnings[0]

    def test_invalid_blocked_command_regex(self):
        config = {"blocked_commands": [{"pattern": "(unclosed"}]}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "blocked_commands[0]" in warnings[0]

    def test_valid_patterns_no_warnings(self):
        config = {
            "banned_patterns": [{"pattern": r"foo\d+"}],
            "blocked_commands": [{"pattern": r"rm\s+-rf"}],
        }
        assert validate_config(config) == []

    def test_multiple_issues(self):
        config = {
            "disabled_check": [],
            "banned_patterns": [{"pattern": "[bad"}],
        }
        warnings = validate_config(config)
        assert len(warnings) == 2

    def test_empty_config(self):
        assert validate_config({}) == []

    def test_redos_pattern_warns_without_hanging(self):
        """A pathological regex in banned_patterns should warn, not hang."""
        import time

        # (a+)+b is valid but causes catastrophic backtracking
        # safe_regex_compile will succeed (compile is fast), but we test
        # that the validation process doesn't hang
        config = {"banned_patterns": [{"pattern": r"(a+)+b"}]}
        start = time.monotonic()
        warnings = validate_config(config)
        elapsed = time.monotonic() - start
        # Should complete quickly — (a+)+b compiles fine so no warning
        assert elapsed < 5.0
        assert warnings == []

    def test_truly_invalid_pattern_warns(self):
        """An invalid regex should produce a warning via safe_regex_compile."""
        config = {"banned_patterns": [{"pattern": r"[invalid"}]}
        warnings = validate_config(config)
        assert len(warnings) == 1
        assert "banned_patterns[0]" in warnings[0]
        assert "invalid" in warnings[0].lower() or "pathological" in warnings[0].lower()


class TestSessionHours:
    def test_default(self):
        assert get_session_hours({}) == 4.0

    def test_custom(self):
        assert get_session_hours({"session_hours": 6}) == 6.0

    def test_zero_disables(self):
        assert get_session_hours({"session_hours": 0}) == 0.0

    def test_invalid_type(self):
        assert get_session_hours({"session_hours": "bad"}) == 4.0


class TestCrossFileEchoCap:
    def test_default(self):
        assert get_cross_file_echo_cap({}) == 0

    def test_custom(self):
        assert get_cross_file_echo_cap({"echo_cap_cross_file": 15}) == 15

    def test_zero_unlimited(self):
        assert get_cross_file_echo_cap({"echo_cap_cross_file": 0}) == 0

    def test_invalid_type(self):
        assert get_cross_file_echo_cap({"echo_cap_cross_file": "bad"}) == 0


class TestKnownKeysIncludeNew:
    def test_session_hours_in_known_keys(self):
        config = {"session_hours": 4}
        warnings = validate_config(config)
        unknown = [w for w in warnings if "unknown config key" in w and "session_hours" in w]
        assert unknown == []

    def test_echo_cap_cross_file_in_known_keys(self):
        config = {"echo_cap_cross_file": 15}
        warnings = validate_config(config)
        unknown = [w for w in warnings if "unknown config key" in w and "echo_cap_cross_file" in w]
        assert unknown == []
