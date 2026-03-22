# Expected: exit 1, check=test-conditional
# Filename must start with test_ for test-quality checks to fire.
def test_with_branch():
    x = 1
    if x > 0:
        assert x == 1
    else:
        assert x == 0
