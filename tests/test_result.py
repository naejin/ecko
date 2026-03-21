"""Tests for the Echo formatter."""

from __future__ import annotations

from checks.result import (
    Echo,
    format_correction_summary,
    format_file_echoes,
    format_session_stats,
    format_stop_echoes,
)


class TestFormatFileEchoes:
    def test_empty(self):
        assert format_file_echoes("test.py", []) == ""

    def test_single_echo(self):
        echoes = [Echo(check="unused-imports", line=3, message="`os` is unused.")]
        output = format_file_echoes("test.py", echoes)
        assert "~~ ecko ~~" in output
        assert "test.py" in output
        assert "unused-imports (L3)" in output

    def test_multiple_echoes_same_check(self):
        echoes = [
            Echo(check="unused-imports", line=5, message="m1"),
            Echo(check="unused-imports", line=10, message="m2"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "unused-imports (L5, L10)" in output

    def test_multiple_checks(self):
        echoes = [
            Echo(check="unused-imports", line=5, message="m1"),
            Echo(check="bare-except", line=30, message="m2", severity="error"),
        ]
        output = format_file_echoes("test.py", echoes)
        assert "unused-imports (L5)" in output
        assert "[error] bare-except (L30)" in output

    def test_compact_one_line(self):
        """Output should be a single line."""
        echoes = [
            Echo(check="unused-imports", line=5, message="m1"),
            Echo(check="bare-except", line=30, message="m2"),
        ]
        output = format_file_echoes("test.py", echoes)
        # Should be one line (plus trailing newline)
        assert output.count("\n") == 1

    def test_overflow_when_many_lines(self):
        """More than 3 line numbers per check should show +N overflow."""
        echoes = [Echo(check="unused-imports", line=i, message="m") for i in range(1, 8)]
        output = format_file_echoes("test.py", echoes)
        assert "+4" in output
        assert "L1" in output
        assert "L2" in output
        assert "L3" in output


class TestFormatStopEchoes:
    def test_empty(self):
        assert format_stop_echoes({}) == ""

    def test_single_file(self):
        file_echoes = {
            "src/a.py": [Echo(check="type-error", line=10, message="bad type")]
        }
        output = format_stop_echoes(file_echoes)
        assert "1 echo across 1 file" in output
        assert "src/a.py" in output
        assert "type-error (L10)" in output

    def test_multiple_files(self):
        file_echoes = {
            "a.py": [Echo(check="a", line=1, message="m1")],
            "b.py": [Echo(check="b", line=2, message="m2")],
        }
        output = format_stop_echoes(file_echoes)
        assert "2 echoes across 2 files" in output
        assert "a.py" in output
        assert "b.py" in output

    def test_compact_one_line_per_file(self):
        """Each file should get one line in the output."""
        file_echoes = {
            "a.py": [Echo(check="unused-imports", line=1, message="m")],
            "b.py": [Echo(check="dead-code", line=2, message="m")],
        }
        output = format_stop_echoes(file_echoes)
        lines = [l for l in output.strip().split("\n") if l.strip()]
        assert len(lines) == 3  # header + 2 file lines


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
        assert "capped" in output

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
        assert "unused-imports" in output
        assert "type-error" in output
        assert "capped" in output

    def test_overflow_message_includes_advice(self):
        file_echoes = {
            "a.py": [Echo(check="x", line=1, message="m")] * 5,
        }
        output = format_stop_echoes(file_echoes, cross_file_cap=2)
        assert "echo_cap_cross_file" in output


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


class TestFormatSessionStats:
    def test_empty_entries(self):
        assert format_session_stats([], {}) == ""

    def test_entries_with_echoes(self):
        entries = [
            {"file": "a.py", "echoes": {"unused-imports": 2, "bare-except": 1}},
            {"file": "b.py", "echoes": {"unused-imports": 1}},
        ]
        result = format_session_stats(entries, {})
        assert "4 echoes" in result
        assert "2 files" in result
        assert "~~ ecko ~~ session:" in result
        assert "self-corrected" not in result

    def test_entries_with_corrections(self):
        entries = [
            {"file": "a.py", "echoes": {"unused-imports": 2}},
        ]
        corrections = {"unused-imports": 2}
        result = format_session_stats(entries, corrections)
        assert "2 self-corrected" in result

    def test_entries_clean_files(self):
        entries = [
            {"file": "a.py", "echoes": {}},
        ]
        result = format_session_stats(entries, {})
        assert "0 echoes" in result
        assert "1 files" in result
