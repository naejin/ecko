#!/usr/bin/env python3
"""Ecko runner — orchestrates Layer 1 (auto-fix), Layer 2 (echoes), and Layer 3 (deep analysis)."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

# Ensure the checks package is importable
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from fnmatch import fnmatch

from checks.config import (
    get_banned_patterns,
    get_disabled_checks,
    get_exclude_patterns,
    get_obsolete_terms,
    is_deep_enabled,
    load_config,
)
from checks.result import Echo, emit, format_file_echoes, format_stop_echoes

# Paths that are almost never worth linting — skip by default.
# Each entry is matched at any depth: "fixtures" matches both
# "tests/fixtures/x.py" and "fixtures/x.py".
_DEFAULT_EXCLUDE_DIRS = [
    "fixtures",
    "__fixtures__",
    "__snapshots__",
    "vendor",
    "node_modules",
    ".git",
    "dist",
    "build",
    "__pycache__",
]

LANG_MAP = {
    ".py": "python",
    ".pyi": "python",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".css": "css",
    ".json": "json",
    ".md": "markdown",
}


def detect_language(file_path: str) -> str:
    ext = Path(file_path).suffix.lower()
    return LANG_MAP.get(ext, "unknown")


def is_excluded(file_path: str, cwd: str, user_excludes: list[str]) -> bool:
    """Check if a file matches any exclude pattern (default + user-configured)."""
    # Use the path relative to cwd for matching
    try:
        rel = os.path.relpath(file_path, cwd)
    except ValueError:
        rel = file_path
    # Normalize to forward slashes for consistent matching
    rel = rel.replace(os.sep, "/")

    # Built-in: skip if any path segment is in _DEFAULT_EXCLUDE_DIRS
    parts = rel.split("/")
    for part in parts[:-1]:  # check directories, not the filename
        if part in _DEFAULT_EXCLUDE_DIRS:
            return True

    # User-configured glob patterns (matched against relative path)
    for pattern in user_excludes:
        if fnmatch(rel, pattern):
            return True
    return False


def _is_standalone_comment(line_text: str) -> bool:
    """Check if a line is a standalone comment (not code with an inline comment).

    Used to decide whether an ecko:ignore on the line above should apply to
    the next line.  Standalone comments (``# ecko:ignore``, ``// ecko:ignore``)
    suppress the line below; inline comments (``import os  # ecko:ignore``)
    suppress only their own line.
    """
    stripped = line_text.lstrip()
    return (
        stripped.startswith("#")
        or stripped.startswith("//")
        or stripped.startswith("/*")
        or stripped.startswith("<!--")
    )


def filter_suppressed(echoes: list[Echo], file_path: str) -> list[Echo]:
    """Remove echoes suppressed by inline ecko:ignore comments."""
    if not echoes or not os.path.isfile(file_path):
        return echoes
    try:
        with open(file_path) as f:
            lines = f.readlines()
    except (OSError, UnicodeDecodeError):
        return echoes

    filtered = []
    for echo in echoes:
        line_idx = echo.line - 1
        suppressed = False
        # Check current line and line above
        for check_idx in (line_idx, line_idx - 1):
            if 0 <= check_idx < len(lines):
                line_text = lines[check_idx]
                if "ecko:ignore" in line_text:
                    # Line-above suppression only applies to standalone
                    # comment lines.  Inline ignores (e.g.
                    # ``import os  # ecko:ignore``) only suppress echoes
                    # on their own line, not the line below.
                    if check_idx == line_idx - 1 and not _is_standalone_comment(
                        line_text
                    ):
                        continue
                    # Check if it's a targeted ignore
                    bracket_start = line_text.find("ecko:ignore[")
                    if bracket_start != -1:
                        bracket_end = line_text.find("]", bracket_start)
                        if bracket_end != -1:
                            checks_str = line_text[
                                bracket_start + len("ecko:ignore[") : bracket_end
                            ]
                            ignored_checks = {c.strip() for c in checks_str.split(",")}
                            if echo.check in ignored_checks:
                                suppressed = True
                    else:
                        # Blanket ignore
                        suppressed = True
        if not suppressed:
            filtered.append(echo)
    return filtered


def run_post_tool_use(file_path: str, cwd: str, plugin_root: str) -> int:
    """Run Layer 1 (auto-fix) then Layer 2 (echoes) on a single file."""
    if not os.path.isfile(file_path):
        return 0

    config = load_config(cwd)
    if is_excluded(file_path, cwd, get_exclude_patterns(config)):
        return 0

    disabled = get_disabled_checks(config)
    lang = detect_language(file_path)

    # --- Layer 1: Auto-fix (silent) ---
    from checks.formatter import autofix

    autofix(file_path, lang, config)

    # --- Layer 2: Echoes ---
    echoes: list[Echo] = []

    # Tool checks
    if lang == "python":
        from checks.tools.ruff_adapter import run_ruff

        echoes.extend(run_ruff(file_path))
    elif lang in ("typescript", "javascript"):
        from checks.tools.biome_adapter import run_biome

        echoes.extend(run_biome(file_path, plugin_root))

    # Custom checks (Python AST)
    if lang == "python":
        from checks.custom.duplicate_keys import check_duplicate_keys
        from checks.custom.unreachable_code import check_unreachable_code

        echoes.extend(check_duplicate_keys(file_path))
        echoes.extend(check_unreachable_code(file_path))

    # Custom checks (universal)
    from checks.custom.unicode_artifacts import check_unicode_artifacts

    echoes.extend(check_unicode_artifacts(file_path))

    # Banned patterns
    banned = get_banned_patterns(config)
    if banned:
        from checks.custom.banned_patterns import check_banned_patterns

        echoes.extend(check_banned_patterns(file_path, banned, cwd))

    # Obsolete terms
    obsolete = get_obsolete_terms(config)
    if obsolete:
        from checks.custom.banned_patterns import check_obsolete_terms

        echoes.extend(check_obsolete_terms(file_path, obsolete))

    # Filter
    echoes = filter_suppressed(echoes, file_path)
    echoes = [e for e in echoes if e.check not in disabled]

    if echoes:
        emit(format_file_echoes(file_path, echoes))
        return 1
    return 0


def _normalize_path(path: str, cwd: str) -> str:
    """Normalize a file path to absolute, resolving relative paths against cwd."""
    if os.path.isabs(path):
        return os.path.normpath(path)
    return os.path.normpath(os.path.join(cwd, path))


def run_stop(cwd: str, plugin_root: str) -> int:
    """Run Layer 3 (deep analysis) + Layer 2 re-sweep on all modified files."""
    config = load_config(cwd)
    disabled = get_disabled_checks(config)
    user_excludes = get_exclude_patterns(config)

    # Find modified files, filtering excluded paths
    modified = [
        f for f in _get_modified_files(cwd) if not is_excluded(f, cwd, user_excludes)
    ]
    if not modified:
        return 0

    py_files = [f for f in modified if detect_language(f) == "python"]
    ts_files = [
        f for f in modified if detect_language(f) in ("typescript", "javascript")
    ]

    all_echoes: dict[str, list[Echo]] = {}

    # --- Layer 3: Deep analysis ---
    # TypeScript type checking
    if ts_files and is_deep_enabled(config, "tsc"):
        tsconfig = os.path.join(cwd, "tsconfig.json")
        if os.path.isfile(tsconfig):
            from checks.tools.tsc_adapter import run_tsc

            for path, echoes in run_tsc(cwd).items():
                all_echoes.setdefault(_normalize_path(path, cwd), []).extend(echoes)

    # Python type checking
    if py_files and is_deep_enabled(config, "pyright"):
        from checks.tools.pyright_adapter import run_pyright

        for path, echoes in run_pyright(py_files, cwd).items():
            all_echoes.setdefault(_normalize_path(path, cwd), []).extend(echoes)

    # Python dead code
    if py_files and is_deep_enabled(config, "vulture"):
        from checks.tools.vulture_adapter import run_vulture

        for path, echoes in run_vulture(cwd).items():
            all_echoes.setdefault(_normalize_path(path, cwd), []).extend(echoes)

    # TypeScript unused exports
    if ts_files and is_deep_enabled(config, "knip"):
        from checks.tools.knip_adapter import run_knip

        for path, echoes in run_knip(cwd).items():
            all_echoes.setdefault(_normalize_path(path, cwd), []).extend(echoes)

    # --- Layer 2: Re-sweep all modified files ---
    for file_path in modified:
        if not os.path.isfile(file_path):
            continue
        lang = detect_language(file_path)
        echoes: list[Echo] = []

        if lang == "python":
            from checks.tools.ruff_adapter import run_ruff

            echoes.extend(run_ruff(file_path))
            from checks.custom.duplicate_keys import check_duplicate_keys
            from checks.custom.unreachable_code import check_unreachable_code

            echoes.extend(check_duplicate_keys(file_path))
            echoes.extend(check_unreachable_code(file_path))
        elif lang in ("typescript", "javascript"):
            from checks.tools.biome_adapter import run_biome

            echoes.extend(run_biome(file_path, plugin_root))

        from checks.custom.unicode_artifacts import check_unicode_artifacts

        echoes.extend(check_unicode_artifacts(file_path))

        banned = get_banned_patterns(config)
        if banned:
            from checks.custom.banned_patterns import check_banned_patterns

            echoes.extend(check_banned_patterns(file_path, banned, cwd))

        obsolete = get_obsolete_terms(config)
        if obsolete:
            from checks.custom.banned_patterns import check_obsolete_terms

            echoes.extend(check_obsolete_terms(file_path, obsolete))

        if echoes:
            all_echoes.setdefault(_normalize_path(file_path, cwd), []).extend(echoes)

    # Deduplicate echoes per file (same check + line)
    for path in all_echoes:
        seen: set[tuple[str, int]] = set()
        deduped: list[Echo] = []
        for echo in all_echoes[path]:
            key = (echo.check, echo.line)
            if key not in seen:
                seen.add(key)
                deduped.append(echo)
        all_echoes[path] = deduped

    # Apply suppression, exclusion, and disabled-check filters.
    # This runs after all echoes (Layer 2 + Layer 3) are merged so that
    # ecko:ignore comments work uniformly across all check sources.
    for path in list(all_echoes.keys()):
        if is_excluded(path, cwd, user_excludes):
            del all_echoes[path]
            continue
        all_echoes[path] = filter_suppressed(all_echoes[path], path)
        all_echoes[path] = [e for e in all_echoes[path] if e.check not in disabled]
        if not all_echoes[path]:
            del all_echoes[path]

    if all_echoes:
        emit(format_stop_echoes(all_echoes))
        return 1
    return 0


def _get_modified_files(cwd: str) -> list[str]:
    """Get files modified in the current session via git."""
    files: set[str] = set()
    try:
        # Staged changes
        result = subprocess.run(
            ["git", "diff", "--name-only", "--cached"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))

        # Unstaged changes
        result = subprocess.run(
            ["git", "diff", "--name-only"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))

        # Untracked files
        result = subprocess.run(
            ["git", "ls-files", "--others", "--exclude-standard"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    return sorted(files)


def main() -> None:
    parser = argparse.ArgumentParser(description="Ecko runner")
    parser.add_argument("--file", help="File to check (PostToolUse mode)")
    parser.add_argument(
        "--mode",
        choices=["post-tool-use", "stop"],
        required=True,
        help="Run mode",
    )
    parser.add_argument("--cwd", required=True, help="Project working directory")
    parser.add_argument("--plugin-root", required=True, help="Plugin root directory")
    args = parser.parse_args()

    if args.mode == "post-tool-use":
        if not args.file:
            print("--file is required for post-tool-use mode", file=sys.stderr)
            sys.exit(2)
        sys.exit(run_post_tool_use(args.file, args.cwd, args.plugin_root))
    elif args.mode == "stop":
        sys.exit(run_stop(args.cwd, args.plugin_root))


if __name__ == "__main__":
    main()
