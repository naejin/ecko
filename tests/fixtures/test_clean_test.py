"""Fixture: clean test file — should trigger no test-quality echoes."""


def test_addition():
    assert 1 + 1 == 2


def test_string():
    assert "hello".upper() == "HELLO"
