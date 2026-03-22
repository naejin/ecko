# Expected: exit 1, check=builtin-shadowing
# Note: type/id are in default allowlist, use non-allowlisted builtins
len = 5
int = "string"

def process(len, int):
    return len, int
