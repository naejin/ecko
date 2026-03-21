"""Tests for dry-run mode."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

RUNNER = str(Path(__file__).parent.parent / "checks" / "runner.py")
PLUGIN_ROOT = str(Path(__file__).parent.parent)


def _run_dry_run(file_path: str, cwd: str | None = None) -> tuple[int, str]:
    """Run ecko in dry-run mode and return (exit_code, stdout)."""
    result = subprocess.run(
        [
            sys.executable,
            RUNNER,
            "--file",
            file_path,
            "--mode",
            "dry-run",
            "--cwd",
            cwd or "/tmp",
            "--plugin-root",
            PLUGIN_ROOT,
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )
    return result.returncode, result.stdout


class TestDryRun:
    def test_python_file(self):
        code, output = _run_dry_run("test.py")
        assert code == 0
        assert "Language: python" in output
        assert "ruff:" in output
        assert "duplicate-keys:" in output
        assert "unreachable-code:" in output
        assert "unicode-artifact:" in output

    def test_typescript_file(self):
        code, output = _run_dry_run("app.ts")
        assert code == 0
        assert "Language: typescript" in output
        assert "biome:" in output
        assert "tsc:" in output

    def test_go_file(self):
        code, output = _run_dry_run("main.go")
        assert code == 0
        assert "Language: go" in output
        assert "golangci-lint:" in output

    def test_rust_file(self):
        code, output = _run_dry_run("lib.rs")
        assert code == 0
        assert "Language: rust" in output
        assert "clippy:" in output

    def test_unknown_extension(self):
        code, output = _run_dry_run("file.xyz")
        assert code == 0
        assert "No checks configured" in output

    def test_disabled_checks_shown(self, tmp_path):
        config = tmp_path / "ecko.yaml"
        config.write_text("disabled_checks:\n  - ruff\n  - dead-code\n", encoding="utf-8")
        code, output = _run_dry_run("test.py", cwd=str(tmp_path))
        assert code == 0
        assert "disabled" in output.lower()

    def test_always_returns_zero(self):
        code, _ = _run_dry_run("test.py")
        assert code == 0
