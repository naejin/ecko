def greet():
    return "hello"
    print("never runs")


def process():
    for item in [1, 2, 3]:
        break
        print("unreachable")
