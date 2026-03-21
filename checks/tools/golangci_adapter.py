"""golangci-lint adapter — run golangci-lint and parse JSON output into Echoes."""

from __future__ import annotations

import json
import os
import subprocess

from checks.result import Echo, emit
from checks.tools.resolve import resolve_binary_tool


def run_golangci(
    cwd: str, modified_files: list[str] | None = None
) -> dict[str, list[Echo]]:
    """Run golangci-lint on a Go project. Returns echoes grouped by file.

    Layer 3 only — runs at project level, not per-file.
    """
    cmd = resolve_binary_tool("golangci-lint")
    if not cmd:
        return {}

    try:
        result = subprocess.run(
            [*cmd, "run", "--out-format", "json", "./..."],
            capture_output=True,
            text=True,
            timeout=120,
            cwd=cwd,
        )
    except subprocess.TimeoutExpired:
        emit("~~ ecko ~~ warning: golangci-lint timed out (120s limit)\n")
        return {}
    except OSError as exc:
        emit(f"~~ ecko ~~ warning: golangci-lint failed: {exc}\n")
        return {}

    output = result.stdout.strip()
    if not output:
        if result.returncode != 0 and result.stderr and result.stderr.strip():
            emit(f"~~ ecko ~~ warning: golangci-lint: {result.stderr.strip()[:200]}\n")
        return {}

    try:
        data = json.loads(output)
    except json.JSONDecodeError:
        return {}

    issues = data.get("Issues") or []
    if not issues:
        return {}

    # Build modified file set for post-filtering
    modified_set: set[str] | None = None
    if modified_files:
        modified_set = {os.path.normpath(os.path.abspath(f)) for f in modified_files}

    file_echoes: dict[str, list[Echo]] = {}
    for issue in issues:
        pos = issue.get("Pos", {})
        rel_path = pos.get("Filename", "")
        if not rel_path:
            continue
        abs_path = os.path.normpath(os.path.join(cwd, rel_path))

        # Post-filter to modified files
        if modified_set and abs_path not in modified_set:
            continue

        line = pos.get("Line", 0)
        message = issue.get("Text", "")
        linter = issue.get("FromLinter", "unknown")
        check = f"go-{linter}"
        issue_severity = issue.get("Severity", "warning")
        severity = "error" if issue_severity == "error" else "warn"

        file_echoes.setdefault(abs_path, []).append(
            Echo(check=check, line=line, message=message, severity=severity)
        )

    return file_echoes
