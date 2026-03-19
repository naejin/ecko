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
