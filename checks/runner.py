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
    get_blocked_commands,
    get_builtin_shadow_allowlist,
    get_disabled_checks,
    get_echo_cap,
    get_exclude_patterns,
    get_import_rules,
    get_obsolete_terms,
    is_deep_enabled,
    is_reverb_enabled,
    load_config,
    validate_config,
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


def _is_test_file(file_path: str) -> bool:
    """Check if a file is a Python test file (by filename convention)."""
    name = os.path.basename(file_path)
    return (
        name.startswith("test_") or name.endswith("_test.py") or name == "conftest.py"
    )


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
        with open(file_path, encoding="utf-8") as f:
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


def _emit_config_warnings(config: dict) -> None:
    """Emit config validation warnings to stderr (once per session)."""
    warnings = validate_config(config)
    for w in warnings:
        emit(f"~~ ecko ~~ warning: {w}\n")


def _emit_skipped_tools(skipped: list[str]) -> None:
    """Emit a summary line for tools that were unavailable."""
    if skipped:
        names = ", ".join(f"{t} (not found)" for t in skipped)
        emit(f"~~ ecko ~~ note: {names} — install for deeper checks\n")


def run_post_tool_use(file_path: str, cwd: str, plugin_root: str) -> int:
    """Run Layer 1 (auto-fix) then Layer 2 (echoes) on a single file."""
    if not os.path.isfile(file_path):
        return 0

    # Type stubs (.pyi) exist for type checkers, not runtime — skip linting
    if file_path.endswith(".pyi"):
        return 0

    config = load_config(cwd)
    _emit_config_warnings(config)

    if is_excluded(file_path, cwd, get_exclude_patterns(config)):
        return 0

    disabled = get_disabled_checks(config)
    shadow_allowlist = get_builtin_shadow_allowlist(config)
    echo_cap = get_echo_cap(config)
    lang = detect_language(file_path)

    # --- Layer 1: Auto-fix (silent) ---
    from checks.formatter import autofix

    autofix(file_path, lang, config)

    # --- Layer 2: Echoes ---
    echoes: list[Echo] = []
    skipped: list[str] = []

    # Tool checks
    if lang == "python":
        from checks.tools.resolve import resolve_python_tool
        from checks.tools.ruff_adapter import run_ruff

        if resolve_python_tool("ruff") is None:
            skipped.append("ruff")
        else:
            echoes.extend(
                run_ruff(file_path, builtin_shadow_allowlist=shadow_allowlist)
            )
    elif lang in ("typescript", "javascript"):
        from checks.tools.biome_adapter import run_biome
        from checks.tools.resolve import resolve_node_tool

        if resolve_node_tool("biome") is None:
            skipped.append("biome")
        else:
            echoes.extend(run_biome(file_path, plugin_root))

    # Custom checks (Python AST)
    if lang == "python":
        from checks.custom.duplicate_keys import check_duplicate_keys
        from checks.custom.unreachable_code import check_unreachable_code

        echoes.extend(check_duplicate_keys(file_path))
        echoes.extend(check_unreachable_code(file_path))

        if _is_test_file(file_path):
            from checks.custom.test_quality import check_test_quality

            echoes.extend(check_test_quality(file_path))

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

    # Import layer rules
    import_rules = get_import_rules(config)
    if import_rules:
        from checks.custom.import_layers import check_import_layers

        echoes.extend(check_import_layers(file_path, import_rules, cwd))

    # Filter
    echoes = filter_suppressed(echoes, file_path)
    echoes = [e for e in echoes if e.check not in disabled]

    _emit_skipped_tools(skipped)

    if echoes:
        emit(format_file_echoes(file_path, echoes, echo_cap=echo_cap))
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
    _emit_config_warnings(config)

    disabled = get_disabled_checks(config)
    user_excludes = get_exclude_patterns(config)
    shadow_allowlist = get_builtin_shadow_allowlist(config)
    echo_cap = get_echo_cap(config)
    import_rules = get_import_rules(config)

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
    skipped: list[str] = []

    # --- Layer 3: Deep analysis (parallelized) ---
    from concurrent.futures import ThreadPoolExecutor, as_completed

    from checks.tools.resolve import resolve_node_tool, resolve_python_tool

    # Build set of normalized modified paths for post-filtering
    modified_set = {_normalize_path(f, cwd) for f in modified}

    def _run_tsc() -> dict[str, list[Echo]]:
        from checks.tools.tsc_adapter import run_tsc

        return run_tsc(cwd)

    def _run_pyright() -> dict[str, list[Echo]]:
        from checks.tools.pyright_adapter import run_pyright

        return run_pyright(py_files, cwd)

    def _run_vulture() -> dict[str, list[Echo]]:
        from checks.tools.vulture_adapter import run_vulture

        return run_vulture(cwd, modified_files=py_files)

    def _run_knip() -> dict[str, list[Echo]]:
        from checks.tools.knip_adapter import run_knip

        return run_knip(cwd)

    futures = {}
    with ThreadPoolExecutor(max_workers=4) as pool:
        if ts_files and is_deep_enabled(config, "tsc"):
            tsconfig = os.path.join(cwd, "tsconfig.json")
            if os.path.isfile(tsconfig):
                if resolve_node_tool("tsc", package="typescript") is None:
                    skipped.append("tsc")
                else:
                    futures[pool.submit(_run_tsc)] = "tsc"

        if py_files and is_deep_enabled(config, "pyright"):
            if resolve_node_tool("pyright") is None:
                skipped.append("pyright")
            else:
                futures[pool.submit(_run_pyright)] = "pyright"

        if py_files and is_deep_enabled(config, "vulture"):
            if resolve_python_tool("vulture") is None:
                skipped.append("vulture")
            else:
                futures[pool.submit(_run_vulture)] = "vulture"

        if ts_files and is_deep_enabled(config, "knip"):
            if resolve_node_tool("knip") is None:
                skipped.append("knip")
            else:
                futures[pool.submit(_run_knip)] = "knip"

        for future in as_completed(futures):
            try:
                result = future.result()
            except Exception:
                continue
            for path, echoes in result.items():
                norm = _normalize_path(path, cwd)
                # Post-filter tsc/knip results to modified files only
                if norm not in modified_set:
                    continue
                all_echoes.setdefault(norm, []).extend(echoes)

    # --- Layer 2: Re-sweep all modified files ---
    banned = get_banned_patterns(config)
    obsolete = get_obsolete_terms(config)

    # Check Layer 2 tool availability once before the loop
    ruff_available = resolve_python_tool("ruff") is not None
    biome_available = resolve_node_tool("biome") is not None
    if py_files and not ruff_available and "ruff" not in skipped:
        skipped.append("ruff")
    if ts_files and not biome_available and "biome" not in skipped:
        skipped.append("biome")

    for file_path in modified:
        if not os.path.isfile(file_path):
            continue
        lang = detect_language(file_path)
        echoes: list[Echo] = []

        if lang == "python":
            if ruff_available:
                from checks.tools.ruff_adapter import run_ruff

                echoes.extend(
                    run_ruff(file_path, builtin_shadow_allowlist=shadow_allowlist)
                )
            from checks.custom.duplicate_keys import check_duplicate_keys
            from checks.custom.unreachable_code import check_unreachable_code

            echoes.extend(check_duplicate_keys(file_path))
            echoes.extend(check_unreachable_code(file_path))

            if _is_test_file(file_path):
                from checks.custom.test_quality import check_test_quality

                echoes.extend(check_test_quality(file_path))
        elif lang in ("typescript", "javascript"):
            if biome_available:
                from checks.tools.biome_adapter import run_biome

                echoes.extend(run_biome(file_path, plugin_root))

        from checks.custom.unicode_artifacts import check_unicode_artifacts

        echoes.extend(check_unicode_artifacts(file_path))

        if banned:
            from checks.custom.banned_patterns import check_banned_patterns

            echoes.extend(check_banned_patterns(file_path, banned, cwd))

        if obsolete:
            from checks.custom.banned_patterns import check_obsolete_terms

            echoes.extend(check_obsolete_terms(file_path, obsolete))

        if import_rules:
            from checks.custom.import_layers import check_import_layers

            echoes.extend(check_import_layers(file_path, import_rules, cwd))

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

    _emit_skipped_tools(skipped)

    has_echoes = bool(all_echoes)

    if has_echoes:
        emit(format_stop_echoes(all_echoes, echo_cap=echo_cap))

    # Reverb nudge — only when echoes were found (something went wrong)
    if has_echoes and is_reverb_enabled(config):
        emit(
            "\n~~ ecko ~~ Leave a reverb note at "
            ".ecko-reverb/{date}-{slug}.md — what echoed back? "
            "Include: 1) Surprises about naming, structure, or conventions "
            "2) Things that tripped you up "
            "3) Suggestions for guardrails or checks that would have helped.\n"
        )

    if has_echoes:
        return 1
    return 0


def check_bash_command(command: str, user_patterns: list[dict[str, str]]) -> str | None:
    """Check a bash command against blocked patterns.

    Returns the block message if the command should be blocked, or None if allowed.
    """
    import re

    # Hardcoded patterns (always active, truly dangerous)
    hardcoded = [
        {
            "pattern": r"git\b.*--no-verify",
            "message": "Blocked: --no-verify skips hooks — remove it to let ecko checks run",
        },
        {
            "pattern": r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*\s+/(\s|$|;|&|\|)",
            "message": "Blocked: rm -rf / is catastrophic — specify a subdirectory",
        },
        {
            "pattern": r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*\s+~(\s|$|;|&|\||/)",
            "message": "Blocked: rm -rf ~ would delete the home directory",
        },
        {
            "pattern": r"git\s+push\b.*--force(?!-with-lease)(\s|$|;|&|\|)",
            "message": "Blocked: use --force-with-lease instead of --force to prevent overwriting remote work",
        },
        {
            "pattern": r"git\s+reset\s+--hard(\s|$|;|&|\|)",
            "message": "Blocked: git reset --hard discards commits permanently — use git stash or git revert instead",
        },
        {
            "pattern": r"git\s+clean\s+-[a-zA-Z]*f[a-zA-Z]*(\s|$|;|&|\|)",
            "message": "Blocked: git clean -f deletes untracked files permanently — review with git clean -n first",
        },
    ]

    for entry in hardcoded + user_patterns:
        pattern = entry.get("pattern", "")
        message = entry.get("message", "Command blocked by ecko")
        if not pattern:
            continue
        try:
            if _safe_regex_search(pattern, command):
                return message
        except re.error:
            continue
    return None


def _safe_regex_search(pattern: str, text: str, timeout_ms: int = 500) -> bool:
    """Run re.search with a timeout to guard against ReDoS from user patterns.

    Uses a thread with a timeout. Returns False on timeout or error.
    """
    import re
    import threading

    result: list[bool] = [False]

    def _search() -> None:
        try:
            result[0] = bool(re.search(pattern, text))
        except re.error:
            result[0] = False

    t = threading.Thread(target=_search, daemon=True)
    t.start()
    t.join(timeout=timeout_ms / 1000.0)
    if t.is_alive():
        # Timed out — likely ReDoS, treat as no match
        return False
    return result[0]


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
        choices=["post-tool-use", "stop", "pre-tool-use-bash"],
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
    elif args.mode == "pre-tool-use-bash":
        sys.exit(run_pre_tool_use_bash(args.cwd))


if __name__ == "__main__":
    main()
