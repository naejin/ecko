"""Knip adapter — run knip and parse JSON output into Echoes."""

from __future__ import annotations

import json
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_node_tool


def run_knip(cwd: str) -> dict[str, list[Echo]]:
    """Run knip for unused exports/imports detection. Returns echoes grouped by file."""
    cmd = resolve_node_tool("knip")
    if not cmd:
        return {}

    try:
        result = subprocess.run(
            [*cmd, "--reporter", "json"],
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

    # knip JSON structure varies by version; handle common formats
    # Format: { files: [...], issues: [...] } or { "unused-exports": [...], ... }
    for category in ("files", "unlisted", "exports", "types", "duplicates"):
        items = data.get(category, [])
        if not isinstance(items, list):
            continue
        for item in items:
            if isinstance(item, str):
                # Unused file
                file_echoes.setdefault(item, []).append(
                    Echo(
                        check="unused-file",
                        line=1,
                        message=f"File appears to be unused ({category}).",
                        suggestion="Remove it or add it to knip config.",
                    )
                )
            elif isinstance(item, dict):
                path = item.get("file", item.get("filePath", ""))
                name = item.get("name", item.get("symbol", ""))
                line = item.get("line", item.get("row", 1))
                if path:
                    msg = f"Unused {category.rstrip('s')}"
                    if name:
                        msg += f" `{name}`"
                    file_echoes.setdefault(path, []).append(
                        Echo(
                            check=f"unused-{category.rstrip('s')}",
                            line=int(line) if line else 1,
                            message=f"{msg}.",
                            suggestion="Remove it.",
                        )
                    )

    return file_echoes
