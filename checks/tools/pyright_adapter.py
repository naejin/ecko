"""Pyright adapter — run pyright and parse JSON output into Echoes."""

from __future__ import annotations

import json
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_python_tool


def run_pyright(
    files: list[str], cwd: str
) -> dict[str, list[Echo]]:
    """Run pyright on specified files. Returns echoes grouped by file."""
    cmd = resolve_python_tool("pyright")
    if not cmd:
        return {}

    try:
        result = subprocess.run(
            [*cmd, "--outputjson", *files],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=60,
        )
    except (subprocess.TimeoutExpired, OSError):
        return {}

    output = result.stdout.strip()
    if not output:
        return {}

    try:
        data = json.loads(output)
    except json.JSONDecodeError:
        return {}

    file_echoes: dict[str, list[Echo]] = {}
    diagnostics = data.get("generalDiagnostics", [])
    for diag in diagnostics:
        severity = diag.get("severity", "")
        if severity != "error":
            continue
        path = diag.get("file", "")
        message = diag.get("message", "")
        line = diag.get("range", {}).get("start", {}).get("line", 0)
        # pyright uses 0-indexed lines
        line += 1
        file_echoes.setdefault(path, []).append(
            Echo(check="type-error", line=line, message=message)
        )

    return file_echoes
