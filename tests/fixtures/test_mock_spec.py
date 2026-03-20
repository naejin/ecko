"""Fixture: mock spec bypass (should trigger mock-spec-bypass)."""
from unittest.mock import Mock, MagicMock


class User:
    name: str
    email: str


def test_bypass_spec():
    mock_user = Mock(spec=User)
    mock_user.nonexistent_attr = "bad"  # bypasses spec


def test_allowed_attrs():
    mock_user = Mock(spec=User)
    mock_user.return_value = "ok"  # allowed
    mock_user.side_effect = ValueError  # allowed


def test_no_spec():
    mock_user = Mock()
    mock_user.anything = "fine"  # no spec, no problem


def test_magicmock_spec():
    mock_user = MagicMock(spec=User)
    mock_user.fake_method = lambda: None  # bypasses spec
