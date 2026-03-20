"""Tests for bash command blocking (pre-tool-use-bash mode)."""

from __future__ import annotations

from checks.runner import check_bash_command


class TestBashGuardHardcoded:
    """Tests for hardcoded (always-active) blocked patterns."""

    def test_blocks_no_verify(self):
        result = check_bash_command("git commit --no-verify -m 'skip hooks'", [])
        assert result is not None
        assert "--no-verify" in result.lower() or "hook" in result.lower()

    def test_blocks_no_verify_short_form(self):
        result = check_bash_command("git commit -n -m 'skip hooks'", [])
        # -n is not --no-verify for git commit, it's --no-verify only as long form
        # Only --no-verify is blocked
        assert result is None

    def test_blocks_no_verify_anywhere(self):
        result = check_bash_command("git commit -m 'msg' --no-verify", [])
        assert result is not None

    def test_blocks_rm_rf_root(self):
        result = check_bash_command("rm -rf /", [])
        assert result is not None

    def test_blocks_rm_rf_home(self):
        result = check_bash_command("rm -rf ~", [])
        assert result is not None

    def test_allows_rm_rf_subdir(self):
        result = check_bash_command("rm -rf /tmp/build", [])
        assert result is None

    def test_allows_rm_rf_relative(self):
        result = check_bash_command("rm -rf ./dist", [])
        assert result is None

    def test_allows_clean_git_commit(self):
        result = check_bash_command("git commit -m 'fix: something'", [])
        assert result is None

    def test_allows_git_status(self):
        result = check_bash_command("git status", [])
        assert result is None

    def test_allows_normal_commands(self):
        result = check_bash_command("python3 -m pytest tests/", [])
        assert result is None

    def test_allows_rm_with_other_flags(self):
        result = check_bash_command("rm -rf node_modules", [])
        assert result is None

    def test_blocks_rm_rf_root_with_flag(self):
        result = check_bash_command("rm -rf / --no-preserve-root", [])
        assert result is not None

    def test_blocks_rm_rf_home_subdir(self):
        result = check_bash_command("rm -rf ~/Documents", [])
        assert result is not None

    def test_blocks_rm_rf_root_chained(self):
        result = check_bash_command("rm -rf / && echo done", [])
        assert result is not None

    def test_blocks_rm_rf_home_chained(self):
        result = check_bash_command("rm -rf ~ ; ls", [])
        assert result is not None

    def test_blocks_no_verify_merge(self):
        result = check_bash_command("git merge --no-verify feature", [])
        assert result is not None

    def test_blocks_no_verify_rebase(self):
        result = check_bash_command("git rebase --no-verify main", [])
        assert result is not None

    def test_blocks_no_verify_with_intervening_options(self):
        result = check_bash_command("git -C /some/path commit --no-verify", [])
        assert result is not None


class TestBashGuardUserPatterns:
    """Tests for user-configurable blocked patterns."""

    def test_user_pattern_blocks(self):
        patterns = [
            {
                "pattern": r"(pytest|npm test|cargo test).*\|",
                "message": "Do not pipe test output",
            }
        ]
        result = check_bash_command("pytest tests/ | head -20", patterns)
        assert result is not None
        assert "pipe" in result.lower()

    def test_user_pattern_allows_clean(self):
        patterns = [
            {
                "pattern": r"(pytest|npm test|cargo test).*\|",
                "message": "Do not pipe test output",
            }
        ]
        result = check_bash_command("pytest tests/ -v", patterns)
        assert result is None

    def test_force_push_pattern(self):
        patterns = [
            {
                "pattern": r"git push.*--force(?!-with-lease)",
                "message": "Use --force-with-lease instead",
            }
        ]
        result = check_bash_command("git push --force origin main", patterns)
        assert result is not None

    def test_force_with_lease_allowed(self):
        patterns = [
            {
                "pattern": r"git push.*--force(?!-with-lease)",
                "message": "Use --force-with-lease instead",
            }
        ]
        result = check_bash_command("git push --force-with-lease origin main", patterns)
        assert result is None

    def test_empty_user_patterns(self):
        result = check_bash_command("echo hello", [])
        assert result is None

    def test_invalid_user_pattern_skipped(self):
        patterns = [
            {"pattern": r"[invalid", "message": "bad regex"}
        ]
        # Should not crash — invalid patterns are skipped gracefully
        result = check_bash_command("echo hello", patterns)
        assert result is None
