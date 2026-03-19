"""Integration tests — run the full runner with real tools via uvx/npx."""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

PLUGIN_ROOT = str(Path(__file__).parent.parent)
RUNNER = str(Path(__file__).parent.parent / "checks" / "runner.py")
FIXTURES = Path(__file__).parent / "fixtures"

has_uvx = shutil.which("uvx") is not None
has_npx = shutil.which("npx") is not None


def run_ecko(fixture: str, cwd: str | None = None) -> tuple[int, str]:
    """Copy fixture to temp dir, run ecko on it. Returns (exit_code, stderr).

    Copies to prevent Layer 1 auto-fix from modifying the original fixtures.
    """
    with tempfile.TemporaryDirectory() as tmp:
        src = FIXTURES / fixture
        dst = Path(tmp) / fixture
        shutil.copy2(src, dst)
        result = subprocess.run(
            [
                sys.executable,
                RUNNER,
                "--file",
                str(dst),
                "--mode",
                "post-tool-use",
                "--cwd",
                cwd or tmp,
                "--plugin-root",
                PLUGIN_ROOT,
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )
        return result.returncode, result.stderr


class TestCleanFiles:
    def test_clean_python(self):
        code, output = run_ecko("clean.py")
        assert code == 0
        assert output == ""

    @pytest.mark.skipif(not has_npx, reason="npx not available")
    def test_clean_typescript(self):
        code, output = run_ecko("clean.ts")
        assert code == 0
        assert output == ""


class TestPythonCustomChecks:
    """Custom checks work without any external tools."""

    def test_duplicate_keys(self):
        code, output = run_ecko("duplicate_keys.py")
        assert code == 1
        assert "duplicate-keys" in output

    def test_unreachable_code(self):
        code, output = run_ecko("unreachable.py")
        assert code == 1
        assert "unreachable-code" in output

    def test_unicode_artifacts_js(self):
        code, output = run_ecko("unicode_artifacts.js")
        assert code == 1
        assert "unicode-artifact" in output

    def test_unicode_skips_js_strings(self):
        code, output = run_ecko("unicode_in_js_strings.ts")
        assert code == 0
        assert "unicode-artifact" not in output

    def test_unicode_detects_js_code(self):
        code, output = run_ecko("unicode_in_js_code.ts")
        assert code == 1
        assert "unicode-artifact" in output
        assert output.count("unicode-artifact") == 2


class TestSuppression:
    def test_suppressed_echoes(self):
        code, output = run_ecko("suppressed.py")
        assert code == 0
        assert output == ""


@pytest.mark.skipif(not has_uvx, reason="uvx not available")
class TestRuffViaUvx:
    def test_unused_imports(self):
        code, output = run_ecko("unused_imports.py")
        assert code == 1
        assert "unused-imports" in output
        # Should find all 3 unused imports
        assert output.count("unused-imports") >= 3


@pytest.mark.skipif(not has_npx, reason="npx not available")
class TestBiomeViaNpx:
    def test_biome_issues(self):
        code, output = run_ecko("biome_issues.ts")
        if code == 0 and not output:
            pytest.skip("biome not available or failed to run on this platform")
        assert code == 1
        # Check for various biome echoes
        assert "unused-imports" in output
        assert "debugger-statements" in output
        assert "unreachable-code" in output
        assert "duplicate-keys" in output
        assert "var-declarations" in output
        assert "useless-catch" in output


def _init_git_repo(path: Path) -> None:
    """Initialize a git repo with a dummy user config for testing."""
    subprocess.run(["git", "init", "-q"], cwd=path, capture_output=True, check=True)
    subprocess.run(
        ["git", "config", "user.email", "test@ecko.dev"],
        cwd=path, capture_output=True, check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Ecko Test"],
        cwd=path, capture_output=True, check=True,
    )


def run_ecko_stop(cwd: str) -> tuple[int, str]:
    """Run ecko in stop mode.  Returns (exit_code, stderr)."""
    result = subprocess.run(
        [
            sys.executable,
            RUNNER,
            "--mode",
            "stop",
            "--cwd",
            cwd,
            "--plugin-root",
            PLUGIN_ROOT,
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    return result.returncode, result.stderr


class TestStopMode:
    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_detects_issues_in_modified_files(self, tmp_path):
        """Stop mode should detect issues in git-modified files."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )

        # Modify with issues
        clean.write_text("import os\nimport sys\n\ndef hello():\n    return 'hello'\n")

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 1
        assert "unused-imports" in output

    def test_clean_repo_no_issues(self, tmp_path):
        """Stop mode on a repo with no uncommitted changes should exit 0."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 0
        assert output == ""

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_no_duplicate_paths(self, tmp_path):
        """The same file must not appear under different path representations."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        clean.write_text("import os\nimport sys\n\ndef hello():\n    return 'hello'\n")

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 1
        # File headers in the stop output end with ":"
        path_headers = [
            line for line in output.splitlines()
            if "app.py" in line and line.strip().endswith(":")
        ]
        assert len(path_headers) <= 1, f"Duplicate path entries: {path_headers}"

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_detects_untracked_files(self, tmp_path):
        """Stop mode should pick up brand-new untracked files."""
        _init_git_repo(tmp_path)
        dummy = tmp_path / "README"
        dummy.write_text("placeholder\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        # Add a new file (untracked) with issues
        new = tmp_path / "new_module.py"
        new.write_text("import os\nimport sys\n")

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 1
        assert "unused-imports" in output

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_suppression_applies_to_all_layers(self, tmp_path):
        """ecko:ignore should suppress echoes from both Layer 2 and Layer 3."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        # Both imports are unused; suppress one with ecko:ignore
        clean.write_text(
            "import os  # ecko:ignore\nimport sys\n\ndef hello():\n    return 'hello'\n"
        )

        code, output = run_ecko_stop(str(tmp_path))
        # os should be suppressed everywhere, sys should still be reported
        assert "unused-imports" in output
        # Count occurrences of "os" in echo lines (should be 0 or only from
        # dead-code which uses different wording). The key assertion is that
        # the suppressed import doesn't appear as "unused-imports".
        echo_lines = [
            line for line in output.splitlines()
            if "unused-imports" in line
        ]
        for line in echo_lines:
            assert "`os`" not in line, f"Suppressed import leaked: {line}"


class TestAutofix:
    def test_trailing_whitespace_stripped(self):
        """Layer 1 should strip trailing whitespace."""
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "test.py"
            f.write_text("def hello():   \n    return 'hi'  \n")

            subprocess.run(
                [
                    sys.executable,
                    RUNNER,
                    "--file",
                    str(f),
                    "--mode",
                    "post-tool-use",
                    "--cwd",
                    tmp,
                    "--plugin-root",
                    PLUGIN_ROOT,
                ],
                capture_output=True,
                text=True,
                timeout=60,
            )

            content = f.read_text()
            assert "   \n" not in content
            assert "  \n" not in content

    def test_autofix_does_not_crash_on_binary(self):
        """Layer 1 should gracefully handle binary files."""
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "data.py"
            f.write_bytes(b"\x00\x01\x02\xff\xfe\n")

            result = subprocess.run(
                [
                    sys.executable,
                    RUNNER,
                    "--file",
                    str(f),
                    "--mode",
                    "post-tool-use",
                    "--cwd",
                    tmp,
                    "--plugin-root",
                    PLUGIN_ROOT,
                ],
                capture_output=True,
                text=True,
                timeout=60,
            )
            # Should not crash — exit 0 or 1, but no exit 2 / traceback
            assert result.returncode in (0, 1)
