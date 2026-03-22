# Expected: exit 1, check=bare-except
try:
    x = 1
except:
    pass

class Foo:
    def method(self):
        try:
            return 1
        except:
            return 0
