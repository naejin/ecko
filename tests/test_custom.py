"""Tests for custom checks (no external tools needed)."""

from __future__ import annotations

import os
from pathlib import Path

from checks.custom.banned_patterns import check_banned_patterns, check_obsolete_terms
from checks.custom.duplicate_keys import check_duplicate_keys
from checks.custom.unicode_artifacts import check_unicode_artifacts
from checks.custom.unreachable_code import check_unreachable_code
from checks.result import Echo
from checks.runner import _is_standalone_comment, _normalize_path, filter_suppressed, is_excluded

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
