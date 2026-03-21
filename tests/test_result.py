"""Tests for the Echo formatter."""

from __future__ import annotations

from checks.result import Echo, format_correction_summary, format_file_echoes, format_stop_echoes


class TestFormatFileEchoes:
    def test_empty(self):
        assert format_file_echoes("test.py", []) == ""

    def test_single_echo(self):
        echoes = [Echo(check="unused-imports", line=3, message="`os` is unused.")]
        output = format_file_echoes("test.py", echoes)
        assert "~~ ecko ~~" in output
        assert "1 echo in test.py" in output
        assert "unused-imports (line 3)" in output
        assert "`os` is unused." in output

    def test_multiple_echoes(self):
        echoes = [
            Echo(check="a", line=1, message="msg1"),
            Echo(check="b", line=2, message="msg2"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "2 echoes in test.py" in output

    def test_suggestion_included(self):
        echoes = [Echo(check="a", line=1, message="msg", suggestion="Fix it.")]
        output = format_file_echoes("test.py", echoes)
        assert "Fix it." in output


class TestFormatStopEchoes:
    def test_empty(self):
        assert format_stop_echoes({}) == ""

    def test_single_file(self):
        file_echoes = {
            "src/a.py": [Echo(check="type-error", line=10, message="bad type")]
        }
        output = format_stop_echoes(file_echoes)
        assert "~~ ecko ~~  final sweep" in output
        assert "1 echo across 1 file" in output
        assert "src/a.py:" in output

    def test_multiple_files(self):
        file_echoes = {
            "a.py": [Echo(check="a", line=1, message="m1")],
            "b.py": [Echo(check="b", line=2, message="m2")],
        }
        output = format_stop_echoes(file_echoes)
        assert "2 echoes across 2 files" in output


class TestCrossFileCap:
    def test_zero_unlimited(self):
        file_echoes = {
            "a.py": [Echo(check="x", line=1, message="m")] * 10,
            "b.py": [Echo(check="x", line=2, message="m")] * 10,
        }
        output = format_stop_echoes(file_echoes, cross_file_cap=0)
        assert "echo_cap_cross_file" not in output

    def test_cap_limits_per_check(self):
        file_echoes = {
            "a.py": [Echo(check="unused-imports", line=i, message="m") for i in range(5)],
            "b.py": [Echo(check="unused-imports", line=i, message="m") for i in range(5)],
        }
        output = format_stop_echoes(file_echoes, cross_file_cap=3)
        # Only 3 unused-imports should be shown, rest in overflow
        assert "more unused-imports" in output

    def test_different_checks_independent(self):
        file_echoes = {
            "a.py": [
                Echo(check="unused-imports", line=1, message="m"),
                Echo(check="type-error", line=2, message="m"),
            ],
            "b.py": [
                Echo(check="unused-imports", line=1, message="m"),
                Echo(check="type-error", line=2, message="m"),
            ],
        }
        output = format_stop_echoes(file_echoes, cross_file_cap=1)
        # Both checks capped at 1, so 2 overflow (1 unused-imports + 1 type-error)
        assert "more unused-imports" in output
        assert "more type-error" in output

    def test_overflow_message_includes_advice(self):
        file_echoes = {
            "a.py": [Echo(check="x", line=1, message="m")] * 5,
        }
        output = format_stop_echoes(file_echoes, cross_file_cap=2)
        assert "echo_cap_cross_file" in output

    def test_cap_with_per_file_cap(self):
        """Cross-file cap and per-file cap should coexist."""
        file_echoes = {
            "a.py": [Echo(check="x", line=i, message="m") for i in range(10)],
            "b.py": [Echo(check="x", line=i, message="m") for i in range(10)],
        }
        output = format_stop_echoes(file_echoes, echo_cap=3, cross_file_cap=5)
        # Per-file cap limits to 3 per file displayed, cross-file cap to 5 total
        assert "more x" in output


class TestFormatCorrectionSummary:
    def test_empty_returns_empty(self):
        assert format_correction_summary({}) == ""

    def test_single_check(self):
        result = format_correction_summary({"unused-imports": 3})
        assert "3 fixed" in result
        assert "3 unused-imports" in result
        assert result.startswith("~~ ecko ~~")
        assert result.endswith("\n")

    def test_multiple_checks_sorted_by_count(self):
        result = format_correction_summary({"bare-except": 1, "unused-imports": 5, "type-error": 3})
        assert "9 fixed" in result
        idx_unused = result.index("unused-imports")
        idx_type = result.index("type-error")
        idx_bare = result.index("bare-except")
        assert idx_unused < idx_type < idx_bare

    def test_no_file_paths_in_output(self):
        """Stop hook output must not contain file paths."""
        result = format_correction_summary({"unused-imports": 2})
        assert "/" not in result
        assert "\\" not in result
