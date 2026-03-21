"""Tests for ledger pruning behavior."""

from __future__ import annotations

import json
import os
import time

from checks.ledger import (
    _PRUNE_SIZE_THRESHOLD,
    _ledger_path,
    _maybe_prune,
    read_session,
)


def _write_entries(path: str, entries: list[dict]) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        for entry in entries:
            f.write(json.dumps(entry, separators=(",", ":")) + "\n")


class TestLedgerPruning:
    def test_no_prune_small_file(self, tmp_path):
        """Files under 50KB are never pruned."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        now = time.time()
        old = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {},
            }
            for i in range(10)
        ]
        _write_entries(path, old)
        assert os.path.getsize(path) < _PRUNE_SIZE_THRESHOLD
        entries = read_session(cwd)
        assert entries == []  # all stale
        # File still has all 10 lines (not pruned)
        with open(path, encoding="utf-8") as f:
            assert len([line for line in f if line.strip()]) == 10

    def test_prune_triggers_on_large_stale_file(self, tmp_path):
        """Files >50KB with >50% stale entries are pruned."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        now = time.time()
        old_entries = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {"x": 1},
                "padding": "x" * 200,
            }
            for i in range(300)
        ]
        new_entries = [
            {"ts": now, "file": f"new{i}.py", "mode": "post-tool-use", "echoes": {}}
            for i in range(5)
        ]
        _write_entries(path, old_entries + new_entries)
        assert os.path.getsize(path) > _PRUNE_SIZE_THRESHOLD
        entries = read_session(cwd)
        assert len(entries) == 5
        # After pruning, file should be much smaller
        with open(path, encoding="utf-8") as f:
            remaining = [line for line in f if line.strip()]
        assert len(remaining) == 5

    def test_no_prune_when_mostly_active(self, tmp_path):
        """Files with <50% stale entries are not pruned."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        now = time.time()
        active = [
            {
                "ts": now,
                "file": f"new{i}.py",
                "mode": "post-tool-use",
                "echoes": {},
                "padding": "x" * 200,
            }
            for i in range(300)
        ]
        stale = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {},
                "padding": "x" * 200,
            }
            for i in range(100)
        ]
        _write_entries(path, stale + active)
        assert os.path.getsize(path) > _PRUNE_SIZE_THRESHOLD
        entries = read_session(cwd)
        assert len(entries) == 300
        # File should still have all 400 lines (75% active, no prune)
        with open(path, encoding="utf-8") as f:
            remaining = [line for line in f if line.strip()]
        assert len(remaining) == 400

    def test_prune_preserves_active_entries(self, tmp_path):
        """After pruning, all active entries are preserved with correct data."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        now = time.time()
        old = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {"x": 1},
                "padding": "x" * 200,
            }
            for i in range(300)
        ]
        active = [
            {
                "ts": now,
                "file": f"active{i}.py",
                "mode": "post-tool-use",
                "echoes": {"unused-imports": i},
            }
            for i in range(3)
        ]
        _write_entries(path, old + active)
        entries = read_session(cwd)
        assert len(entries) == 3
        for i, e in enumerate(entries):
            assert e["file"] == f"active{i}.py"
            assert e["echoes"] == {"unused-imports": i}

    def test_prune_cleans_up_temp_file(self, tmp_path):
        """Temp file does not remain after a successful prune."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        tmp_file = path + ".tmp"
        now = time.time()
        old = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {},
                "padding": "x" * 200,
            }
            for i in range(300)
        ]
        new = [{"ts": now, "file": "new.py", "mode": "post-tool-use", "echoes": {}}]
        _write_entries(path, old + new)
        read_session(cwd)
        assert not os.path.exists(tmp_file)

    def test_prune_produces_valid_jsonl(self, tmp_path):
        """File is valid JSONL after prune."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        now = time.time()
        old = [
            {
                "ts": now - 99999,
                "file": f"old{i}.py",
                "mode": "post-tool-use",
                "echoes": {},
                "padding": "x" * 200,
            }
            for i in range(300)
        ]
        new = [{"ts": now, "file": "new.py", "mode": "post-tool-use", "echoes": {}}]
        _write_entries(path, old + new)
        read_session(cwd)
        with open(path, encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    json.loads(line)  # should not raise

    def test_nonexistent_file_no_error(self, tmp_path):
        """_maybe_prune on nonexistent file does nothing."""
        _maybe_prune(str(tmp_path / "nonexistent.jsonl"), 0, time.time())

    def test_corrupt_lines_dropped_during_prune(self, tmp_path):
        """Corrupt lines are dropped during prune."""
        cwd = str(tmp_path)
        path = _ledger_path(cwd)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        now = time.time()
        with open(path, "w", encoding="utf-8") as f:
            for i in range(300):
                f.write(
                    json.dumps(
                        {
                            "ts": now - 99999,
                            "file": f"old{i}.py",
                            "mode": "post-tool-use",
                            "echoes": {},
                            "padding": "x" * 200,
                        }
                    )
                    + "\n"
                )
            f.write("this is not json\n")
            f.write(
                json.dumps(
                    {"ts": now, "file": "new.py", "mode": "post-tool-use", "echoes": {}}
                )
                + "\n"
            )
        entries = read_session(cwd)
        assert len(entries) == 1
        # After prune, corrupt line is gone
        with open(path, encoding="utf-8") as f:
            lines = [line for line in f if line.strip()]
        assert len(lines) == 1
