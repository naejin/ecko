"""Bash command guard — blocks dangerous shell commands."""

from __future__ import annotations

import sys

from checks.config import get_blocked_commands, load_config
from checks.regex_utils import safe_regex_compile, safe_regex_search
from checks.result import emit

# Optional full-path prefix: /bin/rm, /usr/bin/rm, \rm, command rm
_RM_PREFIX = r"(?:/(?:usr/)?(?:s?bin)/|\\|command\s+)?"

# Hardcoded patterns (always active, truly dangerous)
_HARDCODED_PATTERNS = [
    {
        "pattern": r"git\b.*--no-verify",
        "message": "Blocked: --no-verify skips hooks \u2014 remove it to let ecko checks run",
    },
    {
        "pattern": _RM_PREFIX
        + r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*\s+/(\s|$|;|&|\|)",
        "message": "Blocked: rm -rf / is catastrophic \u2014 specify a subdirectory",
    },
    {
        "pattern": _RM_PREFIX
        + r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*\s+~(\s|$|;|&|\||/)",
        "message": "Blocked: rm -rf ~ would delete the home directory",
    },
    {
        "pattern": r"git\b(?!.*--force-with-lease).*\bpush\b.*(?:--force|-f)(\s|$|;|&|\|)",
        "message": "Blocked: use --force-with-lease instead of --force to prevent overwriting remote work",
    },
    {
        "pattern": r"git\b.*\breset\s+--hard(\s|$|;|&|\|)",
        "message": "Blocked: git reset --hard discards commits permanently \u2014 use git stash or git revert instead",
    },
    {
        "pattern": r"git\b.*\bclean\s+-[a-zA-Z]*f[a-zA-Z]*(\s|$|;|&|\|)",
        "message": "Blocked: git clean -f deletes untracked files permanently \u2014 review with git clean -n first",
    },
]


def check_bash_command(
    command: str, user_patterns: list[dict[str, str]]
) -> str | None:
    """Check a bash command against blocked patterns.

    Returns the block message if the command should be blocked, or None if allowed.
    """
    for entry in _HARDCODED_PATTERNS + user_patterns:
        pattern_str = entry.get("pattern", "")
        message = entry.get("message", "Command blocked by ecko")
        if not pattern_str:
            continue
        compiled = safe_regex_compile(pattern_str)
        if compiled is None:
            continue
        if safe_regex_search(compiled, command):
            return message
    return None


def run_pre_tool_use_bash(cwd: str) -> int:
    """Check a bash command from stdin and block if it matches dangerous patterns.

    Exit 2 = block (PreToolUse convention), exit 0 = allow.
    """
    command = sys.stdin.read().strip()
    if not command:
        return 0

    config = load_config(cwd)
    user_patterns = get_blocked_commands(config)
    result = check_bash_command(command, user_patterns)

    if result:
        emit(f"~~ ecko ~~ {result}\n")
        return 2
    return 0
