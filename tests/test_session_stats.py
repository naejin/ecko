"""Tests for session_stats standalone script."""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path


SCRIPT = str(Path(__file__).parent.parent / "checks" / "session_stats.py")


def _write_ledger(tmp_path, entries):
    """Write ledger entries to a temp .ecko-session/ledger.jsonl."""
    session_dir = tmp_path / ".ecko-session"
    session_dir.mkdir(exist_ok=True)
    ledger = session_dir / "ledger.jsonl"
    lines = [json.dumps(e) for e in entries]
    ledger.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _run_session_stats(cwd):
    """Run session_stats.py and return stdout."""
    result = subprocess.run(
        [sys.executable, SCRIPT, "--cwd", str(cwd)],
        capture_output=True,
        text=True,
        timeout=10,
    )
    return result.stdout


class TestSessionStats:
    def test_empty_ledger(self, tmp_path):
        output = _run_session_stats(tmp_path)
        assert "No session data yet" in output

    def test_disabled_session(self, tmp_path):
        # Write config with session_hours: 0
        config = tmp_path / "ecko.yaml"
        config.write_text("session_hours: 0\n", encoding="utf-8")
        output = _run_session_stats(tmp_path)
        assert "disabled" in output.lower()

    def test_populated_ledger(self, tmp_path):
        now = time.time()
        entries = [
            {"ts": now - 60, "file": "a.py", "mode": "post-tool-use",
             "echoes": {"unused-imports": 2, "bare-except": 1}},
            {"ts": now - 30, "file": "b.py", "mode": "post-tool-use",
             "echoes": {"unused-imports": 1}},
        ]
        _write_ledger(tmp_path, entries)
        output = _run_session_stats(tmp_path)
        assert "Files touched:" in output
        assert "2" in output  # 2 files
        assert "Total echoes:" in output
        assert "unused-imports" in output

    def test_top_checks_ordering(self, tmp_path):
        now = time.time()
        entries = [
            {"ts": now - 60, "file": "a.py", "mode": "post-tool-use",
             "echoes": {"bare-except": 1, "unused-imports": 5, "dead-code": 3}},
        ]
        _write_ledger(tmp_path, entries)
        output = _run_session_stats(tmp_path)
        # unused-imports (5) should appear before dead-code (3) before bare-except (1)
        idx_unused = output.index("unused-imports")
        idx_dead = output.index("dead-code")
        idx_bare = output.index("bare-except")
        assert idx_unused < idx_dead < idx_bare

    def test_clean_first_pass(self, tmp_path):
        now = time.time()
        entries = [
            {"ts": now - 60, "file": "clean.py", "mode": "post-tool-use",
             "echoes": {}},
            {"ts": now - 30, "file": "dirty.py", "mode": "post-tool-use",
             "echoes": {"unused-imports": 1}},
        ]
        _write_ledger(tmp_path, entries)
        output = _run_session_stats(tmp_path)
        assert "Clean first pass:" in output
        assert "1/2" in output
