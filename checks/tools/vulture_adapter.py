"""Vulture adapter — run vulture and parse output into Echoes."""

from __future__ import annotations

import os
import re
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_python_tool

# vulture output: path:line: unused function 'name' (80% confidence)
VULTURE_PATTERN = re.compile(r"^(.+?):(\d+):\s+(.+?)\s+\((\d+)% confidence\)$")

# Protocol params — always framework-injected, never genuinely unused.
_ALWAYS_SKIP = {
    "exc_type", "exc_val", "exc_tb", "exc_info",  # __exit__ (PEP 343)
    "signum", "frame",                              # signal handlers
}

# pytest built-in fixtures — only skip in test/conftest files.
_PYTEST_SKIP = {
    "tmp_path", "tmp_path_factory", "capsys", "capfd", "caplog",
    "monkeypatch", "pytestconfig", "recwarn", "tmpdir", "tmpdir_factory",
}

# Extracts name from "unused variable 'foo'" / "unused argument 'foo'"
_NAME_RE = re.compile(r"unused (?:variable|argument|parameter) '(\w+)'")

# Extracts name from "unused function 'foo'" / "unused method 'foo'"
_FUNC_RE = re.compile(r"unused (?:function|method) '(\w+)'")


def run_vulture(cwd: str) -> dict[str, list[Echo]]:
    """Run vulture with 80% confidence threshold. Returns echoes grouped by file."""
    cmd = resolve_python_tool("vulture")
    if not cmd:
        return {}

    try:
        result = subprocess.run(
            [*cmd, ".", "--min-confidence", "80"],
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
            name_match = _NAME_RE.search(message)
            if name_match:
                name = name_match.group(1)
                if name in _ALWAYS_SKIP:
                    continue
                # Also suppresses genuine unused variables with fixture names
                # in test files — vulture can't distinguish fixture injection
                # from dead code, so we accept the false negative here.
                if name in _PYTEST_SKIP:
                    basename = os.path.basename(path)
                    if (basename.startswith("test_") or basename.endswith("_test.py")
                            or basename in ("conftest.py", "conftest.pyi")):
                        continue
            # Fixture definitions in test/conftest files (reported as "unused function")
            func_match = _FUNC_RE.search(message)
            if func_match and func_match.group(1) in _PYTEST_SKIP:
                basename = os.path.basename(path)
                if (basename.startswith("test_") or basename.endswith("_test.py")
                        or basename in ("conftest.py", "conftest.pyi")):
                    continue
            file_echoes.setdefault(path, []).append(
                Echo(
                    check="dead-code",
                    line=lineno,
                    message=message,
                    suggestion="Remove it if truly unused.",
                )
            )

    return file_echoes
