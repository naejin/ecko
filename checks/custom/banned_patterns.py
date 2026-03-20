"""Custom check: banned patterns and obsolete terms from ecko.yaml."""

from __future__ import annotations

import fnmatch
import os
import re
import threading

from checks.result import Echo


def _safe_regex_search(pattern: re.Pattern[str], text: str) -> bool:
    """Run regex search with a timeout to guard against ReDoS."""
    result: list[bool] = [False]

    def _search() -> None:
        try:
            result[0] = bool(pattern.search(text))
        except re.error:
            result[0] = False

    t = threading.Thread(target=_search, daemon=True)
    t.start()
    t.join(timeout=0.5)
    return False if t.is_alive() else result[0]


_compiled_cache: dict[str, re.Pattern[str] | None] = {}


def _safe_regex_compile(pattern_str: str, timeout_ms: int = 500) -> re.Pattern[str] | None:
    """Compile a regex with a timeout to guard against ReDoS at compile time.

    Results are cached by pattern string — each unique pattern is compiled at most once.
    """
    if pattern_str in _compiled_cache:
        return _compiled_cache[pattern_str]

    result: list[re.Pattern[str] | None] = [None]

    def _compile() -> None:
        try:
            result[0] = re.compile(pattern_str)
        except re.error:
            result[0] = None

    t = threading.Thread(target=_compile, daemon=True)
    t.start()
    t.join(timeout=timeout_ms / 1000.0)
    compiled = None if t.is_alive() else result[0]
    _compiled_cache[pattern_str] = compiled
    return compiled


def check_banned_patterns(
    file_path: str, patterns: list[dict[str, str]], cwd: str = ""
) -> list[Echo]:
    """Check file against banned regex patterns from config."""
    try:
        with open(file_path, encoding="utf-8") as f:
            lines = f.readlines()
    except (OSError, UnicodeDecodeError):
        return []

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

        regex = _safe_regex_compile(pattern_str)
        if regex is None:
            continue

        for line_num, line in enumerate(lines, 1):
            if _safe_regex_search(regex, line):
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
