"""Minimal ecko.yaml config loader.

Uses a simple line-based YAML subset parser — no PyYAML dependency.
Supports: scalar values, lists (- item), nested mappings (one level),
and list-of-dicts for banned_patterns/obsolete_terms.
"""

from __future__ import annotations

import os
from typing import Any


def _parse_yaml_subset(text: str) -> dict[str, Any]:
    """Parse a minimal YAML subset into a dict.

    Supports:
      key: value
      key:
        subkey: value
      key:
        - item
        - sub: val
          sub2: val2
    """
    result: dict[str, Any] = {}
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.rstrip()
        if not stripped or stripped.startswith("#"):
            i += 1
            continue
        indent = len(line) - len(line.lstrip())
        if indent > 0:
            i += 1
            continue
        if ":" not in stripped:
            i += 1
            continue
        key, _, rest = stripped.partition(":")
        key = key.strip()
        rest = rest.strip()
        if rest and not rest.startswith("#"):
            result[key] = _parse_scalar(rest)
            i += 1
            continue
        # Collect indented block
        i += 1
        block_items: list[str] = []
        while i < len(lines):
            bline = lines[i]
            bstripped = bline.rstrip()
            if not bstripped or bstripped.lstrip().startswith("#"):
                block_items.append(bline)
                i += 1
                continue
            bindent = len(bline) - len(bline.lstrip())
            if bindent == 0:
                break
            block_items.append(bline)
            i += 1
        block = _parse_block(block_items)
        result[key] = block
    return result


def _parse_block(lines: list[str]) -> Any:
    """Parse an indented block into a dict or list."""
    # Determine if it's a list or mapping
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith("- "):
            return _parse_list_block(lines)
        if ":" in stripped:
            return _parse_mapping_block(lines)
        break
    return {}


def _parse_list_block(lines: list[str]) -> list[Any]:
    """Parse a list block (lines starting with '- ')."""
    items: list[Any] = []
    i = 0
    while i < len(lines):
        stripped = lines[i].strip()
        if not stripped or stripped.startswith("#"):
            i += 1
            continue
        if stripped.startswith("- "):
            content = stripped[2:]
            if ":" in content:
                # Dict item
                item_dict: dict[str, Any] = {}
                k, _, v = content.partition(":")
                item_dict[k.strip()] = _parse_scalar(v.strip())
                i += 1
                # Collect continuation lines at deeper indent
                list_item_indent = len(lines[i - 1]) - len(lines[i - 1].lstrip())
                while i < len(lines):
                    nline = lines[i]
                    nstripped = nline.strip()
                    if not nstripped or nstripped.startswith("#"):
                        i += 1
                        continue
                    nindent = len(nline) - len(nline.lstrip())
                    if nindent <= list_item_indent or nstripped.startswith("- "):
                        break
                    if ":" in nstripped:
                        nk, _, nv = nstripped.partition(":")
                        item_dict[nk.strip()] = _parse_scalar(nv.strip())
                    i += 1
                items.append(item_dict)
            else:
                items.append(_parse_scalar(content))
                i += 1
        else:
            i += 1
    return items


def _parse_mapping_block(lines: list[str]) -> dict[str, Any]:
    """Parse a mapping block."""
    result: dict[str, Any] = {}
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if ":" in stripped:
            k, _, v = stripped.partition(":")
            result[k.strip()] = _parse_scalar(v.strip())
    return result


def _parse_scalar(value: str) -> Any:
    """Parse a scalar value."""
    if not value:
        return ""
    # Remove inline comments
    if "  #" in value:
        value = value[: value.index("  #")].rstrip()
    # Strip quotes and process escapes
    if value.startswith('"') and value.endswith('"'):
        return (
            value[1:-1]
            .replace("\\\\", "\x00")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\x00", "\\")
        )
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    if value.lower() == "true":
        return True
    if value.lower() == "false":
        return False
    if value.lower() in ("null", "~"):
        return None
    if value == "[]":
        return []
    try:
        return int(value)
    except ValueError:
        pass
    try:
        return float(value)
    except ValueError:
        pass
    return value


def load_config(cwd: str) -> dict[str, Any]:
    """Load ecko.yaml from the project root. Returns empty dict if not found."""
    path = os.path.join(cwd, "ecko.yaml")
    if not os.path.isfile(path):
        return {}
    with open(path, encoding="utf-8") as f:
        return _parse_yaml_subset(f.read())


def get_disabled_checks(config: dict[str, Any]) -> set[str]:
    """Return set of disabled check names from config."""
    disabled = config.get("disabled_checks", [])
    if isinstance(disabled, list):
        return set(disabled)
    return set()


def is_autofix_enabled(config: dict[str, Any], tool: str) -> bool:
    """Check if a specific autofix tool is enabled."""
    autofix = config.get("autofix", {})
    if isinstance(autofix, dict):
        if not autofix.get("enabled", True):
            return False
        return bool(autofix.get(tool, True))
    return True


def is_deep_enabled(config: dict[str, Any], tool: str) -> bool:
    """Check if a specific deep analysis tool is enabled."""
    deep = config.get("deep_analysis", {})
    if isinstance(deep, dict):
        return bool(deep.get(tool, True))
    return True


def get_exclude_patterns(config: dict[str, Any]) -> list[str]:
    """Return list of glob patterns to exclude from checks."""
    patterns = config.get("exclude", [])
    if isinstance(patterns, list):
        return [str(p) for p in patterns]
    return []


def get_banned_patterns(config: dict[str, Any]) -> list[dict[str, str]]:
    """Return list of banned pattern dicts from config."""
    patterns = config.get("banned_patterns", [])
    if isinstance(patterns, list):
        return patterns
    return []


def get_obsolete_terms(config: dict[str, Any]) -> list[dict[str, str]]:
    """Return list of obsolete term dicts from config."""
    terms = config.get("obsolete_terms", [])
    if isinstance(terms, list):
        return terms
    return []


def get_blocked_commands(config: dict[str, Any]) -> list[dict[str, str]]:
    """Return list of blocked command pattern dicts from config."""
    patterns = config.get("blocked_commands", [])
    if isinstance(patterns, list):
        return patterns
    return []


def is_learnings_enabled(config: dict[str, Any]) -> bool:
    """Check if the learnings nudge is enabled."""
    learnings = config.get("learnings", {})
    if isinstance(learnings, dict):
        return bool(learnings.get("enabled", False))
    return False
