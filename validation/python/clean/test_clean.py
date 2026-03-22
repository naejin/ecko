# Expected: exit 0
# Clean test file: guard clauses, loop-filtering, parametrize.
import sys
import pytest


@pytest.mark.parametrize("x", [1, 2, 3])
def test_values(x):
    assert x > 0


def test_platform_guard():
    if sys.platform == "win32":
        pytest.skip("Windows only")
    assert True


def test_filter_loop():
    items = [1, 2, 3, 4, 5]
    evens = []
    for item in items:
        if item % 2 == 0:
            evens.append(item)
    assert evens == [2, 4]
