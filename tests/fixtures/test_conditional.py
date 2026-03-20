"""Fixture: test with if/else (should trigger test-conditional)."""


def test_with_conditional():
    result = some_function()
    if result.startswith("win"):
        assert result == "windows"
    else:
        assert result == "unix"


def test_clean():
    """This test has no conditionals — should not trigger."""
    assert 1 + 1 == 2
