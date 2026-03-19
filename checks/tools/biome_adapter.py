"""Biome adapter — run biome and parse JSON output into Echoes."""

from __future__ import annotations

import json
import os
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_node_tool

# Map biome rule names to ecko check names
RULE_MAP = {
    "noUnusedImports": "unused-imports",
    "noUnreachable": "unreachable-code",
    "noDebugger": "debugger-statements",
    "noVar": "var-declarations",
    "noDuplicateObjectKeys": "duplicate-keys",
    "noEmptyBlockStatements": "empty-error-handlers",
    "noUselessCatch": "useless-catch",
}


def run_biome(file_path: str, plugin_root: str) -> list[Echo]:
    """Run biome on a file and return echoes."""
    cmd = resolve_node_tool("biome", package="@biomejs/biome")
    if not cmd:
        return []

    config_path = os.path.join(plugin_root, "config")

    try:
        result = subprocess.run(
            [
                *cmd,
                "lint",
                "--config-path",
                config_path,
                "--reporter=json",
                file_path,
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (subprocess.TimeoutExpired, OSError):
        return []

    output = result.stdout.strip()
    if not output:
        return []

    try:
        data = json.loads(output)
    except json.JSONDecodeError:
        return []

    echoes: list[Echo] = []
    diagnostics = data.get("diagnostics", [])
    for diag in diagnostics:
        rule_name = ""
        category = diag.get("category", "")
        # biome category format: "lint/ruleName" or "lint/group/ruleName"
        if "/" in category:
            rule_name = category.rsplit("/", 1)[-1]

        check = RULE_MAP.get(rule_name, rule_name)
        message = diag.get("description", "") or diag.get("message", "")
        # Extract line number from location
        line = 0
        location = diag.get("location", {})
        span = location.get("span", {})
        if isinstance(span, list) and span:
            # biome gives byte offsets — approximate line from sourceCode
            source = location.get("sourceCode", "")
            offset = span[0] if isinstance(span[0], int) else 0
            line = source[:offset].count("\n") + 1 if source else 0
        elif isinstance(span, dict):
            line = span.get("start", {}).get("line", 0)

        if check:
            echoes.append(Echo(check=check, line=line, message=str(message)))

    return echoes
