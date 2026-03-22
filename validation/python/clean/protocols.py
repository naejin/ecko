# Expected: exit 0
# Protocol classes with ... body, @overload stubs.
from typing import Protocol, runtime_checkable, overload


@runtime_checkable
class Readable(Protocol):
    def read(self) -> str:
        ...


class Writable(Protocol):
    def write(self, data: str) -> None:
        ...


@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x):
    return x
