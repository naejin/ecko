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


def format_file_echoes(file_path: str, echoes: list[Echo]) -> str:
    """Format echoes for a single file (Layer 2 PostToolUse output)."""
    if not echoes:
        return ""
    count = len(echoes)
    lines = [
        f"~~ ecko ~~  {count} {'echo' if count == 1 else 'echoes'} in {file_path}",
        "",
    ]
    for i, echo in enumerate(echoes, 1):
        lines.append(f"  {i}. {echo.check} (line {echo.line})")
        lines.append(f"     {echo.message}")
        if echo.suggestion:
            lines.append(f"     {echo.suggestion}")
        lines.append("")
    return "\n".join(lines)


def format_stop_echoes(file_echoes: dict[str, list[Echo]]) -> str:
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
        for echo in echoes:
            detail = echo.message
            if echo.suggestion:
                detail += f" {echo.suggestion}"
            lines.append(f"    {i}. {echo.check} (line {echo.line}) \u2014 {detail}")
            i += 1
        lines.append("")
    return "\n".join(lines)


def emit(text: str) -> None:
    """Write text to stderr (where Claude Code reads hook output)."""
    sys.stderr.write(text)
    sys.stderr.flush()
