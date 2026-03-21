"""Ruff adapter — run ruff and parse JSON output into Echoes."""

from __future__ import annotations

import json
import re
import subprocess

from checks.result import Echo, emit
from checks.tools.resolve import resolve_python_tool

# Ruff rules we check
RUFF_RULES = "F401,E711,E712,E722,F403,B006,A001,A002"

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
}
# Note: S110 (empty-error-handlers / try-except-pass) removed in v0.9.1.
# E722 (bare-except) already catches the dangerous case. try/except-pass with
# a named exception is a legitimate guard pattern. Users can re-enable via
# ruff_extra_rules: [S110]


# Codes that get error severity (safety-critical)
_ERROR_CODES = frozenset({"E722", "F403"})

# Ruff A001/A002 message format: Variable `type` is shadowing a Python builtin
_SHADOW_NAME_RE = re.compile(r"`(\w+)` is shadowing")


_extra_rules_warned = False


def run_ruff(
    file_path: str,
    builtin_shadow_allowlist: frozenset[str] | None = None,
    extra_rules: list[str] | None = None,
    use_project_config: bool = False,
) -> list[Echo]:
    """Run ruff on a file and return echoes."""
    global _extra_rules_warned

    cmd = resolve_python_tool("ruff")
    if not cmd:
        return []

    if use_project_config:
        # Defer to project's ruff.toml / pyproject.toml [tool.ruff].
        # Always pass --no-fix (safety: project fix=true would create infinite loop).
        if extra_rules and not _extra_rules_warned:
            _extra_rules_warned = True
            emit(
                "~~ ecko ~~ note: ruff_extra_rules ignored when"
                " ruff_use_project_config is enabled\n"
            )
        run_cmd = [*cmd, "check", "--output-format", "json", "--no-fix", file_path]
    else:
        run_cmd = [
            *cmd,
            "check",
            "--select",
            RUFF_RULES + ("," + ",".join(extra_rules) if extra_rules else ""),
            "--output-format",
            "json",
            "--no-fix",
            file_path,
        ]

    try:
        result = subprocess.run(
            run_cmd,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        emit(f"~~ ecko ~~ warning: ruff timed out on {file_path} (30s limit)\n")
        return []
    except OSError as exc:
        emit(f"~~ ecko ~~ warning: ruff failed: {exc}\n")
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
        check = RULE_MAP.get(code) or code.lower()
        line = v.get("location", {}).get("row", 0)
        message = v.get("message", "")

        # Filter builtin-shadowing by allowlist + dunder skip
        if check == "builtin-shadowing":
            m = _SHADOW_NAME_RE.search(message)
            if m:
                name = m.group(1)
                # Dunder-prefixed params are intentional API design, not accidental
                if name.startswith("__") and name.endswith("__"):
                    continue
                if builtin_shadow_allowlist is not None and name in builtin_shadow_allowlist:
                    continue

        severity = "error" if code in _ERROR_CODES else "warn"
        echoes.append(Echo(check=check, line=line, message=message, severity=severity))

    return echoes
