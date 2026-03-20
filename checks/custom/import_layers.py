"""Custom check: architecture layer enforcement via import rules."""

from __future__ import annotations

import ast
import os
import re
from fnmatch import fnmatch

from checks.result import Echo

# JS/TS import patterns: import X from 'mod', require('mod')
_JS_IMPORT_RE = re.compile(
    r"""(?:import\s+.*?\s+from\s+['"](.+?)['"]|require\s*\(\s*['"](.+?)['"]\s*\))"""
)


def _extract_python_imports(file_path: str) -> list[str]:
    """Extract top-level module names from Python imports via AST."""
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
        tree = ast.parse(source, filename=file_path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return []

    imports: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                imports.append(alias.name)
        elif isinstance(node, ast.ImportFrom) and node.module:
            imports.append(node.module)
    return imports


def _extract_js_imports(file_path: str) -> list[str]:
    """Extract module specifiers from JS/TS import/require statements."""
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
    except (OSError, UnicodeDecodeError):
        return []

    imports: list[str] = []
    for m in _JS_IMPORT_RE.finditer(source):
        mod = m.group(1) or m.group(2)
        if mod:
            imports.append(mod)
    return imports


def _matches_deny(imp: str, denied: str, is_python: bool) -> bool:
    """Check if an import matches a denied module (separator-aware prefix match)."""
    if imp == denied:
        return True
    sep = "." if is_python else "/"
    return imp.startswith(denied + sep)


def check_import_layers(
    file_path: str,
    rules: list[dict],
    cwd: str,
) -> list[Echo]:
    """Check file imports against architecture layer rules."""
    if not rules:
        return []

    # Compute relative path for file matching
    try:
        rel = os.path.relpath(file_path, cwd).replace(os.sep, "/")
    except ValueError:
        rel = os.path.basename(file_path)

    ext = os.path.splitext(file_path)[1].lower()
    is_python = ext in (".py", ".pyi")
    is_js = ext in (".js", ".jsx", ".ts", ".tsx")

    if not is_python and not is_js:
        return []

    # Extract imports
    if is_python:
        imports = _extract_python_imports(file_path)
    else:
        imports = _extract_js_imports(file_path)

    if not imports:
        return []

    echoes: list[Echo] = []
    for rule in rules:
        file_glob = rule.get("files", "")
        if not file_glob:
            continue
        # Match against relative path and basename
        if not (fnmatch(rel, file_glob) or fnmatch(os.path.basename(file_path), file_glob)):
            continue

        deny_list = rule.get("deny_import", [])
        if isinstance(deny_list, str):
            deny_list = [deny_list]
        message = rule.get("message", "Import violates architecture layer rules")

        for imp in imports:
            for denied in deny_list:
                if _matches_deny(imp, str(denied), is_python):
                    echoes.append(
                        Echo(
                            check="import-layer",
                            line=0,
                            message=f"Import '{imp}' is denied by rule: {message}",
                            suggestion=f"Remove or replace the import of '{denied}'.",
                        )
                    )
    return echoes
