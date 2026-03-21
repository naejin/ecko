"""Pyright adapter — run pyright and parse JSON output into Echoes."""

from __future__ import annotations

import json
import subprocess

from checks.result import Echo, emit
from checks.tools.resolve import resolve_python_tool


def run_pyright(files: list[str], cwd: str) -> dict[str, list[Echo]]:
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
    except subprocess.TimeoutExpired:
        emit("~~ ecko ~~ warning: pyright timed out (60s limit)\n")
        return {}
    except OSError as exc:
        emit(f"~~ ecko ~~ warning: pyright failed: {exc}\n")
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
        # Skip unresolved imports — indicates missing deps, not code defects.
        if "could not be resolved" in message:
            continue
        line = diag.get("range", {}).get("start", {}).get("line", 0)
        # pyright uses 0-indexed lines
        line += 1
        file_echoes.setdefault(path, []).append(
            Echo(check="type-error", line=line, message=message, severity="error")
        )

    return file_echoes
