"""Custom check: banned patterns and obsolete terms from ecko.yaml."""

from __future__ import annotations

import bisect
import fnmatch
import os

from checks.regex_utils import safe_regex_compile, safe_regex_finditer
from checks.result import Echo


def check_banned_patterns(
    file_path: str, patterns: list[dict[str, str]], cwd: str = ""
) -> list[Echo]:
    """Check file against banned regex patterns from config."""
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
    except (OSError, UnicodeDecodeError):
        return []

    if not source:
        return []

    # Build line-start offset table for bisect lookup
    line_starts = [0]
    for i, ch in enumerate(source):
        if ch == "\n":
            line_starts.append(i + 1)

    basename = os.path.basename(file_path)
    rel_path = ""
    if cwd:
        try:
            rel_path = os.path.relpath(file_path, cwd).replace(os.sep, "/")
        except ValueError:
            pass
    echoes: list[Echo] = []

    for rule in patterns:
        pattern_str = rule.get("pattern", "")
        glob_filter = rule.get("glob", "")
        message = rule.get("message", f"Banned pattern `{pattern_str}` found.")

        if not pattern_str:
            continue

        # Apply glob filter if specified (match against basename and relative path)
        if glob_filter:
            if not fnmatch.fnmatch(basename, glob_filter) and not (
                rel_path and fnmatch.fnmatch(rel_path, glob_filter)
            ):
                continue

        regex = safe_regex_compile(pattern_str)
        if regex is None:
            continue

        # One thread per pattern via finditer (not per line)
        matches = safe_regex_finditer(regex, source)
        for m in matches:
            line_num = bisect.bisect_right(line_starts, m.start())
            echoes.append(
                Echo(
                    check="banned-pattern",
                    line=line_num,
                    message=message,
                )
            )

    return echoes


def check_obsolete_terms(file_path: str, terms: list[dict[str, str]]) -> list[Echo]:
    """Check file for obsolete terms that should be renamed."""
    try:
        with open(file_path, encoding="utf-8") as f:
            lines = f.readlines()
    except (OSError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []

    for term in terms:
        old = term.get("old", "")
        new = term.get("new", "")
        if not old:
            continue

        for line_num, line in enumerate(lines, 1):
            if old in line:
                echoes.append(
                    Echo(
                        check="obsolete-term",
                        line=line_num,
                        message=f"Obsolete term `{old}` found.",
                        suggestion=f"Rename to `{new}`.",
                    )
                )

    return echoes
