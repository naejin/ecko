"""Custom check: unreachable code after return/raise/break/continue in Python (AST-based)."""

from __future__ import annotations

import ast

from checks.result import Echo

TERMINAL_TYPES = (ast.Return, ast.Raise, ast.Break, ast.Continue)


def check_unreachable_code(file_path: str) -> list[Echo]:
    """Walk Python AST for statements after terminal statements in the same body."""
    try:
        with open(file_path) as f:
            source = f.read()
        tree = ast.parse(source, filename=file_path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []
    _walk_bodies(tree, echoes)
    return echoes


def _walk_bodies(node: ast.AST, echoes: list[Echo]) -> None:
    """Recursively check statement bodies for unreachable code."""
    for child in ast.iter_child_nodes(node):
        # Check bodies of functions, classes, if/elif/else, for, while, with
        for attr in ("body", "orelse", "finalbody", "handlers"):
            body = getattr(child, attr, None)
            if isinstance(body, list):
                _check_body(body, echoes)
        _walk_bodies(child, echoes)


def _check_body(body: list[ast.stmt], echoes: list[Echo]) -> None:
    """Check a single body list for unreachable statements."""
    found_terminal = False
    for stmt in body:
        if found_terminal:
            echoes.append(
                Echo(
                    check="unreachable-code",
                    line=stmt.lineno,
                    message="Unreachable code after return/raise/break/continue.",
                    suggestion="Remove the unreachable statement.",
                )
            )
            break  # Only report the first unreachable statement per body
        if isinstance(stmt, TERMINAL_TYPES):
            found_terminal = True
