# Expected: exit 1, check=duplicate-keys
data = {
    "name": "Alice",
    "age": 30,
    "name": "Bob",
}
