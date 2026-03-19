"""Ruff adapter — run ruff and parse JSON output into Echoes."""

from __future__ import annotations

import json
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_python_tool

# Ruff rules we check
RUFF_RULES = "F401,E711,E712,E722,F403,B006,A001,A002,S110"

# Map ruff rule codes to ecko check names
RULE_MAP = {
    "F401": "unused-imports",
    "E711": "singleton-comparison",
    "E712": "singleton-comparison",
    "E722": "bare-except",
    "F403": "star-imports",
    "B006": "mutable-default-args",
    "A001": "builtin-shadowing",
    "A002": "builtin-shadowing",
    "S110": "empty-error-handlers",
}


def run_ruff(file_path: str) -> list[Echo]:
    """Run ruff on a file and return echoes."""
    cmd = resolve_python_tool("ruff")
    if not cmd:
        return []

    try:
        result = subprocess.run(
            [
                *cmd,
                "check",
                "--select",
                RUFF_RULES,
                "--output-format",
                "json",
                "--no-fix",
                file_path,
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (subprocess.TimeoutExpired, OSError):
        return []

    # ruff exits 1 when violations found — that's expected
    output = result.stdout.strip()
    if not output:
        return []

    try:
        violations = json.loads(output)
    except json.JSONDecodeError:
        return []

    echoes: list[Echo] = []
    for v in violations:
        code = v.get("code", "")
        check = RULE_MAP.get(code, code.lower())
        line = v.get("location", {}).get("row", 0)
        message = v.get("message", "")
        echoes.append(Echo(check=check, line=line, message=message))

    return echoes
