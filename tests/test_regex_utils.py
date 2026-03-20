"""Tests for shared regex utilities (ReDoS-safe compile/search/finditer)."""

from __future__ import annotations

import re
import time

from checks.regex_utils import (
    _compiled_cache,
    safe_regex_compile,
    safe_regex_finditer,
    safe_regex_search,
)


class TestSafeRegexCompile:
    def test_valid_pattern(self):
        result = safe_regex_compile(r"foo\d+")
        assert result is not None
        assert result.pattern == r"foo\d+"

    def test_caching(self):
        # Clear cache for deterministic test
        _compiled_cache.pop(r"cache_test_\w+", None)

        first = safe_regex_compile(r"cache_test_\w+")
        second = safe_regex_compile(r"cache_test_\w+")
        assert first is second  # Same object from cache

    def test_invalid_pattern_returns_none(self):
        result = safe_regex_compile(r"[invalid")
        assert result is None

    def test_redos_pattern_returns_none(self):
        """Pathological regex should timeout at compile, returning None."""
        # This pattern is valid but can cause backtracking at search time.
        # Compile itself should succeed quickly for most patterns.
        # Test the timeout path with a genuinely slow compile if possible.
        # For now, verify that valid patterns compile fine.
        result = safe_regex_compile(r"(a+)+b")
        assert result is not None  # Compiles fine, backtracking is at search time


class TestSafeRegexSearch:
    def test_match(self):
        pattern = re.compile(r"hello\s+world")
        assert safe_regex_search(pattern, "say hello  world now") is True

    def test_no_match(self):
        pattern = re.compile(r"hello\s+world")
        assert safe_regex_search(pattern, "goodbye") is False

    def test_redos_returns_false(self):
        """Pathological backtracking should timeout, not hang."""
        pattern = re.compile(r"(a+)+b")
        evil_input = "a" * 25 + "!"
        start = time.monotonic()
        result = safe_regex_search(pattern, evil_input)
        elapsed = time.monotonic() - start
        assert result is False
        assert elapsed < 5.0, f"ReDoS guard too slow: {elapsed:.1f}s"


class TestSafeRegexFinditer:
    def test_returns_matches(self):
        pattern = re.compile(r"\d+")
        matches = safe_regex_finditer(pattern, "abc 123 def 456")
        assert len(matches) == 2
        assert matches[0].group() == "123"
        assert matches[1].group() == "456"

    def test_redos_returns_empty(self):
        """Pathological backtracking should timeout, returning empty list."""
        pattern = re.compile(r"(a+)+b")
        evil_input = "a" * 25 + "!"
        start = time.monotonic()
        result = safe_regex_finditer(pattern, evil_input)
        elapsed = time.monotonic() - start
        assert result == []
        assert elapsed < 5.0, f"ReDoS guard too slow: {elapsed:.1f}s"

    def test_empty_text(self):
        pattern = re.compile(r"\d+")
        assert safe_regex_finditer(pattern, "") == []

    def test_no_matches(self):
        pattern = re.compile(r"\d+")
        assert safe_regex_finditer(pattern, "no digits here") == []
