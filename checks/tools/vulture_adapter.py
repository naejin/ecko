"""Vulture adapter — run vulture and parse output into Echoes."""

from __future__ import annotations

import ast
import glob
import os
import re
import subprocess

from checks.result import Echo
from checks.tools.resolve import resolve_python_tool

# vulture output: path:line: unused function 'name' (80% confidence)
VULTURE_PATTERN = re.compile(r"^(.+?):(\d+):\s+(.+?)\s+\((\d+)% confidence\)$")

# Protocol params — always framework-injected, never genuinely unused.
_ALWAYS_SKIP = {
    "exc_type", "exc_val", "exc_tb", "exc_info",  # __exit__ (PEP 343)
    "signum", "frame",                              # signal handlers
    "objtype", "owner",                              # descriptor __get__
    "sender",                                        # signal/event handlers
}

# pytest built-in fixtures — only skip in test/conftest files.
_PYTEST_SKIP = {
    "tmp_path", "tmp_path_factory", "capsys", "capfd", "caplog",
    "monkeypatch", "pytestconfig", "recwarn", "tmpdir", "tmpdir_factory",
}

# Extracts name from "unused variable 'foo'" / "unused argument 'foo'"
_NAME_RE = re.compile(r"unused (?:variable|argument|parameter) '(\w+)'")

# Extracts name from "unused function 'foo'" / "unused method 'foo'"
_FUNC_RE = re.compile(r"unused (?:function|method) '(\w+)'")

# Matches vulture's unreachable code message
_UNREACHABLE_RE = re.compile(r"unreachable code after '(?:raise|return|break|continue)'")


def _has_fixture_decorator(func: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Check if a function has @pytest.fixture or @fixture decorator."""
    for dec in func.decorator_list:
        # @fixture
        if isinstance(dec, ast.Name) and dec.id == "fixture":
            return True
        # @pytest.fixture
        if isinstance(dec, ast.Attribute) and dec.attr == "fixture":
            return True
        # @pytest.fixture(...) or @fixture(...)
        if isinstance(dec, ast.Call):
            f = dec.func
            if isinstance(f, ast.Name) and f.id == "fixture":
                return True
            if isinstance(f, ast.Attribute) and f.attr == "fixture":
                return True
    return False


def _is_yield_after_raise(path: str, lineno: int) -> bool:
    """Check if an unreachable line is a yield in a generator (intentional protocol)."""
    try:
        with open(path, encoding="utf-8") as f:
            source = f.read()
        tree = ast.parse(source, filename=path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return False
    # Check if the line is a yield/yield from statement
    lines = source.splitlines()
    if lineno < 1 or lineno > len(lines):
        return False
    line_text = lines[lineno - 1].strip()
    if not (line_text.startswith("yield") or line_text.startswith("await") and "yield" in line_text):
        return False
    # Find the enclosing function and check if it's a generator
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.lineno <= lineno and node.end_lineno and node.end_lineno >= lineno:
                # Check if this function contains yield statements
                for child in ast.walk(node):
                    if isinstance(child, (ast.Yield, ast.YieldFrom)):
                        return True
    return False


def _collect_fixture_names(cwd: str) -> set[str]:
    """Scan conftest.py files for @pytest.fixture decorated function names."""
    names: set[str] = set()
    for path in glob.glob(os.path.join(cwd, "**", "conftest.py"), recursive=True):
        try:
            with open(path, encoding="utf-8") as f:
                source = f.read()
            tree = ast.parse(source, filename=path)
        except (OSError, SyntaxError, UnicodeDecodeError):
            continue
        for node in ast.iter_child_nodes(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                if _has_fixture_decorator(node):
                    names.add(node.name)
    return names


def run_vulture(cwd: str) -> dict[str, list[Echo]]:
    """Run vulture with 80% confidence threshold. Returns echoes grouped by file."""
    cmd = resolve_python_tool("vulture")
    if not cmd:
        return {}

    try:
        result = subprocess.run(
            [*cmd, ".", "--min-confidence", "80"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=60,
        )
    except (subprocess.TimeoutExpired, OSError):
        return {}

    output = result.stdout.strip()
    if not output:
        return {}

    project_fixtures = _collect_fixture_names(cwd)
    effective_skip = _PYTEST_SKIP | project_fixtures

    file_echoes: dict[str, list[Echo]] = {}
    for line in output.splitlines():
        match = VULTURE_PATTERN.match(line.strip())
        if match:
            path = match.group(1)
            lineno = int(match.group(2))
            message = match.group(3)
            name_match = _NAME_RE.search(message)
            if name_match:
                name = name_match.group(1)
                if name in _ALWAYS_SKIP:
                    continue
                if name.startswith("__"):
                    continue
                # Also suppresses genuine unused variables with fixture names
                # in test files — vulture can't distinguish fixture injection
                # from dead code, so we accept the false negative here.
                if name in effective_skip:
                    basename = os.path.basename(path)
                    if (basename.startswith("test_") or basename.endswith("_test.py")
                            or basename in ("conftest.py", "conftest.pyi")):
                        continue
            # Fixture definitions in test/conftest files (reported as "unused function")
            func_match = _FUNC_RE.search(message)
            if func_match:
                fname = func_match.group(1)
                if fname.startswith("__"):
                    continue
                if fname in effective_skip:
                    basename = os.path.basename(path)
                    if (basename.startswith("test_") or basename.endswith("_test.py")
                            or basename in ("conftest.py", "conftest.pyi")):
                        continue
            # Skip yield-after-raise in generators (intentional protocol pattern)
            if _UNREACHABLE_RE.search(message):
                abs_path = os.path.join(cwd, path) if not os.path.isabs(path) else path
                if _is_yield_after_raise(abs_path, lineno):
                    continue
            file_echoes.setdefault(path, []).append(
                Echo(
                    check="dead-code",
                    line=lineno,
                    message=message,
                    suggestion="Remove it if truly unused.",
                )
            )

    return file_echoes
