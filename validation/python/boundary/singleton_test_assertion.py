# Expected: exit 1, check=singleton-comparison
# Known FP: == True/False in test assertions is intentional equality testing.
# This IS flagged (known remaining FP pattern).
def test_exact_bool():
    result = get_result()
    assert result == True
    assert result == False
