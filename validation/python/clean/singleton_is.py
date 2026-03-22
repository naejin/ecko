# Expected: exit 0
# Correct singleton comparison with `is` instead of `==`.
x = None
if x is None:
    pass
if x is not True:
    pass
if x is False:
    pass
