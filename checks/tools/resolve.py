"""Tool resolution — find tools on PATH, fall back to uvx/npx."""

from __future__ import annotations

import shutil


def resolve_python_tool(binary: str, package: str = "") -> list[str] | None:
    """Resolve a Python tool. Checks PATH → uvx → pipx run."""
    if shutil.which(binary):
        return [binary]
    pkg = package or binary
    if shutil.which("uvx"):
        return ["uvx", pkg]
    if shutil.which("pipx"):
        return ["pipx", "run", pkg]
    return None


def resolve_node_tool(binary: str, package: str = "") -> list[str] | None:
    """Resolve a Node tool. Checks PATH → npx → pnpx.

    When package differs from binary (e.g. tsc from typescript),
    uses --package to specify the npm package and binary as the command.
    """
    if shutil.which(binary):
        return [binary]
    pkg = package or binary
    if shutil.which("npx"):
        if pkg != binary:
            return ["npx", "--yes", "--package", pkg, binary]
        return ["npx", "--yes", binary]
    if shutil.which("pnpx"):
        if pkg != binary:
            return ["pnpx", "--package", pkg, binary]
        return ["pnpx", binary]
    return None
