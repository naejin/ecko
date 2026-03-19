"""Custom check: unicode artifacts from LLM output or copy-paste."""

from __future__ import annotations

import io
import os
import re
import tokenize

from checks.result import Echo

# Characters that shouldn't appear in source code
ARTIFACTS = {
    "\u2014": "Em dash (\u2014)",
    "\u2013": "En dash (\u2013)",
    "\u201c": "Left double smart quote (\u201c)",
    "\u201d": "Right double smart quote (\u201d)",
    "\u2018": "Left single smart quote (\u2018)",
    "\u2019": "Right single smart quote (\u2019)",
    "\u200b": "Zero-width space",
    "\u200c": "Zero-width non-joiner",
    "\u200d": "Zero-width joiner",
    "\ufeff": "Byte order mark",
    "\u00a0": "Non-breaking space",
    "\u2003": "Em space",
    "\u2002": "En space",
}

# Pattern matching any artifact character
ARTIFACT_PATTERN = re.compile("[" + "".join(ARTIFACTS.keys()) + "]")

# Python token types to skip (strings & comments).
# Python 3.12+ tokenizes f-strings as FSTRING_START/MIDDLE/END instead of
# a single STRING token, so we include those when available.
_PY_SKIP_TOKENS = {tokenize.STRING, tokenize.COMMENT}
for _attr in ("FSTRING_START", "FSTRING_MIDDLE", "FSTRING_END"):
    _tok_type = getattr(tokenize, _attr, None)
    if _tok_type is not None:
        _PY_SKIP_TOKENS.add(_tok_type)


# Prose extensions where em dashes, smart quotes, etc. are normal punctuation.
_PROSE_EXTENSIONS = {".md", ".txt", ".rst", ".adoc", ".rdoc"}

# JS/TS family — use the JS string/comment scanner.
_JS_EXTENSIONS = {".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".css", ".json", ".jsonc"}


def check_unicode_artifacts(file_path: str) -> list[Echo]:
    """Scan file for unicode artifacts, skipping string literals and comments."""
    _, ext = os.path.splitext(file_path)
    if ext.lower() in _PROSE_EXTENSIONS:
        return []

    try:
        with open(file_path) as f:
            source = f.read()
    except (OSError, UnicodeDecodeError):
        return []

    if not ARTIFACT_PATTERN.search(source):
        return []

    lines = source.splitlines()
    # Determine which line regions are strings/comments to skip
    skip_regions = _get_skip_regions(file_path, source)

    echoes: list[Echo] = []
    seen_lines: set[int] = set()

    for line_num, line in enumerate(lines, 1):
        for match in ARTIFACT_PATTERN.finditer(line):
            col = match.start()
            if _in_skip_region(line_num, col, skip_regions):
                continue
            if line_num in seen_lines:
                continue
            seen_lines.add(line_num)
            char = match.group()
            name = ARTIFACTS.get(char, f"Unicode U+{ord(char):04X}")
            echoes.append(
                Echo(
                    check="unicode-artifact",
                    line=line_num,
                    message=f"{name} found in source code. Likely from copy-pasting LLM output.",
                    suggestion="Replace with the ASCII equivalent.",
                )
            )

    return echoes


def _get_skip_regions(file_path: str, source: str) -> list[tuple[int, int, int, int]]:
    """Get regions (start_line, start_col, end_line, end_col) that are strings or comments.

    Uses Python tokenizer for .py files, simple heuristics for others.
    """
    regions: list[tuple[int, int, int, int]] = []

    lp = file_path.lower()
    if lp.endswith(".py") or lp.endswith(".pyi"):
        try:
            tokens = tokenize.generate_tokens(io.StringIO(source).readline)
            for tok in tokens:
                if tok.type in _PY_SKIP_TOKENS:
                    regions.append((tok.start[0], tok.start[1], tok.end[0], tok.end[1]))
        except tokenize.TokenError:
            pass
    else:
        _, ext = os.path.splitext(lp)
        if ext in _JS_EXTENSIONS:
            regions = _scan_js_skip_regions(source)
        else:
            regions = _scan_hash_skip_regions(source)

    return regions


def _scan_js_skip_regions(source: str) -> list[tuple[int, int, int, int]]:
    """Scan JS/TS/CSS/JSON source for string literals and comments.

    Returns (start_line, start_col, end_line, end_col) regions to skip.
    Handles: // line comments, /* block comments */, and string literals
    with single, double, and backtick (template literal) delimiters.
    """
    regions: list[tuple[int, int, int, int]] = []
    line_offsets: list[int] = [0]
    for i, c in enumerate(source):
        if c == '\n':
            line_offsets.append(i + 1)

    def pos_to_lc(pos: int) -> tuple[int, int]:
        """Convert flat offset to (1-based line, 0-based col)."""
        lo, hi = 0, len(line_offsets) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if line_offsets[mid] <= pos:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1, pos - line_offsets[lo]

    i = 0
    n = len(source)
    while i < n:
        c = source[i]

        # Single-line comment
        if c == "/" and i + 1 < n and source[i + 1] == "/":
            start = i
            while i < n and source[i] != "\n":
                i += 1
            sl, sc = pos_to_lc(start)
            el, ec = pos_to_lc(i)
            regions.append((sl, sc, el, ec))
            continue

        # Block comment
        if c == "/" and i + 1 < n and source[i + 1] == "*":
            start = i
            i += 2
            while i + 1 < n and not (source[i] == "*" and source[i + 1] == "/"):
                i += 1
            if i + 1 < n:
                i += 2  # skip */
            sl, sc = pos_to_lc(start)
            el, ec = pos_to_lc(min(i, n))
            regions.append((sl, sc, el, ec))
            continue

        # String literals
        if c in ('"', "'", "`"):
            start = i
            quote = c
            i += 1
            while i < n:
                if source[i] == "\\" and i + 1 < n:
                    i += 2  # skip escaped char
                elif source[i] == quote:
                    i += 1
                    break
                elif quote != "`" and source[i] == "\n":
                    break  # unterminated single-line string
                else:
                    i += 1
            sl, sc = pos_to_lc(start)
            el, ec = pos_to_lc(min(i, n))
            regions.append((sl, sc, el, ec))
            continue

        i += 1

    return regions


def _scan_hash_skip_regions(source: str) -> list[tuple[int, int, int, int]]:
    """Scan source for #-style line comments and basic string literals.

    Used for shell scripts, YAML, TOML, and other non-JS/non-Python files.
    """
    regions: list[tuple[int, int, int, int]] = []
    for line_num, line in enumerate(source.splitlines(), 1):
        i = 0
        n = len(line)
        while i < n:
            c = line[i]
            # String literal — skip to avoid treating # inside strings as comments
            if c in ('"', "'"):
                start = i
                quote = c
                i += 1
                while i < n:
                    if line[i] == "\\" and i + 1 < n:
                        i += 2
                    elif line[i] == quote:
                        i += 1
                        break
                    else:
                        i += 1
                regions.append((line_num, start, line_num, i))
                continue
            # Hash comment — rest of line is a comment
            if c == "#":
                regions.append((line_num, i, line_num, n))
                break
            i += 1
    return regions


def _in_skip_region(
    line: int, col: int, regions: list[tuple[int, int, int, int]]
) -> bool:
    """Check if a position is within a skip region."""
    for start_line, start_col, end_line, end_col in regions:
        if start_line == end_line:
            if line == start_line and start_col <= col < end_col:
                return True
        else:
            if line == start_line and col >= start_col:
                return True
            if start_line < line < end_line:
                return True
            if line == end_line and col < end_col:
                return True
    return False
