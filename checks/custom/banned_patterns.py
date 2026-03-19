"""Custom check: banned patterns and obsolete terms from ecko.yaml."""

from __future__ import annotations

import fnmatch
import os
import re

from checks.result import Echo


def check_banned_patterns(
    file_path: str, patterns: list[dict[str, str]]
) -> list[Echo]:
    """Check file against banned regex patterns from config."""
    try:
        with open(file_path) as f:
            lines = f.readlines()
    except (OSError, UnicodeDecodeError):
        return []

    basename = os.path.basename(file_path)
    echoes: list[Echo] = []

    for rule in patterns:
        pattern_str = rule.get("pattern", "")
        glob_filter = rule.get("glob", "")
        message = rule.get("message", f"Banned pattern `{pattern_str}` found.")

        if not pattern_str:
            continue

        # Apply glob filter if specified
        if glob_filter and not fnmatch.fnmatch(basename, glob_filter):
            continue

        try:
            regex = re.compile(pattern_str)
        except re.error:
            continue

        for line_num, line in enumerate(lines, 1):
            if regex.search(line):
                echoes.append(
                    Echo(
                        check="banned-pattern",
                        line=line_num,
                        message=message,
                    )
                )

    return echoes


def check_obsolete_terms(
    file_path: str, terms: list[dict[str, str]]
) -> list[Echo]:
    """Check file for obsolete terms that should be renamed."""
    try:
        with open(file_path) as f:
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
