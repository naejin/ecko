"""Echo dataclass and output formatter."""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass


@dataclass
class Echo:
    check: str
    line: int
    message: str
    suggestion: str = ""
    severity: str = "warn"


# Max line numbers shown per check in compact format
_COMPACT_LINES_PER_CHECK = 3


def _format_compact_checks(echoes: list[Echo]) -> str:
    """Format echoes as compact 'check (L1, L2 +N)' pairs."""
    # Group by check, preserving order of first occurrence
    by_check: dict[str, list[int]] = {}
    severity_map: dict[str, str] = {}
    for e in echoes:
        by_check.setdefault(e.check, []).append(e.line)
        if e.severity == "error":
            severity_map[e.check] = "error"

    parts: list[str] = []
    for check, lines in by_check.items():
        prefix = "[error] " if severity_map.get(check) == "error" else ""
        if len(lines) <= _COMPACT_LINES_PER_CHECK:
            line_str = ", ".join(f"L{ln}" for ln in lines)
        else:
            shown = ", ".join(f"L{ln}" for ln in lines[:_COMPACT_LINES_PER_CHECK])
            line_str = f"{shown} +{len(lines) - _COMPACT_LINES_PER_CHECK}"
        parts.append(f"{prefix}{check} ({line_str})")
    return ", ".join(parts)


def format_file_echoes(file_path: str, echoes: list[Echo]) -> str:
    """Format echoes for a single file (Layer 2 PostToolUse output).

    Compact one-line format: ~~ ecko ~~ file — check (L1, L2), check2 (L5)
    """
    if not echoes:
        return ""
    compact = _format_compact_checks(echoes)
    return f"~~ ecko ~~ {file_path} — {compact}\n"


def format_stop_echoes(
    file_echoes: dict[str, list[Echo]], cross_file_cap: int = 0
) -> str:
    """Format echoes for the stop hook (Layer 3 deep analysis output).

    Compact format: header line + one line per file.
    """
    total = sum(len(e) for e in file_echoes.values())
    if total == 0:
        return ""
    file_count = len(file_echoes)
    header = (
        f"~~ ecko ~~  {total} {'echo' if total == 1 else 'echoes'}"
        f" across {file_count} {'file' if file_count == 1 else 'files'}"
    )
    lines = [header]

    # Apply cross-file cap: count echoes per check across all files
    cross_counts: dict[str, int] = {}
    cross_overflow: dict[str, int] = {}

    for path, echoes in file_echoes.items():
        if cross_file_cap > 0:
            # Filter echoes that exceed the cross-file cap
            filtered: list[Echo] = []
            for echo in echoes:
                cross_counts[echo.check] = cross_counts.get(echo.check, 0) + 1
                if cross_counts[echo.check] <= cross_file_cap:
                    filtered.append(echo)
                else:
                    cross_overflow[echo.check] = cross_overflow.get(echo.check, 0) + 1
            if not filtered:
                continue
            compact = _format_compact_checks(filtered)
        else:
            compact = _format_compact_checks(echoes)
        lines.append(f"  {path} — {compact}")

    if cross_overflow:
        lines[0] += f" (display capped at {cross_file_cap} per check)"
        overflow_parts = [f"{check} +{count}" for check, count in cross_overflow.items()]
        lines.append(f"  ... capped: {', '.join(overflow_parts)} (set echo_cap_cross_file: 0 to see all)")

    return "\n".join(lines) + "\n"


def format_correction_summary(corrections: dict[str, int]) -> str:
    """Format a one-line self-correction summary for stop hook output.

    Returns empty string if no corrections.
    """
    if not corrections:
        return ""
    fixed = sum(corrections.values())
    breakdown = sorted(corrections.items(), key=lambda x: -x[1])
    parts = [f"{count} {check}" for check, count in breakdown]
    return f"~~ ecko ~~ self-corrections: {fixed} fixed ({', '.join(parts)})\n"


def format_session_stats(
    entries: list[dict], corrections: dict[str, int]
) -> str:
    """Format a one-line session stats summary for stop hook output.

    Returns empty string if no session data.
    """
    if not entries:
        return ""
    files: set[str] = set()
    total_echoes = 0
    for entry in entries:
        f = entry.get("file", "")
        if f:
            files.add(f)
        for count in entry.get("echoes", {}).values():
            total_echoes += count
    total_corrected = sum(corrections.values())
    parts = [f"{total_echoes} echoes across {len(files)} files"]
    if total_corrected:
        parts.append(f"{total_corrected} self-corrected")
    return f"~~ ecko ~~ session: {', '.join(parts)}\n"


def format_file_echoes_json(
    file_path: str,
    echoes: list[Echo],
    skipped_tools: list[str] | None = None,
) -> str:
    """Format echoes as JSON for a single file (no echo caps applied)."""
    data = {
        "schema_version": 1,
        "mode": "post-tool-use",
        "file": file_path,
        "echoes": [
            {
                "check": e.check,
                "line": e.line,
                "message": e.message,
                "suggestion": e.suggestion,
                "severity": e.severity,
            }
            for e in echoes
        ],
        "skipped_tools": skipped_tools or [],
    }
    return json.dumps(data) + "\n"


def format_stop_echoes_json(
    file_echoes: dict[str, list[Echo]],
    elapsed: float,
    skipped_tools: list[str] | None = None,
    corrections: dict[str, int] | None = None,
) -> str:
    """Format stop mode echoes as JSON (no echo caps applied)."""
    files = {}
    for path, echoes in file_echoes.items():
        files[path] = [
            {
                "check": e.check,
                "line": e.line,
                "message": e.message,
                "suggestion": e.suggestion,
                "severity": e.severity,
            }
            for e in echoes
        ]
    data: dict = {
        "schema_version": 1,
        "mode": "stop",
        "files": files,
        "elapsed": round(elapsed, 1),
        "skipped_tools": skipped_tools or [],
    }
    if corrections:
        data["corrections"] = corrections
    return json.dumps(data) + "\n"


def has_errors(echoes: list[Echo]) -> bool:
    """Return True if any echo has error severity."""
    return any(e.severity == "error" for e in echoes)


def emit(text: str) -> None:
    """Write text to stderr (where Claude Code reads hook output)."""
    sys.stderr.write(text)
    sys.stderr.flush()
