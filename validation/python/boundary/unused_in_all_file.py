# Expected: exit 0
# Known limitation: __all__ short-circuits the entire unused-imports check.
# sys IS genuinely unused here, but __all__ prevents detection.
import os
import sys

__all__ = ["os"]
