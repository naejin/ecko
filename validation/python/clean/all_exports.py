# Expected: exit 0
# __all__ causes unused-imports check to short-circuit entirely.
from os.path import join, exists

__all__ = ["join"]
