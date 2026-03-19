"""Tests for the Echo formatter."""

from __future__ import annotations

from checks.result import Echo, format_file_echoes, format_stop_echoes


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
