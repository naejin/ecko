# Expected: exit 0
# field(default_factory=list) is NOT a mutable default.
from dataclasses import dataclass, field


@dataclass
class Config:
    items: list = field(default_factory=list)
    data: dict = field(default_factory=dict)
