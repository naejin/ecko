"""Custom check: placeholder function bodies in Python (AST) and JS/TS (regex)."""

from __future__ import annotations

import ast
import re

from checks.result import Echo


def _has_decorator(func: ast.FunctionDef | ast.AsyncFunctionDef, names: set[str]) -> bool:
    """Check if a function has any decorator whose name is in the given set."""
    for dec in func.decorator_list:
        if isinstance(dec, ast.Name) and dec.id in names:
            return True
        if isinstance(dec, ast.Attribute) and dec.attr in names:
            return True
    return False


def _is_protocol_class(cls: ast.ClassDef) -> bool:
    """Check if a class inherits from Protocol."""
    for base in cls.bases:
        if isinstance(base, ast.Name) and base.id == "Protocol":
            return True
        if isinstance(base, ast.Attribute) and base.attr == "Protocol":
            return True
    return False


def _is_placeholder_body(body: list[ast.stmt]) -> str | None:
    """Check if a function body is a single placeholder statement.

    Returns a description of the placeholder kind, or None if not a placeholder.
    """
    # Filter out docstrings — a function with only a docstring + pass/... is still placeholder
    stmts = [
        stmt for stmt in body
        if not (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Constant)
            and isinstance(stmt.value.value, str)
        )
    ]

    if len(stmts) != 1:
        return None

    stmt = stmts[0]

    # pass
    if isinstance(stmt, ast.Pass):
        return "pass"

    # ... (Ellipsis)
    if (
        isinstance(stmt, ast.Expr)
        and isinstance(stmt.value, ast.Constant)
        and stmt.value.value is ...
    ):
        return "..."

    # raise NotImplementedError(...)
    if isinstance(stmt, ast.Raise) and stmt.exc is not None:
        exc = stmt.exc
        if isinstance(exc, ast.Call) and isinstance(exc.func, ast.Name):
            if exc.func.id == "NotImplementedError":
                return "raise NotImplementedError"
        if isinstance(exc, ast.Name) and exc.id == "NotImplementedError":
            return "raise NotImplementedError"

    return None


_SKIP_DECORATORS = {"abstractmethod", "overload"}

# JS/TS: throw new Error("not implemented") or throw new Error("TODO")
_JS_PLACEHOLDER_RE = re.compile(
    r'throw\s+new\s+Error\(\s*["\'](?:not\s*implemented|TODO)["\']',
    re.IGNORECASE,
)


def _check_function(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
    is_protocol: bool,
) -> Echo | None:
    """Check a single function for placeholder body. Returns Echo or None."""
    if _has_decorator(func, _SKIP_DECORATORS):
        return None
    if is_protocol:
        return None
    # Dunder methods often have intentionally empty bodies (protocol stubs)
    if func.name.startswith("__") and func.name.endswith("__"):
        return None

    placeholder_kind = _is_placeholder_body(func.body)
    if placeholder_kind is None:
        return None

    return Echo(
        check="placeholder-code",
        line=func.lineno,
        message=f"Placeholder function body ({placeholder_kind}).",
        suggestion="Implement the function or mark it @abstractmethod.",
    )


def check_placeholder_code(file_path: str) -> list[Echo]:
    """Detect placeholder function bodies in Python files.

    Walks module-level and class-level functions only (not nested functions)
    to avoid false positives on inner helpers.
    """
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
        tree = ast.parse(source, filename=file_path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []

    for node in ast.iter_child_nodes(tree):
        # Module-level functions
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            echo = _check_function(node, is_protocol=False)
            if echo:
                echoes.append(echo)
        # Class-level methods
        elif isinstance(node, ast.ClassDef):
            protocol = _is_protocol_class(node)
            for child in ast.iter_child_nodes(node):
                if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    echo = _check_function(child, is_protocol=protocol)
                    if echo:
                        echoes.append(echo)

    return echoes


def check_placeholder_code_js(file_path: str) -> list[Echo]:
    """Detect placeholder patterns in JS/TS files."""
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
    except (OSError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []
    in_block_comment = False
    for i, line in enumerate(source.splitlines(), 1):
        stripped = line.lstrip()

        # Track block comment state
        if in_block_comment:
            if "*/" in stripped:
                in_block_comment = False
            continue
        if stripped.startswith("/*"):
            if "*/" not in stripped[2:]:
                in_block_comment = True
            continue
        if stripped.startswith("//"):
            continue

        if _JS_PLACEHOLDER_RE.search(line):
            echoes.append(
                Echo(
                    check="placeholder-code",
                    line=i,
                    message='Placeholder: throw new Error("not implemented").',
                    suggestion="Implement the function body.",
                )
            )

    return echoes
