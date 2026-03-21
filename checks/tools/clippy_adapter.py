"""Clippy adapter — run cargo clippy and parse streaming JSON output into Echoes."""

from __future__ import annotations

import json
import os
import subprocess

from checks.result import Echo, emit
from checks.tools.resolve import resolve_binary_tool


def run_clippy(
    cwd: str, modified_files: list[str] | None = None
) -> dict[str, list[Echo]]:
    """Run cargo clippy on a Rust project. Returns echoes grouped by file.

    Layer 3 only — runs at project level, not per-file.
    Streaming JSON: one JSON object per line.
    """
    cmd = resolve_binary_tool("cargo")
    if not cmd:
        return {}

    # Gate on Cargo.toml
    if not os.path.isfile(os.path.join(cwd, "Cargo.toml")):
        return {}

    try:
        result = subprocess.run(
            [*cmd, "clippy", "--message-format=json", "--", "-W", "clippy::all"],
            capture_output=True,
            text=True,
            timeout=120,
            cwd=cwd,
        )
    except subprocess.TimeoutExpired:
        emit("~~ ecko ~~ warning: clippy timed out (120s limit)\n")
        return {}
    except OSError as exc:
        emit(f"~~ ecko ~~ warning: clippy failed: {exc}\n")
        return {}

    # Build modified file set for post-filtering
    modified_set: set[str] | None = None
    if modified_files:
        modified_set = {os.path.normpath(os.path.abspath(f)) for f in modified_files}

    file_echoes: dict[str, list[Echo]] = {}

    # Streaming JSON: one JSON object per stdout line
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue

        if obj.get("reason") != "compiler-message":
            continue

        msg = obj.get("message", {})
        code_info = msg.get("code")
        if not code_info:
            continue
        code = code_info.get("code", "")
        if not code:
            continue

        text = msg.get("message", "")
        spans = msg.get("spans", [])
        if not spans:
            continue

        # Use the primary span (clippy marks it with is_primary=true)
        span = next((s for s in spans if s.get("is_primary")), spans[0])
        file_name = span.get("file_name", "")
        if not file_name:
            continue
        abs_path = os.path.normpath(os.path.join(cwd, file_name))

        # Post-filter to modified files
        if modified_set and abs_path not in modified_set:
            continue

        line_num = span.get("line_start", 0)
        check = f"rust-{code}"
        level = msg.get("level", "warning")
        severity = "error" if level == "error" else "warn"

        file_echoes.setdefault(abs_path, []).append(
            Echo(check=check, line=line_num, message=text, severity=severity)
        )

    return file_echoes
