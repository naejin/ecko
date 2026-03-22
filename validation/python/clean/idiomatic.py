# Expected: exit 0
# Realistic module with all imports used, proper error handling.
import os
import json
from pathlib import Path


class ConfigLoader:
    def __init__(self, path: str):
        self.path = Path(path)

    def load(self):
        if not self.path.exists():
            return None
        try:
            with open(self.path, encoding="utf-8") as f:
                return json.load(f)
        except Exception as e:
            print(f"Error: {e}")
            return None


def get_home():
    return os.path.expanduser("~")


if __name__ == "__main__":
    loader = ConfigLoader("config.json")
    data = loader.load()
    home = get_home()
    print(data, home)
