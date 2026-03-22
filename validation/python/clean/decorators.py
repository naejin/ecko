# Expected: exit 0
# Decorators, @property, @abstractmethod stubs.
from abc import ABC, abstractmethod
from functools import wraps


def my_decorator(func):
    @wraps(func)
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper


class Base(ABC):
    @property
    def name(self):
        return "base"

    @staticmethod
    def create():
        return None

    @classmethod
    def from_dict(cls, data):
        return cls()

    @abstractmethod
    def process(self):
        ...
