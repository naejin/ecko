"""Tests for custom checks (no external tools needed)."""

from __future__ import annotations

from pathlib import Path

from checks.custom.banned_patterns import check_banned_patterns, check_obsolete_terms
from checks.custom.duplicate_keys import check_duplicate_keys
from checks.custom.unicode_artifacts import check_unicode_artifacts
from checks.custom.unreachable_code import check_unreachable_code

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

    def test_clean_file(self):
        echoes = check_unicode_artifacts(str(FIXTURES / "clean.py"))
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
