"""Project fingerprinting — detect frameworks from marker files."""

from __future__ import annotations

import json
import os

# Maximum file size to read (10KB) — avoid reading massive lockfiles
_MAX_FILE_SIZE = 10_240

# Framework detection markers: {framework: [(file, dependency_name), ...]}
_MARKERS: dict[str, list[tuple[str, str]]] = {
    "django": [
        ("requirements.txt", "django"),
        ("pyproject.toml", "django"),
    ],
    "flask": [
        ("requirements.txt", "flask"),
        ("pyproject.toml", "flask"),
    ],
    "fastapi": [
        ("requirements.txt", "fastapi"),
        ("pyproject.toml", "fastapi"),
    ],
    "express": [
        ("package.json", "express"),
    ],
    "nextjs": [
        ("package.json", "next"),
    ],
    "react": [
        ("package.json", "react"),
    ],
    "vue": [
        ("package.json", "vue"),
    ],
}


def _read_file_safe(path: str) -> str:
    """Read a file up to _MAX_FILE_SIZE characters, return empty string on failure."""
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read(_MAX_FILE_SIZE)
    except OSError:
        return ""


def _check_text_dependency(content: str, dep: str) -> bool:
    """Check if a dependency name appears in text content (requirements.txt, pyproject.toml)."""
    # Case-insensitive match on word boundary
    dep_lower = dep.lower()
    for line in content.lower().splitlines():
        # Strip comments and whitespace
        line = line.split("#")[0].strip()
        if dep_lower in line:
            return True
    return False


def _check_package_json(content: str, dep: str) -> bool:
    """Check if a dependency appears in package.json."""
    try:
        data = json.loads(content)
    except (json.JSONDecodeError, ValueError):
        return False
    for section in ("dependencies", "devDependencies", "peerDependencies"):
        deps = data.get(section, {})
        if isinstance(deps, dict) and dep in deps:
            return True
    return False


def detect_frameworks(cwd: str) -> set[str]:
    """Detect frameworks from marker files in cwd. Returns set of framework identifiers."""
    detected: set[str] = set()

    # Cache file contents to avoid re-reading
    file_cache: dict[str, str] = {}

    for framework, markers in _MARKERS.items():
        for filename, dep in markers:
            path = os.path.join(cwd, filename)
            if path not in file_cache:
                file_cache[path] = _read_file_safe(path)
            content = file_cache[path]
            if not content:
                continue

            checker = _check_package_json if filename == "package.json" else _check_text_dependency
            if checker(content, dep):
                detected.add(framework)
                break

    return detected
