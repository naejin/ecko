# Expected: exit 0
# except Exception (not bare except) with handling.
def safe_load(path):
    try:
        with open(path) as f:
            return f.read()
    except Exception as e:
        print(f"Failed: {e}")
        return None
