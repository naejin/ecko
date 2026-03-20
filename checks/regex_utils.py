"""Shared timeout-protected regex utilities.

Guards against ReDoS by running compile/search/finditer in daemon threads
with configurable timeouts. Pure utility — no emit(), no I/O.
"""

from __future__ import annotations

import re
import threading
from typing import TypeVar

_T = TypeVar("_T")

_compiled_cache: dict[str, re.Pattern[str] | None] = {}

_SENTINEL = object()  # distinguishes "timed out" from "returned None"


def _run_with_timeout(fn: object, default: _T, timeout_ms: int) -> tuple[bool, _T]:
    """Run a callable in a daemon thread with a timeout.

    Returns (timed_out, result).  On timeout, result is *default*.
    """
    result = [_SENTINEL]

    def _inner() -> None:
        result[0] = fn()  # type: ignore[operator]

    t = threading.Thread(target=_inner, daemon=True)
    t.start()
    t.join(timeout=timeout_ms / 1000.0)
    if t.is_alive():
        return True, default
    return False, result[0]  # type: ignore[return-value]


def safe_regex_compile(
    pattern_str: str, timeout_ms: int = 500
) -> re.Pattern[str] | None:
    """Compile a regex with a timeout to guard against ReDoS at compile time.

    Results are cached by pattern string — each unique pattern is compiled
    at most once per process.  Returns None on timeout or re.error.
    """
    if pattern_str in _compiled_cache:
        return _compiled_cache[pattern_str]

    def _compile() -> re.Pattern[str] | None:
        try:
            return re.compile(pattern_str)
        except re.error:
            return None

    timed_out, compiled = _run_with_timeout(_compile, None, timeout_ms)
    if not timed_out:
        _compiled_cache[pattern_str] = compiled
    return compiled


def safe_regex_search(
    pattern: re.Pattern[str], text: str, timeout_ms: int = 500
) -> bool:
    """Run regex search with a timeout to guard against ReDoS.

    Accepts a pre-compiled pattern.  Returns False on timeout or error.
    """

    def _search() -> bool:
        try:
            return bool(pattern.search(text))
        except re.error:
            return False

    _, result = _run_with_timeout(_search, False, timeout_ms)
    return result


def safe_regex_finditer(
    pattern: re.Pattern[str], text: str, timeout_ms: int = 500
) -> list[re.Match[str]]:
    """Run regex finditer with a timeout to guard against ReDoS.

    Returns all matches as a list, or [] on timeout or error.
    One thread per call (not per line), so a 500-line file × 3 patterns
    spawns 3 threads instead of 1,500.
    """

    def _finditer() -> list[re.Match[str]]:
        try:
            return list(pattern.finditer(text))
        except re.error:
            return []

    _, result = _run_with_timeout(_finditer, [], timeout_ms)
    return result
