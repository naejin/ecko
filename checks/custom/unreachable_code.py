"""Custom check: unreachable code after return/raise/break/continue in Python (AST-based)."""

from __future__ import annotations

import ast

from checks.result import Echo

TERMINAL_TYPES = (ast.Return, ast.Raise, ast.Break, ast.Continue)


def _has_contextmanager_decorator(func: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Check if a function is decorated with @contextmanager/@asynccontextmanager."""
    for dec in func.decorator_list:
        if isinstance(dec, ast.Name) and dec.id in ("contextmanager", "asynccontextmanager"):
            return True
        if isinstance(dec, ast.Attribute) and dec.attr in ("contextmanager", "asynccontextmanager"):
            return True
    return False


def _is_generator(func: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Check if a function contains yield/yield from (is a generator)."""
    for node in ast.walk(func):
        if isinstance(node, (ast.Yield, ast.YieldFrom)):
            return True
    return False


def _is_yield_stmt(stmt: ast.stmt) -> bool:
    """Check if a statement is a bare yield expression."""
    return (
        isinstance(stmt, ast.Expr)
        and isinstance(stmt.value, (ast.Yield, ast.YieldFrom))
    )


def check_unreachable_code(file_path: str) -> list[Echo]:
    """Walk Python AST for statements after terminal statements in the same body."""
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
        tree = ast.parse(source, filename=file_path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []
    _walk_bodies(tree, echoes, enclosing_func=None)
    return echoes


def _walk_bodies(
    node: ast.AST,
    echoes: list[Echo],
    enclosing_func: ast.FunctionDef | ast.AsyncFunctionDef | None = None,
) -> None:
    """Recursively check statement bodies for unreachable code."""
    for child in ast.iter_child_nodes(node):
        # Track enclosing function context for yield-after-raise detection
        if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
            func = child
        else:
            func = enclosing_func
        # Check bodies of functions, classes, if/elif/else, for, while, with
        for attr in ("body", "orelse", "finalbody", "handlers"):
            body = getattr(child, attr, None)
            if isinstance(body, list):
                _check_body(body, echoes, enclosing_func=func)
        _walk_bodies(child, echoes, enclosing_func=func)


def _check_body(
    body: list[ast.stmt],
    echoes: list[Echo],
    enclosing_func: ast.FunctionDef | ast.AsyncFunctionDef | None = None,
) -> None:
    """Check a single body list for unreachable statements."""
    found_terminal = False
    for stmt in body:
        if found_terminal:
            # Skip yield-after-raise in generators and @contextmanager functions
            if (
                _is_yield_stmt(stmt)
                and enclosing_func is not None
                and (_is_generator(enclosing_func) or _has_contextmanager_decorator(enclosing_func))
            ):
                found_terminal = False
                continue
            echoes.append(
                Echo(
                    check="unreachable-code",
                    line=stmt.lineno,
                    message="Unreachable code after return/raise/break/continue.",
                    suggestion="Remove the unreachable statement.",
                    severity="error",
                )
            )
            break  # Only report the first unreachable statement per body
        if isinstance(stmt, TERMINAL_TYPES):
            found_terminal = True
