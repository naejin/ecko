"""Vulture adapter — run vulture and parse output into Echoes."""

from __future__ import annotations

import re
import shutil
import subprocess

from checks.result import Echo

# vulture output: path:line: unused function 'name' (confidence: 80%)
VULTURE_PATTERN = re.compile(
    r"^(.+?):(\d+):\s+(.+?)\s+\((\d+)% confidence\)$"
)


def run_vulture(cwd: str) -> dict[str, list[Echo]]:
    """Run vulture with 80% confidence threshold. Returns echoes grouped by file."""
    vulture = shutil.which("vulture")
    if not vulture:
        return {}

    try:
        result = subprocess.run(
            [vulture, ".", "--min-confidence", "80"],
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

    file_echoes: dict[str, list[Echo]] = {}
    for line in output.splitlines():
        match = VULTURE_PATTERN.match(line.strip())
        if match:
            path = match.group(1)
            lineno = int(match.group(2))
            message = match.group(3)
            file_echoes.setdefault(path, []).append(
                Echo(
                    check="dead-code",
                    line=lineno,
                    message=message,
                    suggestion="Remove it if truly unused.",
                )
            )

    return file_echoes
