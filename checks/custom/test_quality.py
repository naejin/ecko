"""Test quality checks — AST-based sub-checks for Python test files.

Sub-checks:
  test-conditional  — if/else in test_* functions (tests should not branch)
  fixed-wait        — time.sleep / asyncio.sleep / wait_for_timeout (use polling)
  mock-spec-bypass  — mock.attr = val bypassing Mock(spec=...) validation
"""

from __future__ import annotations

import ast
import os
from collections import deque

from checks.result import Echo


def check_test_quality(file_path: str) -> list[Echo]:
    """Run all test quality sub-checks on a Python test file."""
    if not os.path.isfile(file_path):
        return []
    try:
        with open(file_path, encoding="utf-8") as f:
            source = f.read()
    except (OSError, UnicodeDecodeError):
        return []
    try:
        tree = ast.parse(source, filename=file_path)
    except SyntaxError:
        return []

    echoes: list[Echo] = []
    echoes.extend(_check_test_conditional(tree))
    echoes.extend(_check_fixed_wait(tree))
    echoes.extend(_check_mock_spec_bypass(tree))
    return echoes


# ---------------------------------------------------------------------------
# test-conditional
# ---------------------------------------------------------------------------

# Constant-expression guards to skip (version checks, TYPE_CHECKING, etc.)
_CONSTANT_GUARD_NAMES = frozenset({
    "TYPE_CHECKING",
})

# Loop node types — if statements inside these with no assertions are data filters
_LOOP_TYPES = (ast.For, ast.AsyncFor, ast.While)


def _is_guard_clause(node: ast.If) -> bool:
    """Check if an if-statement is a guard clause we should skip.

    Skips:
      - Constant-expression guards (TYPE_CHECKING, version_info, platform, os.name)
      - Guard-then-skip (self.skipTest, pytest.skip, raise pytest.skip)
      - Guard-then-fail (pytest.fail)
      - Early return guards (if not X: return)
      - if __name__ == "__main__"
    """
    test = node.test
    # if __name__ == "__main__"
    if isinstance(test, ast.Compare):
        if isinstance(test.left, ast.Name) and test.left.id == "__name__":
            return True
        # sys.version_info >= (3, 10) and similar
        if isinstance(test.left, ast.Attribute):
            attr_name = test.left.attr
            if attr_name in ("version_info", "platform"):
                return True
        # sys.version_info[:2] >= (3, 10) — subscript form
        if isinstance(test.left, ast.Subscript):
            subscript_val = test.left.value
            if isinstance(subscript_val, ast.Attribute) and subscript_val.attr in (
                "version_info",
                "platform",
            ):
                return True
        if isinstance(test.left, ast.Name) and test.left.id in _CONSTANT_GUARD_NAMES:
            return True
        # os.name == "nt" / os.name != "nt"
        if (
            isinstance(test.left, ast.Attribute)
            and test.left.attr == "name"
            and isinstance(test.left.value, ast.Name)
            and test.left.value.id == "os"
        ):
            return True
    # if TYPE_CHECKING:
    if isinstance(test, ast.Name) and test.id in _CONSTANT_GUARD_NAMES:
        return True
    # if sys.version_info >= ... / if sys.platform == ...
    if isinstance(test, ast.Attribute):
        if test.attr in ("version_info", "platform"):
            return True

    # Guard-then-skip/fail/return: single-statement body, NO else branch.
    # An if/else where one branch skips is still a conditional — only
    # single-branch guards (no orelse) are suppressed.
    if len(node.body) == 1 and not node.orelse:
        stmt = node.body[0]
        # Early return guard
        if isinstance(stmt, ast.Return):
            return True
        # raise pytest.skip(...) — idiomatic skip via raise
        if isinstance(stmt, ast.Raise) and stmt.exc is not None:
            if _is_pytest_call(stmt.exc, "skip"):
                return True
        # self.skipTest(...) / pytest.skip(...) / pytest.fail(...)
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Call):
            call = stmt.value
            # self.skipTest(...)
            if (
                isinstance(call.func, ast.Attribute)
                and call.func.attr == "skipTest"
            ):
                return True
            # pytest.skip(...) / pytest.fail(...)
            if _is_pytest_call(call, "skip") or _is_pytest_call(call, "fail"):
                return True

    return False


def _is_pytest_call(node: ast.expr, method: str) -> bool:
    """Check if an expression is pytest.<method>(...)."""
    if not isinstance(node, ast.Call):
        return False
    return (
        isinstance(node.func, ast.Attribute)
        and node.func.attr == method
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "pytest"
    )


def _walk_shallow(node: ast.AST) -> list[ast.AST]:
    """Walk AST children without descending into nested function/class defs."""
    result: list[ast.AST] = []
    queue = deque(ast.iter_child_nodes(node))
    while queue:
        child = queue.popleft()
        result.append(child)
        # Don't descend into nested functions or classes
        if not isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            queue.extend(ast.iter_child_nodes(child))
    return result


def _iter_test_functions(tree: ast.Module):
    """Yield test_* functions at module level and inside classes only.

    Avoids ast.walk which would also find nested test_*-prefixed helpers
    defined inside other functions, causing duplicate or erroneous checks.
    """
    for node in ast.iter_child_nodes(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("test_"):
                yield node
        elif isinstance(node, ast.ClassDef):
            for child in ast.iter_child_nodes(node):
                if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    if child.name.startswith("test_"):
                        yield child


def _if_body_has_assert(node: ast.If) -> bool:
    """Check if an if-statement body (or its else) contains any assert.

    Uses shallow walk to avoid false positives from asserts inside
    nested function definitions within the if body.
    """
    queue = deque(ast.iter_child_nodes(node))
    while queue:
        child = queue.popleft()
        if isinstance(child, ast.Assert):
            return True
        if not isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            queue.extend(ast.iter_child_nodes(child))
    return False


def _check_test_conditional(tree: ast.Module) -> list[Echo]:
    """Flag if/else inside test_* functions.

    Skips guard clauses (version checks, pytest.skip, early return, etc.)
    and data-filtering ifs inside loops (if body contains no assertions).
    """
    echoes: list[Echo] = []
    for node in _iter_test_functions(tree):
        # BFS walk tracking loop depth, skip nested functions/classes
        queue: deque[tuple[ast.AST, bool]] = deque(
            (child, False) for child in ast.iter_child_nodes(node)
        )
        while queue:
            child, in_loop = queue.popleft()

            # Don't descend into nested functions or classes
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                continue

            if isinstance(child, ast.If) and not _is_guard_clause(child):
                # Skip data-filtering ifs inside loops (no assertions in body)
                if in_loop and not _if_body_has_assert(child):
                    # Still walk into the if body for deeper ifs
                    for sub in ast.iter_child_nodes(child):
                        queue.append((sub, in_loop))
                    continue
                echoes.append(Echo(
                    check="test-conditional",
                    line=child.lineno,
                    message="Conditional (if/else) in test function \u2014 tests should control state, not branch on it.",
                    suggestion="Parametrize or split into separate test cases.",
                ))

            # Children of loops are "in_loop"
            child_in_loop = in_loop or isinstance(child, _LOOP_TYPES)
            for sub in ast.iter_child_nodes(child):
                queue.append((sub, child_in_loop))

    return echoes


# ---------------------------------------------------------------------------
# fixed-wait
# ---------------------------------------------------------------------------

_SLEEP_NAMES = frozenset({"sleep"})
_WAIT_METHODS = frozenset({"wait_for_timeout"})


def _is_zero_arg(call: ast.Call) -> bool:
    """Check if a call has a single numeric argument of 0."""
    if len(call.args) == 1 and not call.keywords:
        arg = call.args[0]
        if isinstance(arg, ast.Constant) and arg.value == 0:
            return True
    return False


def _check_fixed_wait(tree: ast.Module) -> list[Echo]:
    """Flag time.sleep / asyncio.sleep / wait_for_timeout in test functions."""
    echoes: list[Echo] = []
    for node in _iter_test_functions(tree):
        for child in _walk_shallow(node):
            if not isinstance(child, ast.Call):
                continue
            func = child.func
            # time.sleep(...) / asyncio.sleep(...)
            if (
                isinstance(func, ast.Attribute)
                and func.attr in _SLEEP_NAMES
                and isinstance(func.value, ast.Name)
                and func.value.id in ("time", "asyncio")
            ):
                # sleep(0) is an idiomatic yield (event-loop or GIL), not a wait
                if _is_zero_arg(child):
                    continue
                echoes.append(Echo(
                    check="fixed-wait",
                    line=child.lineno,
                    message=f"{func.value.id}.sleep() in test — fixed waits are flaky.",
                    suggestion="Use polling, retry loops, or event-based assertions instead.",
                ))
            # *.wait_for_timeout(...)
            elif isinstance(func, ast.Attribute) and func.attr in _WAIT_METHODS:
                echoes.append(Echo(
                    check="fixed-wait",
                    line=child.lineno,
                    message="wait_for_timeout() in test — fixed waits are flaky.",
                    suggestion="Use polling, retry loops, or event-based assertions instead.",
                ))
    return echoes


# ---------------------------------------------------------------------------
# mock-spec-bypass
# ---------------------------------------------------------------------------

_MOCK_CLASSES = frozenset({"Mock", "MagicMock"})
_ALLOWED_MOCK_ATTRS = frozenset({
    "return_value", "side_effect", "assert_called",
    "assert_called_once", "assert_called_with", "assert_called_once_with",
    "assert_any_call", "assert_has_calls", "assert_not_called",
    "call_args", "call_args_list", "call_count", "called",
    "mock_calls", "reset_mock", "configure_mock",
})


def _check_mock_spec_bypass(tree: ast.Module) -> list[Echo]:
    """Flag attribute assignment on Mock(spec=...) objects (best-effort heuristic)."""
    echoes: list[Echo] = []
    for node in _iter_test_functions(tree):

        # Track names assigned from Mock(spec=...) / MagicMock(spec=...)
        spec_mocks: set[str] = set()
        shallow = _walk_shallow(node)
        for child in shallow:
            if isinstance(child, ast.Assign) and len(child.targets) == 1:
                target = child.targets[0]
                if isinstance(target, ast.Name) and _is_mock_with_spec(child.value):
                    spec_mocks.add(target.id)

        if not spec_mocks:
            continue

        # Find attribute assignments on those mocks
        for child in shallow:
            if not isinstance(child, ast.Assign):
                continue
            for target in child.targets:
                if (
                    isinstance(target, ast.Attribute)
                    and isinstance(target.value, ast.Name)
                    and target.value.id in spec_mocks
                    and target.attr not in _ALLOWED_MOCK_ATTRS
                ):
                    echoes.append(Echo(
                        check="mock-spec-bypass",
                        line=child.lineno,
                        message=f"Setting .{target.attr} on a Mock(spec=...) bypasses spec validation.",
                        suggestion="Use configure_mock() or check if the attribute exists on the spec class.",
                    ))
    return echoes


def _is_mock_with_spec(node: ast.expr) -> bool:
    """Check if a call expression is Mock(spec=...) or MagicMock(spec=...)."""
    if not isinstance(node, ast.Call):
        return False
    func = node.func
    # Direct: Mock(spec=Foo) or MagicMock(spec=Foo)
    if isinstance(func, ast.Name) and func.id in _MOCK_CLASSES:
        return _has_spec_kwarg(node)
    # Qualified: unittest.mock.Mock(spec=Foo) or mock.Mock(spec=Foo)
    if isinstance(func, ast.Attribute) and func.attr in _MOCK_CLASSES:
        return _has_spec_kwarg(node)
    return False


def _has_spec_kwarg(call: ast.Call) -> bool:
    """Check if a Call node has a spec= or spec_set= keyword argument."""
    for kw in call.keywords:
        if kw.arg in ("spec", "spec_set"):
            return True
    return False
