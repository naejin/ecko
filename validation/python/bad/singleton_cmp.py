# Expected: exit 1, check=singleton-comparison
x = None
if x == None:
    pass
if x != True:
    pass
if x == False:
    pass
