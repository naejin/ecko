"""Echo dataclass and output formatter."""

from __future__ import annotations

import sys
from dataclasses import dataclass


@dataclass
class Echo:
    check: str
    line: int
    message: str
    suggestion: str = ""


_CAP_ADVICE = "capped at {cap} per check — set echo_cap_per_check: 0 in ecko.yaml to see all"


def _cap_echoes(
    echoes: list[Echo], echo_cap: int
) -> tuple[list[Echo], dict[str, int]]:
    """Apply per-check cap. Returns (displayed echoes, overflow counts by check)."""
    if echo_cap <= 0:
        return echoes, {}
    counts: dict[str, int] = {}
    displayed: list[Echo] = []
    overflow: dict[str, int] = {}
    for echo in echoes:
        counts[echo.check] = counts.get(echo.check, 0) + 1
        if counts[echo.check] <= echo_cap:
            displayed.append(echo)
        else:
            overflow[echo.check] = overflow.get(echo.check, 0) + 1
    return displayed, overflow


def format_file_echoes(
    file_path: str, echoes: list[Echo], echo_cap: int = 0
) -> str:
    """Format echoes for a single file (Layer 2 PostToolUse output)."""
    if not echoes:
        return ""
    total = len(echoes)
    lines = [
        f"~~ ecko ~~  {total} {'echo' if total == 1 else 'echoes'} in {file_path}",
        "",
    ]
    displayed, overflow = _cap_echoes(echoes, echo_cap)
    for i, echo in enumerate(displayed, 1):
        lines.append(f"  {i}. {echo.check} (line {echo.line})")
        lines.append(f"     {echo.message}")
        if echo.suggestion:
            lines.append(f"     {echo.suggestion}")
        lines.append("")
    for check, count in overflow.items():
        lines.append(f"  ... and {count} more {check}")
    if overflow:
        lines.append(f"  ({_CAP_ADVICE.format(cap=echo_cap)})")
        lines.append("")
    return "\n".join(lines)


def format_stop_echoes(
    file_echoes: dict[str, list[Echo]], echo_cap: int = 0
) -> str:
    """Format echoes for the stop hook (Layer 3 deep analysis output)."""
    total = sum(len(e) for e in file_echoes.values())
    if total == 0:
        return ""
    file_count = len(file_echoes)
    lines = [
        f"~~ ecko ~~  final sweep  ~~  {total} {'echo' if total == 1 else 'echoes'} across {file_count} {'file' if file_count == 1 else 'files'}",
        "",
    ]
    i = 1
    for path, echoes in file_echoes.items():
        lines.append(f"  {path}:")
        displayed, overflow = _cap_echoes(echoes, echo_cap)
        for echo in displayed:
            detail = echo.message
            if echo.suggestion:
                detail += f" {echo.suggestion}"
            lines.append(f"    {i}. {echo.check} (line {echo.line}) \u2014 {detail}")
            i += 1
        for check, count in overflow.items():
            lines.append(f"    ... and {count} more {check}")
        if overflow:
            lines.append(f"    ({_CAP_ADVICE.format(cap=echo_cap)})")
        lines.append("")
    return "\n".join(lines)


def emit(text: str) -> None:
    """Write text to stderr (where Claude Code reads hook output)."""
    sys.stderr.write(text)
    sys.stderr.flush()
