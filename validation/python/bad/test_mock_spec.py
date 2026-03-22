# Expected: exit 1, check=mock-spec-bypass
from unittest.mock import Mock

class User:
    name: str

def test_bypass():
    m = Mock(spec=User)
    m.nonexistent = "bad"
