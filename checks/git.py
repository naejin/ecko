"""Git file detection utilities."""

from __future__ import annotations

import os
import subprocess


def normalize_path(path: str, cwd: str) -> str:
    """Normalize a file path to absolute, resolving relative paths against cwd."""
    if os.path.isabs(path):
        return os.path.normpath(path)
    return os.path.normpath(os.path.join(cwd, path))


def get_modified_files(cwd: str, session_hours: float = 4.0) -> list[str]:
    """Get files modified in the current session via git."""
    files: set[str] = set()
    since_arg = f"--since={int(session_hours * 60)}m"
    try:
        # Staged changes
        result = subprocess.run(
            ["git", "diff", "--name-only", "--cached"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))

        # Unstaged changes
        result = subprocess.run(
            ["git", "diff", "--name-only"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))

        # Untracked files
        result = subprocess.run(
            ["git", "ls-files", "--others", "--exclude-standard"],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))

        # Recently committed files (catch files committed during this session)
        result = subprocess.run(
            [
                "git", "log", since_arg,
                "--diff-filter=ACMR", "--name-only", "--pretty=format:",
            ],
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=10,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                if line:
                    files.add(os.path.join(cwd, line))
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    return sorted(files)
