"""Biome adapter — run biome and parse JSON output into Echoes."""

from __future__ import annotations

import json
import os
import subprocess

from checks.result import Echo, emit
from checks.tools.resolve import resolve_node_tool

# Map biome rule names to ecko check names
RULE_MAP = {
    "noUnusedImports": "unused-imports",
    "noUnreachable": "unreachable-code",
    "noDebugger": "debugger-statements",
    "noVar": "var-declarations",
    "noDuplicateObjectKeys": "duplicate-keys",
    "noEmptyBlockStatements": "empty-block-statements",
    "noUselessCatch": "useless-catch",
}


def _find_project_biome_config(start_dir: str) -> str | None:
    """Walk up from start_dir looking for biome.json or biome.jsonc."""
    d = os.path.abspath(start_dir)
    for _ in range(20):  # depth limit
        for name in ("biome.json", "biome.jsonc"):
            candidate = os.path.join(d, name)
            if os.path.isfile(candidate):
                return candidate
        parent = os.path.dirname(d)
        if parent == d:
            break
        d = parent
    return None


def _to_kebab(name: str) -> str:
    """Convert camelCase biome rule name to kebab-case ecko check name.

    Examples: noUnusedImports -> no-unused-imports, noVar -> no-var
    """
    result: list[str] = []
    for ch in name:
        if ch.isupper() and result:
            result.append("-")
        result.append(ch.lower())
    return "".join(result)


def run_biome(
    file_path: str,
    plugin_root: str,
    use_project_config: bool = False,
) -> list[Echo]:
    """Run biome on a file and return echoes."""
    cmd = resolve_node_tool("biome", package="@biomejs/biome")
    if not cmd:
        return []

    cwd = os.path.dirname(os.path.abspath(file_path))
    has_project_config = use_project_config and _find_project_biome_config(cwd)

    if use_project_config and not has_project_config:
        emit(
            "~~ ecko ~~ note: biome_use_project_config enabled"
            " but no biome.json/biome.jsonc found — using ecko config\n"
        )

    if has_project_config:
        run_cmd = [*cmd, "lint", "--reporter=json", file_path]
    else:
        config_path = os.path.normpath(os.path.join(plugin_root, "config"))
        run_cmd = [
            *cmd, "lint", "--config-path", config_path, "--reporter=json", file_path,
        ]

    try:
        result = subprocess.run(
            run_cmd,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        emit(f"~~ ecko ~~ warning: biome timed out on {file_path} (30s limit)\n")
        return []
    except OSError as exc:
        emit(f"~~ ecko ~~ warning: biome failed: {exc}\n")
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
        # biome category format: "lint/group/ruleName"
        if "/" in category:
            rule_name = category.rsplit("/", 1)[-1]

        check = RULE_MAP.get(rule_name)
        if not check:
            if use_project_config and rule_name:
                # Unknown rules get kebab-case name when using project config
                check = _to_kebab(rule_name)
            else:
                continue

        message = diag.get("description", "") or diag.get("message", "")
        # Extract line number from location
        line = 0
        location = diag.get("location", {})
        # v2 format: location.start.line / location.start.column
        start = location.get("start")
        if isinstance(start, dict):
            line = start.get("line", 0)
        else:
            # v1 fallback: span-based
            span = location.get("span", {})
            if isinstance(span, list) and span:
                source = location.get("sourceCode", "")
                offset = span[0] if isinstance(span[0], int) else 0
                line = source[:offset].count("\n") + 1 if source else 0
            elif isinstance(span, dict):
                line = span.get("start", {}).get("line", 0)

        diag_severity = diag.get("severity", "warning")
        severity = "error" if diag_severity == "error" else "warn"
        echoes.append(Echo(check=check, line=line, message=str(message), severity=severity))

    return echoes
