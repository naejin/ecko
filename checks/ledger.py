"""Session ledger — append-only JSONL log with rolling time window.

Records echo counts per hook invocation for self-correction tracking.
Each post-tool-use appends one entry; the stop hook reads and summarizes.
Stale entries (outside the session window) are filtered at read time.
"""

from __future__ import annotations

import json
import os
import time
from typing import Any

_LEDGER_DIR = ".ecko-session"
_LEDGER_FILE = "ledger.jsonl"
_DEFAULT_SESSION_HOURS = 4


def _ledger_path(cwd: str) -> str:
    """Return absolute path to the ledger file."""
    return os.path.join(cwd, _LEDGER_DIR, _LEDGER_FILE)


def _ensure_dir(cwd: str) -> None:
    """Create .ecko-session/ with .gitignore if needed."""
    dir_path = os.path.join(cwd, _LEDGER_DIR)
    os.makedirs(dir_path, exist_ok=True)
    gitignore = os.path.join(dir_path, ".gitignore")
    if not os.path.isfile(gitignore):
        with open(gitignore, "w", encoding="utf-8") as f:
            f.write("*\n")


def append(
    cwd: str,
    file_path: str,
    mode: str,
    echoes: dict[str, int],
) -> None:
    """Append a ledger entry. Stale entries are filtered at read time.

    Args:
        cwd: Project root directory.
        file_path: Absolute path of the file checked (stored as relative).
        mode: "post-tool-use" or "stop".
        echoes: Dict of {check_name: count}. Empty dict for clean files.
    """
    try:
        _ensure_dir(cwd)
    except OSError:
        return  # Can't create directory — skip ledger write

    # Convert to relative path for portable storage
    try:
        rel = os.path.relpath(file_path, cwd).replace(os.sep, "/")
    except ValueError:
        rel = file_path  # Cross-drive on Windows

    entry = {
        "ts": round(time.time(), 1),
        "file": rel,
        "mode": mode,
        "echoes": echoes,
    }

    path = _ledger_path(cwd)

    # True append — no read-modify-write, safe under concurrent access.
    # Stale entries are filtered at read time by _read_raw's cutoff.
    try:
        with open(path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry, separators=(",", ":")) + "\n")
    except OSError:
        pass  # Graceful failure — ledger is best-effort


def read_session(
    cwd: str, session_hours: float = _DEFAULT_SESSION_HOURS
) -> list[dict[str, Any]]:
    """Read all ledger entries within the current session window."""
    path = _ledger_path(cwd)
    cutoff = time.time() - (session_hours * 3600)
    return _read_raw(path, cutoff)


def compute_self_corrections(entries: list[dict[str, Any]]) -> dict[str, int]:
    """Compute self-corrections from ledger entries.

    For each (file, check) pair, compares the count from the first
    post-tool-use entry to the last. Positive delta = echoes resolved.

    Returns {check_name: total_corrections_across_all_files}.
    """
    # Group post-tool-use entries by file, preserving timestamp order
    by_file: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        if entry.get("mode") != "post-tool-use":
            continue
        f = entry.get("file", "")
        if f:
            by_file.setdefault(f, []).append(entry)

    corrections: dict[str, int] = {}
    for file_entries in by_file.values():
        if len(file_entries) < 2:
            continue
        ordered = sorted(file_entries, key=lambda e: e.get("ts", 0))
        first_echoes = ordered[0].get("echoes", {})
        last_echoes = ordered[-1].get("echoes", {})

        for check, count in first_echoes.items():
            current = last_echoes.get(check, 0)
            delta = count - current
            if delta > 0:
                corrections[check] = corrections.get(check, 0) + delta

    return corrections


def _read_raw(path: str, cutoff: float) -> list[dict[str, Any]]:
    """Read entries from ledger file, filtering by cutoff timestamp."""
    if not os.path.isfile(path):
        return []
    entries: list[dict[str, Any]] = []
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                    if entry.get("ts", 0) >= cutoff:
                        entries.append(entry)
                except json.JSONDecodeError:
                    continue  # Skip malformed lines
    except OSError:
        return []
    return entries


