# Expected: exit 1, check=mutable-default-args
def f(x=[]):
    return x

def g(data={}):
    return data

def h(s=set()):
    return s
