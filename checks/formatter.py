"""Layer 1: Silent auto-fix — formatters and trailing whitespace removal."""

from __future__ import annotations

import shutil
import subprocess
from typing import Any

from checks.config import is_autofix_enabled


def autofix(file_path: str, lang: str, config: dict[str, Any]) -> None:
    """Run auto-fix tools on the file. Modifies in-place, no output."""
    if lang == "python":
        if is_autofix_enabled(config, "black"):
            _run_if_available("black", "--quiet", file_path)
        if is_autofix_enabled(config, "isort"):
            _run_if_available("isort", "--quiet", "--profile", "black", file_path)
    elif lang in ("typescript", "javascript", "css", "json"):
        if is_autofix_enabled(config, "prettier"):
            _run_if_available("prettier", "--write", "--log-level", "silent", file_path)

    # Always strip trailing whitespace (no dependency needed)
    _strip_trailing_whitespace(file_path)


def _run_if_available(tool: str, *args: str) -> None:
    """Run a tool if it's on PATH. Silently skip if not found."""
    path = shutil.which(tool)
    if not path:
        return
    try:
        subprocess.run(
            [path, *args],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (subprocess.TimeoutExpired, OSError):
        pass


def _strip_trailing_whitespace(file_path: str) -> None:
    """Remove trailing whitespace from each line in the file."""
    try:
        with open(file_path, "r") as f:
            original = f.read()
        lines = original.splitlines(True)
        cleaned = []
        for line in lines:
            if line.endswith("\n"):
                cleaned.append(line[: -1].rstrip() + "\n")
            else:
                cleaned.append(line.rstrip())
        result = "".join(cleaned)
        if result != original:
            with open(file_path, "w") as f:
                f.write(result)
    except (OSError, UnicodeDecodeError):
        pass
