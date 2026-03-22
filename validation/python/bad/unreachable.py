# Expected: exit 1, check=unreachable-code
def f():
    return 1
    print("dead")

def g():
    raise ValueError
    x = 1
