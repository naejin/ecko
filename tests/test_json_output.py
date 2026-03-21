"""Tests for structured JSON output feature."""

from __future__ import annotations

import json

from checks.config import get_output_format
from checks.result import Echo, format_file_echoes_json, format_stop_echoes_json


class TestConfigGetter:
    def test_default_text(self):
        assert get_output_format({}) == "text"

    def test_explicit_json(self):
        assert get_output_format({"output_format": "json"}) == "json"

    def test_explicit_text(self):
        assert get_output_format({"output_format": "text"}) == "text"

    def test_invalid_value_falls_back_to_text(self):
        assert get_output_format({"output_format": "yaml"}) == "text"

    def test_non_string_falls_back_to_text(self):
        assert get_output_format({"output_format": 42}) == "text"


class TestFileEchoesJson:
    def test_valid_json(self):
        echoes = [Echo(check="unused-imports", line=3, message="unused")]
        output = format_file_echoes_json("test.py", echoes)
        data = json.loads(output)
        assert data["schema_version"] == 1

    def test_mode_is_post_tool_use(self):
        echoes = [Echo(check="unused-imports", line=3, message="unused")]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        assert data["mode"] == "post-tool-use"

    def test_file_path_included(self):
        echoes = [Echo(check="unused-imports", line=3, message="unused")]
        data = json.loads(format_file_echoes_json("src/app.py", echoes))
        assert data["file"] == "src/app.py"

    def test_all_echoes_included_no_cap(self):
        """JSON mode should include all echoes regardless of any cap."""
        echoes = [Echo(check="unused-imports", line=i, message=f"echo {i}") for i in range(20)]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        assert len(data["echoes"]) == 20

    def test_severity_field_included(self):
        echoes = [
            Echo(check="bare-except", line=5, message="bare", severity="error"),
            Echo(check="unused-imports", line=3, message="unused"),
        ]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        assert data["echoes"][0]["severity"] == "error"
        assert data["echoes"][1]["severity"] == "warn"

    def test_suggestion_field_included(self):
        echoes = [Echo(check="dead-code", line=10, message="unused", suggestion="Remove it")]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        assert data["echoes"][0]["suggestion"] == "Remove it"

    def test_skipped_tools(self):
        echoes = [Echo(check="unused-imports", line=3, message="unused")]
        data = json.loads(format_file_echoes_json("test.py", echoes, skipped_tools=["biome"]))
        assert data["skipped_tools"] == ["biome"]

    def test_skipped_tools_default_empty(self):
        echoes = [Echo(check="unused-imports", line=3, message="unused")]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        assert data["skipped_tools"] == []

    def test_echo_fields(self):
        echoes = [Echo(check="unused-imports", line=3, message="msg", suggestion="fix")]
        data = json.loads(format_file_echoes_json("test.py", echoes))
        echo = data["echoes"][0]
        assert echo["check"] == "unused-imports"
        assert echo["line"] == 3
        assert echo["message"] == "msg"
        assert echo["suggestion"] == "fix"
        assert echo["severity"] == "warn"


class TestStopEchoesJson:
    def test_valid_json(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        output = format_stop_echoes_json(file_echoes, elapsed=1.5)
        data = json.loads(output)
        assert data["schema_version"] == 1

    def test_mode_is_stop(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        data = json.loads(format_stop_echoes_json(file_echoes, elapsed=1.5))
        assert data["mode"] == "stop"

    def test_elapsed_included(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        data = json.loads(format_stop_echoes_json(file_echoes, elapsed=2.345))
        assert data["elapsed"] == 2.3

    def test_files_structure(self):
        file_echoes = {
            "a.py": [Echo(check="dead-code", line=5, message="unused")],
            "b.py": [
                Echo(check="unused-imports", line=1, message="import"),
                Echo(check="type-error", line=10, message="mismatch", severity="error"),
            ],
        }
        data = json.loads(format_stop_echoes_json(file_echoes, elapsed=1.0))
        assert len(data["files"]["a.py"]) == 1
        assert len(data["files"]["b.py"]) == 2
        assert data["files"]["b.py"][1]["severity"] == "error"

    def test_corrections_included(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        data = json.loads(
            format_stop_echoes_json(
                file_echoes, elapsed=1.0, corrections={"unused-imports": 3}
            )
        )
        assert data["corrections"] == {"unused-imports": 3}

    def test_corrections_omitted_when_empty(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        data = json.loads(format_stop_echoes_json(file_echoes, elapsed=1.0))
        assert "corrections" not in data

    def test_skipped_tools(self):
        file_echoes = {"a.py": [Echo(check="dead-code", line=5, message="unused")]}
        data = json.loads(
            format_stop_echoes_json(
                file_echoes, elapsed=1.0, skipped_tools=["pyright", "vulture"]
            )
        )
        assert data["skipped_tools"] == ["pyright", "vulture"]

    def test_empty_files(self):
        data = json.loads(format_stop_echoes_json({}, elapsed=0.5))
        assert data["files"] == {}
        assert data["mode"] == "stop"
