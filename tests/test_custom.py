"""Tests for custom checks (no external tools needed)."""

from __future__ import annotations

import os
from pathlib import Path

from checks.custom.banned_patterns import check_banned_patterns, check_obsolete_terms
from checks.custom.duplicate_keys import check_duplicate_keys
from checks.custom.test_quality import check_test_quality
from checks.custom.unicode_artifacts import check_unicode_artifacts
from checks.custom.unreachable_code import check_unreachable_code
from checks.result import Echo
from checks.fileutil import is_test_file
from checks.runner import (
    _is_standalone_comment,
    _normalize_path,
    filter_suppressed,
    is_excluded,
)

FIXTURES = Path(__file__).parent / "fixtures"


class TestDuplicateKeys:
    def test_detects_duplicates(self):
        echoes = check_duplicate_keys(str(FIXTURES / "duplicate_keys.py"))
        assert len(echoes) == 1
        assert echoes[0].check == "duplicate-keys"
        assert "'name'" in echoes[0].message

    def test_clean_file(self):
        echoes = check_duplicate_keys(str(FIXTURES / "clean.py"))
        assert echoes == []

    def test_nonexistent_file(self):
        echoes = check_duplicate_keys("/nonexistent/file.py")
        assert echoes == []


class TestUnreachableCode:
    def test_detects_unreachable(self):
        echoes = check_unreachable_code(str(FIXTURES / "unreachable.py"))
        assert len(echoes) == 2
        assert all(e.check == "unreachable-code" for e in echoes)
        lines = {e.line for e in echoes}
        assert 3 in lines  # after return
        assert 9 in lines  # after break

    def test_clean_file(self):
        echoes = check_unreachable_code(str(FIXTURES / "clean.py"))
        assert echoes == []


class TestUnicodeArtifacts:
    def test_detects_em_dash_in_code(self):
        echoes = check_unicode_artifacts(str(FIXTURES / "unicode_artifacts.js"))
        assert len(echoes) >= 1
        assert any(e.check == "unicode-artifact" for e in echoes)

    def test_skips_python_strings(self):
        # Em dash in Python string literal should be skipped
        echoes = check_unicode_artifacts(str(FIXTURES / "unicode_artifacts.py"))
        # The fixture has em dash only in a comment and a string
        # Comment: skipped for Python (tokenizer)
        # String: skipped for Python (tokenizer)
        assert echoes == []

    def test_skips_js_strings_and_comments(self):
        # Unicode inside JS/TS string literals and comments should be skipped
        echoes = check_unicode_artifacts(str(FIXTURES / "unicode_in_js_strings.ts"))
        assert echoes == []

    def test_detects_unicode_in_js_code(self):
        # Unicode in actual JS/TS code (not strings/comments) should fire
        echoes = check_unicode_artifacts(str(FIXTURES / "unicode_in_js_code.ts"))
        assert len(echoes) == 2
        assert all(e.check == "unicode-artifact" for e in echoes)
        lines = {e.line for e in echoes}
        assert 1 in lines  # non-breaking space
        assert 2 in lines  # em dash

    def test_js_mixed_strings_and_code(self):
        # Unicode in strings should be skipped, unicode in code should fire
        echoes = check_unicode_artifacts(str(FIXTURES / "unicode_js_mixed.ts"))
        assert len(echoes) == 1
        assert echoes[0].check == "unicode-artifact"
        assert echoes[0].line == 3  # non-breaking space in code

    def test_clean_file(self):
        echoes = check_unicode_artifacts(str(FIXTURES / "clean.py"))
        assert echoes == []

    def test_skips_python_fstrings(self, tmp_path):
        """Unicode inside Python f-string literals should be skipped (3.12+ tokenizer)."""
        f = tmp_path / "fstr.py"
        f.write_text('x = 42\nmsg = f"value: {x} \u2014 done"\n')
        echoes = check_unicode_artifacts(str(f))
        assert echoes == []


class TestBannedPatterns:
    def test_detects_pattern(self, tmp_path):
        f = tmp_path / "test.tsx"
        f.write_text('<div className="bg-blue-500">hello</div>\n')
        patterns = [
            {"pattern": r"bg-(blue|red|green)-\d+", "glob": "*.tsx", "message": "Use brand colors"}
        ]
        echoes = check_banned_patterns(str(f), patterns)
        assert len(echoes) == 1
        assert echoes[0].check == "banned-pattern"
        assert "brand colors" in echoes[0].message

    def test_glob_filter(self, tmp_path):
        f = tmp_path / "test.py"
        f.write_text('x = "bg-blue-500"\n')
        patterns = [
            {"pattern": r"bg-(blue|red|green)-\d+", "glob": "*.tsx", "message": "bad"}
        ]
        echoes = check_banned_patterns(str(f), patterns)
        assert echoes == []  # .py doesn't match *.tsx glob

    def test_empty_patterns(self):
        echoes = check_banned_patterns("/some/file.py", [])
        assert echoes == []

    def test_redos_search_does_not_hang(self, tmp_path):
        """Pathological regex at search time should timeout, not hang."""
        import time

        f = tmp_path / "test.py"
        # Input that triggers catastrophic backtracking: 'a' * N + '!'
        # The '!' forces the engine to exhaust all 2^N prefix splits.
        f.write_text("a" * 25 + "!\n")
        patterns = [
            {"pattern": r"(a+)+b", "message": "ReDoS search test"}
        ]
        start = time.monotonic()
        echoes = check_banned_patterns(str(f), patterns)
        elapsed = time.monotonic() - start
        assert echoes == []  # No match (timeout → skip)
        # Must complete within 3s (timeout is 500ms + thread overhead)
        assert elapsed < 5.0, f"ReDoS guard too slow: {elapsed:.1f}s"


class TestObsoleteTerms:
    def test_detects_term(self, tmp_path):
        f = tmp_path / "test.py"
        f.write_text("class UserProfile:\n    pass\n")
        terms = [{"old": "UserProfile", "new": "Account"}]
        echoes = check_obsolete_terms(str(f), terms)
        assert len(echoes) == 1
        assert echoes[0].check == "obsolete-term"
        assert "Account" in echoes[0].suggestion

    def test_no_match(self, tmp_path):
        f = tmp_path / "test.py"
        f.write_text("class Account:\n    pass\n")
        terms = [{"old": "UserProfile", "new": "Account"}]
        echoes = check_obsolete_terms(str(f), terms)
        assert echoes == []


class TestIsExcluded:
    def test_default_fixtures_excluded(self):
        assert is_excluded("/proj/tests/fixtures/bad.py", "/proj", [])

    def test_default_fixtures_at_root(self):
        assert is_excluded("/proj/fixtures/bad.py", "/proj", [])

    def test_default_snapshots_excluded(self):
        assert is_excluded("/proj/src/__snapshots__/x.snap", "/proj", [])

    def test_default_node_modules_excluded(self):
        assert is_excluded("/proj/node_modules/pkg/index.js", "/proj", [])

    def test_default_node_modules_at_root(self):
        assert is_excluded("/proj/node_modules/index.js", "/proj", [])

    def test_default_vendor_excluded(self):
        assert is_excluded("/proj/vendor/lib/foo.go", "/proj", [])

    def test_default_vendor_at_root(self):
        assert is_excluded("/proj/vendor/foo.go", "/proj", [])

    def test_default_dist_excluded(self):
        assert is_excluded("/proj/dist/bundle.js", "/proj", [])

    def test_default_build_excluded(self):
        assert is_excluded("/proj/build/output.js", "/proj", [])

    def test_default_pycache_excluded(self):
        assert is_excluded("/proj/src/__pycache__/mod.pyc", "/proj", [])

    def test_filename_not_matched(self):
        # "fixtures" as a filename should NOT be excluded
        assert not is_excluded("/proj/src/fixtures", "/proj", [])

    def test_normal_file_not_excluded(self):
        assert not is_excluded("/proj/src/main.py", "/proj", [])

    def test_user_exclude_pattern(self):
        assert is_excluded("/proj/generated/api.ts", "/proj", ["generated/*"])

    def test_user_exclude_glob(self):
        assert is_excluded("/proj/app/bundle.min.js", "/proj", ["*.min.js"])

    def test_user_exclude_no_false_match(self):
        assert not is_excluded("/proj/src/app.ts", "/proj", ["generated/*"])


class TestFilterSuppressed:
    """Tests for the ecko:ignore suppression logic."""

    def test_inline_ignore_does_not_leak(self, tmp_path):
        """An inline ecko:ignore should NOT suppress the next line."""
        f = tmp_path / "test.py"
        f.write_text("import os  # ecko:ignore\nimport sys\n")
        echoes = [
            Echo(check="unused-imports", line=1, message="os unused"),
            Echo(check="unused-imports", line=2, message="sys unused"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 2

    def test_standalone_comment_suppresses_next_line(self, tmp_path):
        """A standalone # ecko:ignore comment should suppress the line below."""
        f = tmp_path / "test.py"
        f.write_text("# ecko:ignore\nimport os\nimport sys\n")
        echoes = [
            Echo(check="unused-imports", line=2, message="os unused"),
            Echo(check="unused-imports", line=3, message="sys unused"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 3

    def test_targeted_inline_ignore_does_not_leak(self, tmp_path):
        """An inline ecko:ignore[check] should NOT suppress the next line."""
        f = tmp_path / "test.py"
        f.write_text("import os  # ecko:ignore[unused-imports]\nimport sys\n")
        echoes = [
            Echo(check="unused-imports", line=1, message="os unused"),
            Echo(check="unused-imports", line=2, message="sys unused"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 2

    def test_standalone_targeted_ignore_suppresses_next_line(self, tmp_path):
        """A standalone # ecko:ignore[check] should suppress the line below."""
        f = tmp_path / "test.py"
        f.write_text("# ecko:ignore[unused-imports]\nimport os\nimport sys\n")
        echoes = [
            Echo(check="unused-imports", line=2, message="os unused"),
            Echo(check="unused-imports", line=3, message="sys unused"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 3

    def test_js_standalone_comment_suppresses(self, tmp_path):
        """A standalone // ecko:ignore should suppress the next line in JS/TS."""
        f = tmp_path / "test.ts"
        f.write_text("// ecko:ignore\nvar x = 1;\nvar y = 2;\n")
        echoes = [
            Echo(check="var-declarations", line=2, message="use let"),
            Echo(check="var-declarations", line=3, message="use let"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 3

    def test_js_inline_ignore_does_not_leak(self, tmp_path):
        """An inline // ecko:ignore in JS should NOT suppress the next line."""
        f = tmp_path / "test.ts"
        f.write_text("var x = 1; // ecko:ignore\nvar y = 2;\n")
        echoes = [
            Echo(check="var-declarations", line=1, message="use let"),
            Echo(check="var-declarations", line=2, message="use let"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 1
        assert filtered[0].line == 2

    def test_inline_ignore_still_suppresses_own_line(self, tmp_path):
        """An inline ecko:ignore should still suppress its own line."""
        f = tmp_path / "test.py"
        f.write_text("import os  # ecko:ignore\nimport sys\n")
        echoes = [
            Echo(check="unused-imports", line=1, message="os unused"),
        ]
        filtered = filter_suppressed(echoes, str(f))
        assert len(filtered) == 0


class TestIsStandaloneComment:
    def test_python_hash_comment(self):
        assert _is_standalone_comment("# ecko:ignore\n")

    def test_python_indented_comment(self):
        assert _is_standalone_comment("    # ecko:ignore\n")

    def test_js_double_slash(self):
        assert _is_standalone_comment("// ecko:ignore\n")

    def test_css_block_comment(self):
        assert _is_standalone_comment("/* ecko:ignore */\n")

    def test_html_comment(self):
        assert _is_standalone_comment("<!-- ecko:ignore -->\n")

    def test_inline_code_is_not_standalone(self):
        assert not _is_standalone_comment("import os  # ecko:ignore\n")

    def test_js_inline_is_not_standalone(self):
        assert not _is_standalone_comment("var x = 1; // ecko:ignore\n")


class TestBannedPatternsRelativePath:
    def test_glob_matches_relative_path(self, tmp_path):
        """Glob patterns like 'src/*.tsx' should match against relative paths."""
        src = tmp_path / "src"
        src.mkdir()
        f = src / "app.tsx"
        f.write_text('<div className="bg-blue-500">hello</div>\n')
        patterns = [
            {"pattern": r"bg-blue-\d+", "glob": "src/*.tsx", "message": "bad"}
        ]
        echoes = check_banned_patterns(str(f), patterns, cwd=str(tmp_path))
        assert len(echoes) == 1

    def test_basename_glob_still_works_with_cwd(self, tmp_path):
        """Simple basename globs should still work when cwd is provided."""
        src = tmp_path / "src"
        src.mkdir()
        f = src / "app.tsx"
        f.write_text('<div className="bg-blue-500">hello</div>\n')
        patterns = [
            {"pattern": r"bg-blue-\d+", "glob": "*.tsx", "message": "bad"}
        ]
        echoes = check_banned_patterns(str(f), patterns, cwd=str(tmp_path))
        assert len(echoes) == 1

    def test_relative_glob_no_false_match(self, tmp_path):
        """A glob like 'src/*.tsx' should not match files outside src/."""
        lib = tmp_path / "lib"
        lib.mkdir()
        f = lib / "app.tsx"
        f.write_text('<div className="bg-blue-500">hello</div>\n')
        patterns = [
            {"pattern": r"bg-blue-\d+", "glob": "src/*.tsx", "message": "bad"}
        ]
        echoes = check_banned_patterns(str(f), patterns, cwd=str(tmp_path))
        assert len(echoes) == 0

    def test_no_cwd_falls_back_to_basename(self, tmp_path):
        """Without cwd, only basename matching should be used (backward compat)."""
        src = tmp_path / "src"
        src.mkdir()
        f = src / "app.tsx"
        f.write_text('<div className="bg-blue-500">hello</div>\n')
        patterns = [
            {"pattern": r"bg-blue-\d+", "glob": "src/*.tsx", "message": "bad"}
        ]
        # No cwd — should fall back to basename-only matching
        echoes = check_banned_patterns(str(f), patterns)
        assert len(echoes) == 0  # "src/*.tsx" doesn't match basename "app.tsx"


class TestNormalizePath:
    def test_relative_path_made_absolute(self):
        result = _normalize_path("src/app.py", "/home/user/project")
        assert result == os.path.normpath("/home/user/project/src/app.py")

    def test_absolute_path_unchanged(self):
        result = _normalize_path("/home/user/project/src/app.py", "/home/user/project")
        assert result == os.path.normpath("/home/user/project/src/app.py")

    def test_dots_resolved(self):
        result = _normalize_path("src/../lib/app.py", "/home/user/project")
        assert result == os.path.normpath("/home/user/project/lib/app.py")

    def test_trailing_slash_normalized(self):
        result = _normalize_path("src/app.py", "/home/user/project/")
        assert result == os.path.normpath("/home/user/project/src/app.py")


class TestIsTestFile:
    def test_test_prefix(self):
        assert is_test_file("/proj/tests/test_foo.py")

    def test_test_suffix(self):
        assert is_test_file("/proj/foo_test.py")

    def test_helpers_not_test_file(self):
        assert not is_test_file("/proj/tests/helpers.py")

    def test_utils_not_test_file(self):
        assert not is_test_file("/proj/test/utils.py")

    def test_regular_file(self):
        assert not is_test_file("/proj/src/main.py")

    def test_conftest(self):
        assert is_test_file("/proj/tests/conftest.py")

    def test_conftest_at_root(self):
        assert is_test_file("/proj/conftest.py")

    def test_conftest_pyi(self):
        assert is_test_file("/proj/tests/conftest.pyi")


class TestTestConditional:
    def test_detects_if_in_test(self):
        echoes = check_test_quality(str(FIXTURES / "test_conditional.py"))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 1
        assert conditional_echoes[0].line == 6  # the if result.startswith line

    def test_clean_test_no_echoes(self):
        echoes = check_test_quality(str(FIXTURES / "test_clean_test.py"))
        assert echoes == []

    def test_skips_name_main_guard(self, tmp_path):
        f = tmp_path / "test_guard.py"
        f.write_text(
            'def test_foo():\n    assert True\n\n'
            'if __name__ == "__main__":\n    test_foo()\n',
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        assert echoes == []

    def test_skips_type_checking(self, tmp_path):
        f = tmp_path / "test_types.py"
        f.write_text(
            "from typing import TYPE_CHECKING\n\n"
            "def test_foo():\n"
            "    if TYPE_CHECKING:\n"
            "        pass\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_version_guard(self, tmp_path):
        f = tmp_path / "test_compat.py"
        f.write_text(
            "import sys\n\n"
            "def test_foo():\n"
            "    if sys.version_info >= (3, 10):\n"
            "        pass\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_version_info_subscript(self, tmp_path):
        """sys.version_info[:2] >= (3, 13) should be treated as a version guard."""
        f = tmp_path / "test_ver.py"
        f.write_text(
            "import sys\n\n"
            "def test_foo():\n"
            "    if sys.version_info[:2] >= (3, 13):\n"
            "        pass\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_nested_function_conditional(self, tmp_path):
        """Conditionals inside nested helper functions should not be flagged."""
        f = tmp_path / "test_nested.py"
        f.write_text(
            "def test_with_helper():\n"
            "    def helper(x):\n"
            "        if x > 0:\n"
            "            return x\n"
            "        return 0\n"
            "    assert helper(5) == 5\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_nested_test_prefixed_function(self, tmp_path):
        """A nested function named test_* should not be treated as a test."""
        f = tmp_path / "test_click_style.py"
        f.write_text(
            "def test_prompts():\n"
            "    def test_no():\n"
            "        if confirm('Foo'):\n"
            "            pass\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_skiptest_guard(self, tmp_path):
        """Guard-then-skipTest clauses should not be flagged."""
        f = tmp_path / "test_skip.py"
        f.write_text(
            "def test_guarded(self):\n"
            "    if not hasattr(self, 'feature'):\n"
            "        self.skipTest('feature not available')\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_pytest_skip_guard(self, tmp_path):
        """Guard-then-pytest.skip clauses should not be flagged."""
        f = tmp_path / "test_skip2.py"
        f.write_text(
            "import pytest\n\n"
            "def test_guarded():\n"
            "    if not some_condition:\n"
            "        pytest.skip('not supported')\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_early_return_guard(self, tmp_path):
        """Early return guards should not be flagged."""
        f = tmp_path / "test_return.py"
        f.write_text(
            "def test_guarded():\n"
            "    if not precondition:\n"
            "        return\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_raise_pytest_skip(self, tmp_path):
        """raise pytest.skip(...) should be treated as a guard clause."""
        f = tmp_path / "test_raise_skip.py"
        f.write_text(
            "import pytest\n\n"
            "def test_guarded():\n"
            "    if 'VAR' in os.environ:\n"
            "        raise pytest.skip('not in CI')\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_pytest_fail_guard(self, tmp_path):
        """if condition: pytest.fail(...) should be treated as a guard."""
        f = tmp_path / "test_fail_guard.py"
        f.write_text(
            "import pytest\n\n"
            "def test_guarded():\n"
            "    if not expected:\n"
            "        pytest.fail('missing expected value')\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_os_name_guard(self, tmp_path):
        """os.name == 'nt' should be treated as a platform guard."""
        f = tmp_path / "test_os.py"
        f.write_text(
            "import os\n\n"
            "def test_platform():\n"
            "    if os.name != 'nt':\n"
            "        assert True\n"
            "    else:\n"
            "        assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0


    def test_skips_data_filter_if_in_for_loop(self, tmp_path):
        """if inside for loop with no assert is data filtering — not a test conditional."""
        f = tmp_path / "test_filter.py"
        f.write_text(
            "def test_validate_output():\n"
            "    with open('data.jsonl') as fh:\n"
            "        for line in fh:\n"
            "            if line.strip():\n"
            "                data = json.loads(line)\n"
            "    assert data is not None\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_data_filter_if_in_while_loop(self, tmp_path):
        """if inside while loop with no assert is data filtering."""
        f = tmp_path / "test_while.py"
        f.write_text(
            "def test_drain_queue():\n"
            "    while not q.empty():\n"
            "        item = q.get()\n"
            "        if item is not None:\n"
            "            results.append(item)\n"
            "    assert len(results) == 3\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_flags_if_in_for_loop_with_assert(self, tmp_path):
        """if inside for loop WITH assert is a real test conditional — should flag."""
        f = tmp_path / "test_loop_assert.py"
        f.write_text(
            "def test_all_items_valid():\n"
            "    for item in items:\n"
            "        if item.active:\n"
            "            assert item.valid\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 1

    def test_flags_if_outside_loop(self, tmp_path):
        """if outside a loop is still flagged (existing behavior preserved)."""
        f = tmp_path / "test_branch.py"
        f.write_text(
            "def test_platform():\n"
            "    result = get_result()\n"
            "    if result > 0:\n"
            "        assert result == 42\n"
            "    else:\n"
            "        assert result == 0\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 1

    def test_skips_nested_filter_in_async_for(self, tmp_path):
        """if inside async for with no assert is data filtering."""
        f = tmp_path / "test_async.py"
        f.write_text(
            "async def test_stream():\n"
            "    async for chunk in stream:\n"
            "        if chunk:\n"
            "            data.extend(chunk)\n"
            "    assert len(data) > 0\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0

    def test_skips_loop_filter_with_nested_function_assert(self, tmp_path):
        """if in loop with assert only inside nested function is still data filtering."""
        f = tmp_path / "test_nested_fn.py"
        f.write_text(
            "def test_process():\n"
            "    for item in items:\n"
            "        if item.valid:\n"
            "            def checker():\n"
            "                assert item.ok\n"
            "            checker()\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        conditional_echoes = [e for e in echoes if e.check == "test-conditional"]
        assert len(conditional_echoes) == 0


class TestFixedWait:
    def test_detects_sleep(self):
        echoes = check_test_quality(str(FIXTURES / "test_fixed_wait.py"))
        wait_echoes = [e for e in echoes if e.check == "fixed-wait"]
        assert len(wait_echoes) == 3  # time.sleep, asyncio.sleep, wait_for_timeout

    def test_clean_test_no_waits(self):
        echoes = check_test_quality(str(FIXTURES / "test_clean_test.py"))
        wait_echoes = [e for e in echoes if e.check == "fixed-wait"]
        assert wait_echoes == []

    def test_skips_sleep_in_nested_function(self, tmp_path):
        """Sleep inside nested helper functions should not be flagged."""
        f = tmp_path / "test_nested_sleep.py"
        f.write_text(
            "import asyncio\n\n"
            "def test_async_helper():\n"
            "    async def simulate_work():\n"
            "        await asyncio.sleep(0.1)\n"
            "        return 'done'\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        wait_echoes = [e for e in echoes if e.check == "fixed-wait"]
        assert wait_echoes == []

    def test_skips_asyncio_sleep_zero(self, tmp_path):
        """asyncio.sleep(0) is an event-loop yield, not a fixed wait."""
        f = tmp_path / "test_yield.py"
        f.write_text(
            "import asyncio\n\n"
            "async def test_yield():\n"
            "    await asyncio.sleep(0)\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        wait_echoes = [e for e in echoes if e.check == "fixed-wait"]
        assert wait_echoes == []

    def test_skips_time_sleep_zero(self, tmp_path):
        """time.sleep(0) is an idiomatic GIL yield, not a fixed wait."""
        f = tmp_path / "test_yield2.py"
        f.write_text(
            "import time\n\n"
            "def test_yield():\n"
            "    time.sleep(0)\n"
            "    assert True\n",
            encoding="utf-8",
        )
        echoes = check_test_quality(str(f))
        wait_echoes = [e for e in echoes if e.check == "fixed-wait"]
        assert wait_echoes == []


class TestMockSpecBypass:
    def test_detects_bypass(self):
        echoes = check_test_quality(str(FIXTURES / "test_mock_spec.py"))
        mock_echoes = [e for e in echoes if e.check == "mock-spec-bypass"]
        assert len(mock_echoes) == 2  # bypass_spec + magicmock_spec

    def test_allows_return_value(self):
        echoes = check_test_quality(str(FIXTURES / "test_mock_spec.py"))
        mock_echoes = [e for e in echoes if e.check == "mock-spec-bypass"]
        # return_value and side_effect should NOT be flagged
        attrs_flagged = {e.message for e in mock_echoes}
        assert not any("return_value" in m for m in attrs_flagged)
        assert not any("side_effect" in m for m in attrs_flagged)

    def test_no_spec_not_flagged(self):
        echoes = check_test_quality(str(FIXTURES / "test_mock_spec.py"))
        mock_echoes = [e for e in echoes if e.check == "mock-spec-bypass"]
        # test_no_spec sets .anything on Mock() without spec — should not be flagged
        assert not any("anything" in e.message for e in mock_echoes)

    def test_nonexistent_file(self):
        echoes = check_test_quality("/nonexistent/test_file.py")
        assert echoes == []


class TestBannedPatternsFinditer:
    """Test that banned patterns use finditer with correct line numbers."""

    def test_correct_line_numbers(self, tmp_path):
        target = tmp_path / "sample.py"
        target.write_text("line1\nTODO fix this\nline3\nTODO another\n")
        patterns = [{"pattern": r"TODO", "message": "No TODOs"}]
        echoes = check_banned_patterns(str(target), patterns)
        assert len(echoes) == 2
        assert echoes[0].line == 2
        assert echoes[1].line == 4

    def test_empty_file(self, tmp_path):
        target = tmp_path / "empty.py"
        target.write_text("")
        patterns = [{"pattern": r"TODO", "message": "No TODOs"}]
        echoes = check_banned_patterns(str(target), patterns)
        assert echoes == []

    def test_single_line_no_newline(self, tmp_path):
        target = tmp_path / "single.py"
        target.write_text("TODO fix")
        patterns = [{"pattern": r"TODO", "message": "No TODOs"}]
        echoes = check_banned_patterns(str(target), patterns)
        assert len(echoes) == 1
        assert echoes[0].line == 1


class TestConfigWarningDedup:
    """Config warnings should emit once per cwd."""

    def test_dedup_same_cwd(self, tmp_path):
        from checks.runner import _config_warned

        cwd = str(tmp_path / "dedup_test_same")
        _config_warned.discard(cwd)  # Ensure clean state

        config = {"zzz_unknown_key": True}

        import io
        import sys

        # First call — should emit
        buf1 = io.StringIO()
        old = sys.stderr
        sys.stderr = buf1
        try:
            from checks.runner import _emit_config_warnings
            _emit_config_warnings(config, cwd)
        finally:
            sys.stderr = old
        assert "zzz_unknown_key" in buf1.getvalue()

        # Second call — should NOT emit (deduped)
        buf2 = io.StringIO()
        sys.stderr = buf2
        try:
            _emit_config_warnings(config, cwd)
        finally:
            sys.stderr = old
        assert buf2.getvalue() == ""

        _config_warned.discard(cwd)  # Cleanup

    def test_different_cwds_both_emit(self, tmp_path):
        from checks.runner import _config_warned, _emit_config_warnings

        cwd1 = str(tmp_path / "dedup_test_a")
        cwd2 = str(tmp_path / "dedup_test_b")
        _config_warned.discard(cwd1)
        _config_warned.discard(cwd2)

        config = {"zzz_unknown_key": True}

        import io
        import sys

        buf1 = io.StringIO()
        old = sys.stderr
        sys.stderr = buf1
        try:
            _emit_config_warnings(config, cwd1)
        finally:
            sys.stderr = old
        assert "zzz_unknown_key" in buf1.getvalue()

        buf2 = io.StringIO()
        sys.stderr = buf2
        try:
            _emit_config_warnings(config, cwd2)
        finally:
            sys.stderr = old
        assert "zzz_unknown_key" in buf2.getvalue()

        _config_warned.discard(cwd1)
        _config_warned.discard(cwd2)
