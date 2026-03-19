"""tsc adapter — run TypeScript compiler and parse error output into Echoes."""

from __future__ import annotations

import re
import shutil
import subprocess

from checks.result import Echo

# tsc error format: path(line,col): error TSxxxx: message
TSC_PATTERN = re.compile(r"^(.+?)\((\d+),(\d+)\):\s+error\s+TS\d+:\s+(.+)$")


def run_tsc(cwd: str) -> dict[str, list[Echo]]:
    """Run tsc --noEmit in the project root. Returns echoes grouped by file."""
    tsc = shutil.which("tsc")
    if not tsc:
        return {}

    try:
        result = subprocess.run(
            [tsc, "--noEmit"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=60,
        )
    except (subprocess.TimeoutExpired, OSError):
        return {}

    # tsc writes errors to stdout
    output = result.stdout + result.stderr
    if not output.strip():
        return {}

    file_echoes: dict[str, list[Echo]] = {}
    for line in output.splitlines():
        match = TSC_PATTERN.match(line.strip())
        if match:
            path = match.group(1)
            lineno = int(match.group(2))
            message = match.group(4)
            file_echoes.setdefault(path, []).append(
                Echo(check="type-error", line=lineno, message=message)
            )

    return file_echoes
