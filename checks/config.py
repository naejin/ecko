"""Minimal ecko.yaml config loader.

Uses a simple line-based YAML subset parser — no PyYAML dependency.
Supports: scalar values, lists (- item), nested mappings (one level),
and list-of-dicts for banned_patterns/obsolete_terms.
"""

from __future__ import annotations

import os
from typing import Any

from checks.regex_utils import safe_regex_compile


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
                # Track the most recent key assigned empty value for sub-list binding
                last_empty_key: str | None = None
                if not v.strip():
                    last_empty_key = k.strip()
                # Collect continuation lines at deeper indent
                list_item_indent = len(lines[i - 1]) - len(lines[i - 1].lstrip())
                while i < len(lines):
                    nline = lines[i]
                    nstripped = nline.strip()
                    if not nstripped or nstripped.startswith("#"):
                        i += 1
                        continue
                    nindent = len(nline) - len(nline.lstrip())
                    if nindent <= list_item_indent:
                        break
                    # Sub-list items (- foo) at deeper indent than list item
                    if nstripped.startswith("- ") and last_empty_key is not None:
                        sub_list: list[Any] = []
                        sub_indent = nindent
                        while i < len(lines):
                            sline = lines[i]
                            sstripped = sline.strip()
                            if not sstripped or sstripped.startswith("#"):
                                i += 1
                                continue
                            sindent = len(sline) - len(sline.lstrip())
                            if sindent < sub_indent or (
                                sindent == sub_indent and not sstripped.startswith("- ")
                            ):
                                break
                            if sindent > sub_indent:
                                # Deeper than sub-list — skip (not supported)
                                i += 1
                                continue
                            if sstripped.startswith("- "):
                                sub_list.append(_parse_scalar(sstripped[2:]))
                            i += 1
                        item_dict[last_empty_key] = sub_list
                        last_empty_key = None
                    elif nstripped.startswith("- "):
                        # Sub-list but no empty key to bind to — stop
                        break
                    elif ":" in nstripped:
                        nk, _, nv = nstripped.partition(":")
                        nk_clean = nk.strip()
                        nv_clean = nv.strip()
                        item_dict[nk_clean] = _parse_scalar(nv_clean)
                        if not nv_clean:
                            last_empty_key = nk_clean
                        else:
                            last_empty_key = None
                        i += 1
                    else:
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


_DEFAULT_BUILTIN_SHADOW_ALLOWLIST = frozenset(
    {
        "type",
        "help",
        "input",
        "format",
        "id",
        "dir",
        "open",
        "filter",
        "map",
        "hash",
        "min",
        "max",
        "range",
        "slice",
        "vars",
        "locals",
        "globals",
        "property",
        "repr",
        "ascii",
    }
)


def get_builtin_shadow_allowlist(config: dict[str, Any]) -> frozenset[str]:
    """Return builtin-shadowing allowlist. User list replaces default entirely."""
    user = config.get("builtin_shadow_allowlist")
    if isinstance(user, list):
        return frozenset(str(n) for n in user)
    return _DEFAULT_BUILTIN_SHADOW_ALLOWLIST


def get_echo_cap(config: dict[str, Any]) -> int:
    """Return the per-check-per-file echo cap. Default 5, 0 = unlimited."""
    cap = config.get("echo_cap_per_check", 5)
    if isinstance(cap, int):
        return cap
    return 5


def get_import_rules(config: dict[str, Any]) -> list[dict]:
    """Return list of import rule dicts from config."""
    rules = config.get("import_rules", [])
    if isinstance(rules, list):
        return rules
    return []


def is_reverb_enabled(config: dict[str, Any]) -> bool:
    """Check if the reverb nudge is enabled."""
    reverb = config.get("reverb", {})
    if isinstance(reverb, dict):
        return bool(reverb.get("enabled", False))
    return False


# --- Config validation ---

_KNOWN_KEYS = frozenset(
    {
        "autofix",
        "deep_analysis",
        "banned_patterns",
        "obsolete_terms",
        "import_rules",
        "builtin_shadow_allowlist",
        "echo_cap_per_check",
        "exclude",
        "blocked_commands",
        "reverb",
        "disabled_checks",
    }
)


def validate_config(config: dict[str, Any]) -> list[str]:
    """Validate config and return a list of warning messages.

    Checks for:
    - Unknown top-level keys (possible typos)
    - Invalid regex patterns in banned_patterns and blocked_commands
    """
    warnings: list[str] = []

    # Check for unknown keys
    for key in config:
        if key not in _KNOWN_KEYS:
            # Find closest match for "did you mean?" suggestion
            suggestion = _closest_key(key, _KNOWN_KEYS)
            if suggestion:
                warnings.append(
                    f"unknown config key '{key}' (did you mean '{suggestion}'?)"
                )
            else:
                warnings.append(f"unknown config key '{key}'")

    # Validate regex patterns in banned_patterns (with ReDoS timeout protection)
    for i, rule in enumerate(get_banned_patterns(config)):
        pattern = rule.get("pattern", "")
        if pattern:
            if safe_regex_compile(pattern) is None:
                warnings.append(
                    f"invalid or pathological regex in banned_patterns[{i}]: {pattern!r}"
                )

    # Validate regex patterns in blocked_commands (with ReDoS timeout protection)
    for i, rule in enumerate(get_blocked_commands(config)):
        pattern = rule.get("pattern", "")
        if pattern:
            if safe_regex_compile(pattern) is None:
                warnings.append(
                    f"invalid or pathological regex in blocked_commands[{i}]: {pattern!r}"
                )

    return warnings


def _closest_key(key: str, known: frozenset[str]) -> str | None:
    """Find the closest known key by simple edit-distance heuristic."""
    # Check prefix/suffix match first (catches typos like disabled_check)
    for k in known:
        if k.startswith(key) or key.startswith(k):
            return k
    # Check single-char difference
    for k in known:
        if abs(len(k) - len(key)) <= 2:
            diff = sum(1 for a, b in zip(k, key) if a != b)
            diff += abs(len(k) - len(key))
            if diff <= 2:
                return k
    return None
