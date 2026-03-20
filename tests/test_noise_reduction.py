"""Tests for v0.5.0 noise reduction features (Steps 1-5)."""

from __future__ import annotations

from checks.config import (
    _DEFAULT_BUILTIN_SHADOW_ALLOWLIST,
    get_builtin_shadow_allowlist,
    get_echo_cap,
)
from checks.custom.unreachable_code import check_unreachable_code
from checks.result import Echo, format_file_echoes, format_stop_echoes
from checks.tools.biome_adapter import RULE_MAP


# --- Step 1: biome rename ---


class TestBiomeRename:
    def test_rule_map_uses_new_name(self):
        assert RULE_MAP["noEmptyBlockStatements"] == "empty-block-statements"


# --- Step 2: vulture filters ---


class TestVultureFilters:
    def test_always_skip_has_descriptor_params(self):
        from checks.tools.vulture_adapter import _ALWAYS_SKIP

        assert "objtype" in _ALWAYS_SKIP
        assert "owner" in _ALWAYS_SKIP
        assert "sender" in _ALWAYS_SKIP

    def test_dunder_name_filtered(self):
        """Synthetic vulture output with dunder variable should be filtered."""
        from checks.tools.vulture_adapter import _NAME_RE

        msg = "unused argument '__n'"
        m = _NAME_RE.search(msg)
        assert m is not None
        assert m.group(1).startswith("__")

    def test_dunder_func_filtered(self):
        """Synthetic vulture output with dunder method should be filtered."""
        from checks.tools.vulture_adapter import _FUNC_RE

        msg = "unused method '__get__'"
        m = _FUNC_RE.search(msg)
        assert m is not None
        assert m.group(1).startswith("__")

    def test_vulture_yield_after_raise_filter(self, tmp_path):
        """Vulture adapter should skip yield-after-raise in generators."""
        from checks.tools.vulture_adapter import _is_yield_after_raise

        f = tmp_path / "gen.py"
        f.write_text(
            "async def stream():\n"
            "    raise StopIteration\n"
            "    yield b''\n",
            encoding="utf-8",
        )
        assert _is_yield_after_raise(str(f), 3) is True

    def test_vulture_yield_after_raise_non_generator(self, tmp_path):
        """Non-generator unreachable code should not be filtered."""
        from checks.tools.vulture_adapter import _is_yield_after_raise

        f = tmp_path / "bad.py"
        f.write_text(
            "def foo():\n"
            "    raise ValueError\n"
            "    print('unreachable')\n",
            encoding="utf-8",
        )
        assert _is_yield_after_raise(str(f), 3) is False

    def test_real_var_not_filtered(self):
        from checks.tools.vulture_adapter import _NAME_RE, _ALWAYS_SKIP

        msg = "unused variable 'real_var'"
        m = _NAME_RE.search(msg)
        assert m is not None
        name = m.group(1)
        assert name not in _ALWAYS_SKIP
        assert not name.startswith("__")


# --- Step 3: builtin-shadowing allowlist ---


class TestBuiltinShadowAllowlist:
    def test_default_allowlist(self):
        allowlist = get_builtin_shadow_allowlist({})
        assert allowlist == _DEFAULT_BUILTIN_SHADOW_ALLOWLIST
        assert "type" in allowlist
        assert "help" in allowlist
        assert "id" in allowlist
        assert "repr" in allowlist
        assert "ascii" in allowlist

    def test_user_override_replaces_default(self):
        config = {"builtin_shadow_allowlist": ["type", "custom_name"]}
        allowlist = get_builtin_shadow_allowlist(config)
        assert allowlist == frozenset({"type", "custom_name"})
        assert "help" not in allowlist  # not in user list

    def test_empty_user_list(self):
        config = {"builtin_shadow_allowlist": []}
        allowlist = get_builtin_shadow_allowlist(config)
        assert allowlist == frozenset()


class TestRuffAllowlistFilter:
    def test_shadow_regex_matches_backtick_format(self):
        from checks.tools.ruff_adapter import _SHADOW_NAME_RE

        msg = "Variable `type` is shadowing a Python builtin"
        m = _SHADOW_NAME_RE.search(msg)
        assert m is not None
        assert m.group(1) == "type"

    def test_shadow_filter_skips_allowed(self):
        from checks.tools.ruff_adapter import _SHADOW_NAME_RE

        allowlist = frozenset({"type", "id"})
        msg = "Variable `type` is shadowing a Python builtin"
        m = _SHADOW_NAME_RE.search(msg)
        assert m is not None
        assert m.group(1) in allowlist

    def test_shadow_filter_keeps_non_allowed(self):
        from checks.tools.ruff_adapter import _SHADOW_NAME_RE

        allowlist = frozenset({"type", "id"})
        msg = "Variable `foo` is shadowing a Python builtin"
        m = _SHADOW_NAME_RE.search(msg)
        assert m is not None
        assert m.group(1) not in allowlist


# --- Step 4: echo cap ---


class TestEchoCap:
    def test_default_cap(self):
        assert get_echo_cap({}) == 5

    def test_custom_cap(self):
        assert get_echo_cap({"echo_cap_per_check": 10}) == 10

    def test_unlimited_cap(self):
        assert get_echo_cap({"echo_cap_per_check": 0}) == 0

    def test_cap_limits_same_check(self):
        echoes = [
            Echo(check="var-declarations", line=i, message=f"msg{i}")
            for i in range(1, 6)
        ]
        output = format_file_echoes("test.ts", echoes, echo_cap=2)
        assert "5 echoes in test.ts" in output  # header shows total
        assert "var-declarations (line 1)" in output
        assert "var-declarations (line 2)" in output
        assert "var-declarations (line 3)" not in output
        assert "... and 3 more var-declarations" in output

    def test_cap_mixed_checks(self):
        echoes = [
            Echo(check="check-a", line=1, message="a1"),
            Echo(check="check-a", line=2, message="a2"),
            Echo(check="check-a", line=3, message="a3"),
            Echo(check="check-a", line=4, message="a4"),
            Echo(check="check-b", line=5, message="b1"),
            Echo(check="check-b", line=6, message="b2"),
            Echo(check="check-b", line=7, message="b3"),
        ]
        output = format_file_echoes("test.py", echoes, echo_cap=2)
        assert "7 echoes in test.py" in output
        assert "... and 2 more check-a" in output
        assert "... and 1 more check-b" in output

    def test_cap_zero_unlimited(self):
        echoes = [
            Echo(check="a", line=i, message=f"m{i}") for i in range(1, 11)
        ]
        output = format_file_echoes("test.py", echoes, echo_cap=0)
        assert "10 echoes" in output
        assert "... and" not in output

    def test_cap_stop_echoes(self):
        file_echoes = {
            "a.py": [
                Echo(check="dead-code", line=i, message=f"dc{i}")
                for i in range(1, 6)
            ]
        }
        output = format_stop_echoes(file_echoes, echo_cap=2)
        assert "5 echoes across 1 file" in output  # header total
        assert "... and 3 more dead-code" in output

    def test_disabled_checks_interaction(self):
        """When disabled_checks filters some echoes, cap applies to remaining."""
        echoes = [
            Echo(check="a", line=i, message=f"m{i}") for i in range(1, 9)
        ]
        # Simulate disabled_checks removing 3 echoes
        filtered = echoes[:5]  # 5 remain after filtering
        output = format_file_echoes("test.py", filtered, echo_cap=5)
        assert "5 echoes" in output
        assert "... and" not in output  # exactly at cap, no overflow


# --- Step 5: unreachable-code yield-after-raise ---


class TestUnreachableYieldAfterRaise:
    def test_generator_yield_after_raise(self, tmp_path):
        f = tmp_path / "gen.py"
        f.write_text(
            "def my_gen():\n"
            "    raise StopIteration\n"
            "    yield\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert echoes == []

    def test_contextmanager_yield_after_raise(self, tmp_path):
        f = tmp_path / "ctx.py"
        f.write_text(
            "from contextlib import contextmanager\n\n"
            "@contextmanager\n"
            "def managed():\n"
            "    raise RuntimeError\n"
            "    yield\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert echoes == []

    def test_print_after_raise_still_flagged(self, tmp_path):
        f = tmp_path / "bad.py"
        f.write_text(
            "def foo():\n"
            "    raise ValueError\n"
            "    print('unreachable')\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert len(echoes) == 1
        assert echoes[0].line == 3

    def test_nested_generator_correct_context(self, tmp_path):
        """Nested generator inside a regular function gets its own context."""
        f = tmp_path / "nested.py"
        f.write_text(
            "def outer():\n"
            "    def inner_gen():\n"
            "        raise StopIteration\n"
            "        yield\n"
            "    return inner_gen\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert echoes == []  # inner_gen is a generator

    def test_non_generator_nested_still_flagged(self, tmp_path):
        """Non-generator nested inside a generator should still flag."""
        f = tmp_path / "nested2.py"
        f.write_text(
            "def outer_gen():\n"
            "    yield 1\n"
            "    def inner():\n"
            "        raise ValueError\n"
            "        print('unreachable')\n"
            "    inner()\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert len(echoes) == 1
        assert echoes[0].line == 5

    def test_async_generator_yield_after_raise(self, tmp_path):
        """Async generator with yield b'' after raise — httpx pattern."""
        f = tmp_path / "stream.py"
        f.write_text(
            "async def aiter_bytes():\n"
            "    raise StreamClosed()\n"
            "    yield b''  # pragma: no cover\n",
            encoding="utf-8",
        )
        echoes = check_unreachable_code(str(f))
        assert echoes == []

    def test_existing_fixture_still_works(self):
        """Existing fixture test unchanged — basic unreachable detection."""
        from pathlib import Path

        fixtures = Path(__file__).parent / "fixtures"
        echoes = check_unreachable_code(str(fixtures / "unreachable.py"))
        assert len(echoes) == 2
        lines = {e.line for e in echoes}
        assert 3 in lines
        assert 9 in lines
