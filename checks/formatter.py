"""Layer 1: Silent auto-fix — formatters and trailing whitespace removal."""

from __future__ import annotations

import subprocess
from typing import Any

from checks.config import is_autofix_enabled
from checks.tools.resolve import resolve_node_tool, resolve_python_tool


def autofix(file_path: str, lang: str, config: dict[str, Any]) -> None:
    """Run auto-fix tools on the file. Modifies in-place, no output."""
    if lang == "python":
        if is_autofix_enabled(config, "black"):
            _run_tool(resolve_python_tool("black"), "--quiet", file_path)
        if is_autofix_enabled(config, "isort"):
            _run_tool(
                resolve_python_tool("isort"), "--quiet", "--profile", "black", file_path
            )
    elif lang in ("typescript", "javascript", "css", "json"):
        if is_autofix_enabled(config, "prettier"):
            _run_tool(
                resolve_node_tool("prettier"),
                "--write",
                "--log-level",
                "silent",
                file_path,
            )

    # Always strip trailing whitespace (no dependency needed)
    _strip_trailing_whitespace(file_path)


def _run_tool(cmd: list[str] | None, *args: str) -> None:
    """Run a resolved tool command. Silently skip if not available."""
    if not cmd:
        return
    try:
        subprocess.run(
            [*cmd, *args],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (subprocess.TimeoutExpired, OSError):
        pass


def _strip_trailing_whitespace(file_path: str) -> None:
    """Remove trailing whitespace from each line in the file."""
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            original = f.read()
        lines = original.splitlines(True)
        cleaned = []
        for line in lines:
            if line.endswith("\n"):
                cleaned.append(line[:-1].rstrip() + "\n")
            else:
                cleaned.append(line.rstrip())
        result = "".join(cleaned)
        if result != original:
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(result)
    except (OSError, UnicodeDecodeError):
        pass
