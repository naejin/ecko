# Expected: exit 0
# Dunder methods with pass body are exempt from placeholder-code.
class Resource:
    def __init__(self):
        pass

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        pass

    def __repr__(self):
        return "Resource()"
