"""Tests for the session ledger module."""

from __future__ import annotations

import json
import os
import time

from checks.ledger import (
    _LEDGER_DIR,
    _LEDGER_FILE,
    _ledger_path,
    append,
    compute_self_corrections,
    read_session,
)


class TestAppend:
    def test_creates_directory_and_file(self, tmp_path):
        append(str(tmp_path), str(tmp_path / "app.py"), "post-tool-use", {})
        assert (tmp_path / _LEDGER_DIR / _LEDGER_FILE).is_file()

    def test_creates_gitignore(self, tmp_path):
        append(str(tmp_path), str(tmp_path / "app.py"), "post-tool-use", {})
        gi = tmp_path / _LEDGER_DIR / ".gitignore"
        assert gi.is_file()
        assert gi.read_text() == "*\n"

    def test_gitignore_idempotent(self, tmp_path):
        append(str(tmp_path), str(tmp_path / "a.py"), "post-tool-use", {})
        append(str(tmp_path), str(tmp_path / "b.py"), "post-tool-use", {})
        gi = tmp_path / _LEDGER_DIR / ".gitignore"
        assert gi.read_text() == "*\n"

    def test_appends_entry(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "a.py"), "post-tool-use", {"unused-imports": 2})
        append(cwd, str(tmp_path / "b.py"), "post-tool-use", {"bare-except": 1})
        path = _ledger_path(cwd)
        with open(path, encoding="utf-8") as f:
            lines = [line for line in f if line.strip()]
        assert len(lines) == 2

    def test_records_clean_file(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "clean.py"), "post-tool-use", {})
        entries = read_session(cwd)
        assert len(entries) == 1
        assert entries[0]["echoes"] == {}

    def test_records_echo_counts(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "bad.py"), "post-tool-use", {"unused-imports": 3, "bare-except": 1})
        entries = read_session(cwd)
        assert entries[0]["echoes"] == {"unused-imports": 3, "bare-except": 1}

    def test_stores_relative_path(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "src" / "app.py"), "post-tool-use", {})
        entries = read_session(cwd)
        assert entries[0]["file"] == "src/app.py"

    def test_records_mode(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "a.py"), "post-tool-use", {})
        append(cwd, str(tmp_path / "a.py"), "stop", {"type-error": 1})
        entries = read_session(cwd)
        assert entries[0]["mode"] == "post-tool-use"
        assert entries[1]["mode"] == "stop"

    def test_old_entries_filtered_at_read_time(self, tmp_path):
        """True append — no pruning on write; old entries filtered by read_session."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        old_entry = {"ts": time.time() - 20000, "file": "old.py", "mode": "post-tool-use", "echoes": {}}
        with open(path, "w", encoding="utf-8") as f:
            f.write(json.dumps(old_entry) + "\n")
        # Append keeps both on disk, but read_session filters the stale one
        append(cwd, str(tmp_path / "new.py"), "post-tool-use", {})
        entries = read_session(cwd)
        assert len(entries) == 1
        assert entries[0]["file"] == "new.py"

    def test_graceful_on_readonly(self):
        # Calling append on a nonexistent deeply nested path should not raise
        append("/nonexistent/deep/path", "/nonexistent/deep/path/file.py", "post-tool-use", {})
        # No exception raised

    def test_utf8_file_paths(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "cafè.py"), "post-tool-use", {"unicode-artifact": 1})
        entries = read_session(cwd)
        assert "caf" in entries[0]["file"]

    def test_timestamp_is_float(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "a.py"), "post-tool-use", {})
        entries = read_session(cwd)
        assert isinstance(entries[0]["ts"], float)


class TestReadSession:
    def test_empty_when_no_file(self, tmp_path):
        entries = read_session(str(tmp_path))
        assert entries == []

    def test_returns_entries_within_window(self, tmp_path):
        cwd = str(tmp_path)
        append(cwd, str(tmp_path / "a.py"), "post-tool-use", {"x": 1})
        append(cwd, str(tmp_path / "b.py"), "post-tool-use", {"y": 2})
        entries = read_session(cwd)
        assert len(entries) == 2

    def test_filters_by_window(self, tmp_path):
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        # Write entries: one old, one recent
        now = time.time()
        old = {"ts": now - 20000, "file": "old.py", "mode": "post-tool-use", "echoes": {}}
        recent = {"ts": now, "file": "recent.py", "mode": "post-tool-use", "echoes": {}}
        with open(path, "w", encoding="utf-8") as f:
            f.write(json.dumps(old) + "\n")
            f.write(json.dumps(recent) + "\n")
        entries = read_session(cwd)
        assert len(entries) == 1
        assert entries[0]["file"] == "recent.py"

    def test_skips_corrupt_lines(self, tmp_path):
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        now = time.time()
        with open(path, "w", encoding="utf-8") as f:
            f.write(json.dumps({"ts": now, "file": "ok.py", "mode": "post-tool-use", "echoes": {}}) + "\n")
            f.write("this is not json\n")
            f.write(json.dumps({"ts": now, "file": "also_ok.py", "mode": "post-tool-use", "echoes": {}}) + "\n")
        entries = read_session(cwd)
        assert len(entries) == 2

    def test_custom_session_hours(self, tmp_path):
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        now = time.time()
        # Entry from 3 hours ago — within default 4h but outside 2h window
        entry = {"ts": now - 10800, "file": "old.py", "mode": "post-tool-use", "echoes": {}}
        with open(path, "w", encoding="utf-8") as f:
            f.write(json.dumps(entry) + "\n")
        assert len(read_session(cwd, session_hours=4)) == 1
        assert len(read_session(cwd, session_hours=2)) == 0


class TestComputeSelfCorrections:
    def test_empty_entries(self):
        assert compute_self_corrections([]) == {}

    def test_single_entry_no_corrections(self):
        entries = [{"file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 3}}]
        assert compute_self_corrections(entries) == {}

    def test_echo_reduced(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 3}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 1}},
        ]
        result = compute_self_corrections(entries)
        assert result == {"unused-imports": 2}

    def test_echo_eliminated(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"type-error": 2}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {}},
        ]
        result = compute_self_corrections(entries)
        assert result == {"type-error": 2}

    def test_echo_increased_not_correction(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 1}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 3}},
        ]
        result = compute_self_corrections(entries)
        assert result == {}

    def test_multiple_checks(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 3, "bare-except": 2}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 1, "bare-except": 2}},
        ]
        result = compute_self_corrections(entries)
        assert result == {"unused-imports": 2}

    def test_multiple_files(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 2}},
            {"ts": 1, "file": "b.py", "mode": "post-tool-use", "echoes": {"bare-except": 3}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 0}},
            {"ts": 2, "file": "b.py", "mode": "post-tool-use", "echoes": {"bare-except": 1}},
        ]
        result = compute_self_corrections(entries)
        assert result == {"unused-imports": 2, "bare-except": 2}

    def test_ignores_stop_entries(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 3}},
            {"ts": 2, "file": "a.py", "mode": "stop", "echoes": {"unused-imports": 0}},
        ]
        # Only one post-tool-use entry, so no correction (need >= 2)
        result = compute_self_corrections(entries)
        assert result == {}

    def test_clean_file_skipped(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {}},
        ]
        result = compute_self_corrections(entries)
        assert result == {}

    def test_new_check_in_last_entry(self):
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 2}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 0, "bare-except": 1}},
        ]
        result = compute_self_corrections(entries)
        # unused-imports fixed, bare-except is new (not a correction)
        assert result == {"unused-imports": 2}

    def test_intermediate_entries_ignored(self):
        """Only first and last entries matter for correction computation."""
        entries = [
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 5}},
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 10}},
            {"ts": 3, "file": "a.py", "mode": "post-tool-use", "echoes": {"unused-imports": 2}},
        ]
        result = compute_self_corrections(entries)
        # First had 5, last has 2 = 3 corrected (intermediate spike ignored)
        assert result == {"unused-imports": 3}

    def test_pure_function_no_side_effects(self):
        entries = [
            {"ts": 2, "file": "a.py", "mode": "post-tool-use", "echoes": {"x": 1}},
            {"ts": 1, "file": "a.py", "mode": "post-tool-use", "echoes": {"x": 3}},
        ]
        compute_self_corrections(entries)
        # Entries should not be reordered (sorted() creates a new list)
        assert entries[0]["ts"] == 2
        assert entries[1]["ts"] == 1


