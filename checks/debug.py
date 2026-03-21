"""Debug output for ecko — enabled via ECKO_DEBUG=1 environment variable."""

from __future__ import annotations

import os
import sys

_DEBUG = os.environ.get("ECKO_DEBUG", "") == "1"


def debug(msg: str) -> None:
    """Write a debug message to stderr if ECKO_DEBUG=1."""
    if _DEBUG:
        sys.stderr.write(f"~~ ecko ~~ debug: {msg}\n")
        sys.stderr.flush()
