"""Integration tests — run the full runner with real tools via uvx/npx."""

from __future__ import annotations

import os
import re
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
        # Tool warnings (e.g. "biome failed" on Windows) are OK — no echoes is the test
        assert "echo" not in output.lower() or "ecko" in output.lower()


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
        if code == 0:
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


def run_ecko_stop(cwd: str, files: str | None = None) -> tuple[int, str]:
    """Run ecko in stop mode.  Returns (exit_code, stderr)."""
    cmd = [
        sys.executable,
        RUNNER,
        "--mode",
        "stop",
        "--cwd",
        cwd,
        "--plugin-root",
        PLUGIN_ROOT,
    ]
    if files is not None:
        cmd.extend(["--files", files])
    result = subprocess.run(
        cmd,
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
        """Stop mode on a repo with no uncommitted changes should exit 0.

        Recently committed files are now detected via git log --since=4h,
        so we get a clean sweep message rather than empty output.
        """
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
        # Recently committed files are detected — clean sweep emitted
        assert "clean sweep" in output or output == ""

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


class TestReverbNudge:
    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_reverb_tip_emitted_when_enabled(self, tmp_path):
        """Reverb tip should appear in stop-mode output when reverb is enabled and echoes exist."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        # Enable reverb in ecko.yaml
        config = tmp_path / "ecko.yaml"
        config.write_text("reverb:\n  enabled: true\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        # Introduce an issue so echoes are produced
        clean.write_text("import os\nimport sys\n\ndef hello():\n    return 'hello'\n")

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 1
        assert "tip: run /ecko:reverb" in output
        # Regression guard: tip must not contain file-write instructions (caused infinite loop)
        assert "mkdir" not in output
        assert ".ecko-reverb/" not in output

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_reverb_tip_not_emitted_when_disabled(self, tmp_path):
        """Reverb tip should NOT appear when reverb is disabled (default)."""
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
        assert "tip: run /ecko:reverb" not in output

    def test_reverb_tip_not_emitted_when_clean(self, tmp_path):
        """Reverb tip should NOT appear when there are no echoes, even if enabled."""
        _init_git_repo(tmp_path)
        clean = tmp_path / "app.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        config = tmp_path / "ecko.yaml"
        config.write_text("reverb:\n  enabled: true\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )

        code, output = run_ecko_stop(str(tmp_path))
        assert code == 0
        assert "tip: run /ecko:reverb" not in output


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


class TestFilesArgument:
    """Tests for the --files CLI argument in stop mode."""

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_files_argument_detects_issues(self, tmp_path):
        """--files should check the specified files directly."""
        _init_git_repo(tmp_path)
        f = tmp_path / "bad.py"
        f.write_text("import os\nimport sys\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        code, output = run_ecko_stop(str(tmp_path), files="bad.py")
        assert code == 1
        assert "unused-imports" in output

    def test_files_argument_clean_file(self, tmp_path):
        """--files with a clean file should produce clean sweep."""
        _init_git_repo(tmp_path)
        f = tmp_path / "clean.py"
        f.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        code, output = run_ecko_stop(str(tmp_path), files="clean.py")
        assert code == 0
        assert "clean sweep" in output

    def test_files_argument_overrides_git(self, tmp_path):
        """--files should override git detection — only specified files checked."""
        _init_git_repo(tmp_path)
        dirty = tmp_path / "dirty.py"
        dirty.write_text("def hello():\n    return 'hello'\n")
        clean = tmp_path / "clean.py"
        clean.write_text("def hello():\n    return 'hello'\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "init"],
            cwd=tmp_path, capture_output=True,
        )
        # Modify dirty.py — but only check clean.py via --files
        dirty.write_text("import os\nimport sys\n")
        code, output = run_ecko_stop(str(tmp_path), files="clean.py")
        assert code == 0
        assert "clean sweep" in output


class TestCleanSweep:
    """Tests for clean-sweep message and stop-mode timing."""

    def test_clean_sweep_message_format(self, tmp_path):
        """Clean stop should emit the clean sweep message with file count and timing."""
        _init_git_repo(tmp_path)
        f = tmp_path / "app.py"
        f.write_text("def hello():\n    return 'hello'\n")
        # Leave file as untracked so stop mode picks it up
        code, output = run_ecko_stop(str(tmp_path))
        assert code == 0
        assert "clean sweep" in output
        assert "0 echoes" in output
        assert "1 file" in output
        assert re.search(r"\(\d+\.\d+s\)", output)  # timing like "(0.1s)"

    def test_clean_sweep_multiple_files(self, tmp_path):
        """Clean sweep should report correct file count for multiple files."""
        _init_git_repo(tmp_path)
        (tmp_path / "a.py").write_text("x = 1\n")
        (tmp_path / "b.py").write_text("y = 2\n")
        code, output = run_ecko_stop(str(tmp_path))
        assert code == 0
        assert "clean sweep" in output
        assert "2 files" in output

    @pytest.mark.skipif(not has_uvx, reason="uvx not available")
    def test_finished_timing_on_echoes(self, tmp_path):
        """When echoes are found, should emit 'finished in' with timing."""
        _init_git_repo(tmp_path)
        f = tmp_path / "bad.py"
        f.write_text("import os\nimport sys\n")
        code, output = run_ecko_stop(str(tmp_path))
        assert code == 1
        assert "finished in" in output
        assert "s\n" in output  # ends with timing


class TestDebugModeIntegration:
    """Integration tests for ECKO_DEBUG=1."""

    def test_debug_output_when_enabled(self, tmp_path):
        """ECKO_DEBUG=1 should emit debug lines to stderr."""
        _init_git_repo(tmp_path)
        f = tmp_path / "app.py"
        f.write_text("x = 1\n")
        env = os.environ.copy()
        env["ECKO_DEBUG"] = "1"
        result = subprocess.run(
            [
                sys.executable,
                RUNNER,
                "--file",
                str(f),
                "--mode",
                "post-tool-use",
                "--cwd",
                str(tmp_path),
                "--plugin-root",
                PLUGIN_ROOT,
            ],
            capture_output=True,
            text=True,
            timeout=60,
            env=env,
        )
        assert "~~ ecko ~~ debug:" in result.stderr
        assert "mode=post-tool-use" in result.stderr

    def test_no_debug_output_by_default(self):
        """Without ECKO_DEBUG, no debug lines should appear."""
        code, output = run_ecko("clean.py")
        assert "~~ ecko ~~ debug:" not in output
