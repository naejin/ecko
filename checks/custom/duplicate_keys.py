"""Custom check: duplicate dictionary keys in Python (AST-based)."""

from __future__ import annotations

import ast

from checks.result import Echo


def check_duplicate_keys(file_path: str) -> list[Echo]:
    """Walk Python AST for Dict nodes with duplicate Constant keys."""
    try:
        with open(file_path) as f:
            source = f.read()
        tree = ast.parse(source, filename=file_path)
    except (OSError, SyntaxError, UnicodeDecodeError):
        return []

    echoes: list[Echo] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Dict):
            continue
        seen: dict[object, int] = {}
        for key in node.keys:
            if key is None:
                continue  # **kwargs unpacking
            if isinstance(key, ast.Constant):
                val = key.value
                if val in seen:
                    echoes.append(
                        Echo(
                            check="duplicate-keys",
                            line=key.lineno,
                            message=f"Duplicate dictionary key `{val!r}`.",
                            suggestion="Remove the duplicate or rename it.",
                        )
                    )
                else:
                    seen[val] = key.lineno
    return echoes
