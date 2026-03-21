"""Tests for the placeholder-code custom check."""

from __future__ import annotations

import textwrap

from checks.custom.placeholder_code import check_placeholder_code, check_placeholder_code_js


class TestPythonPlaceholder:
    def test_pass_only_body(self, tmp_path):
        f = tmp_path / "stub.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                pass
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1
        assert echoes[0].check == "placeholder-code"
        assert "pass" in echoes[0].message

    def test_ellipsis_body(self, tmp_path):
        f = tmp_path / "stub.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                ...
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1
        assert "..." in echoes[0].message

    def test_not_implemented_error(self, tmp_path):
        f = tmp_path / "stub.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                raise NotImplementedError("todo")
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1
        assert "NotImplementedError" in echoes[0].message

    def test_not_implemented_error_bare(self, tmp_path):
        f = tmp_path / "stub.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                raise NotImplementedError
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1
        assert "NotImplementedError" in echoes[0].message

    def test_abstractmethod_skip(self, tmp_path):
        f = tmp_path / "abc_class.py"
        f.write_text(textwrap.dedent("""\
            from abc import abstractmethod

            class Base:
                @abstractmethod
                def do_something(self):
                    pass
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_overload_skip(self, tmp_path):
        f = tmp_path / "overloaded.py"
        f.write_text(textwrap.dedent("""\
            from typing import overload

            @overload
            def process(x: int) -> int: ...
            @overload
            def process(x: str) -> str: ...
            def process(x):
                return x
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_protocol_skip(self, tmp_path):
        f = tmp_path / "proto.py"
        f.write_text(textwrap.dedent("""\
            from typing import Protocol

            class Readable(Protocol):
                def read(self) -> bytes:
                    ...
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_multi_statement_body_clean(self, tmp_path):
        f = tmp_path / "real.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                x = 1
                return x
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_docstring_plus_pass(self, tmp_path):
        f = tmp_path / "doc_pass.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                \"\"\"This function does nothing yet.\"\"\"
                pass
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1
        assert "pass" in echoes[0].message

    def test_docstring_plus_real_body_clean(self, tmp_path):
        f = tmp_path / "doc_real.py"
        f.write_text(textwrap.dedent("""\
            def do_something():
                \"\"\"This function does something.\"\"\"
                return 42
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_nonexistent_file(self):
        echoes = check_placeholder_code("/nonexistent/file.py")
        assert echoes == []

    def test_syntax_error_graceful(self, tmp_path):
        f = tmp_path / "broken.py"
        f.write_text("def foo(\n")
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_class_method_placeholder(self, tmp_path):
        f = tmp_path / "cls.py"
        f.write_text(textwrap.dedent("""\
            class MyClass:
                def method(self):
                    pass
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1

    def test_async_function_placeholder(self, tmp_path):
        f = tmp_path / "async_stub.py"
        f.write_text(textwrap.dedent("""\
            async def fetch_data():
                raise NotImplementedError
        """))
        echoes = check_placeholder_code(str(f))
        assert len(echoes) == 1

    def test_nested_function_not_flagged(self, tmp_path):
        """Nested helper functions with pass body should not be flagged."""
        f = tmp_path / "nested.py"
        f.write_text(textwrap.dedent("""\
            def outer():
                def _inner():
                    pass
                return _inner()
        """))
        echoes = check_placeholder_code(str(f))
        assert echoes == []

    def test_outer_placeholder_flagged_not_nested(self, tmp_path):
        """Only module/class-level functions are checked, not nested."""
        f = tmp_path / "outer_stub.py"
        f.write_text(textwrap.dedent("""\
            def outer():
                pass

            def wrapper():
                def _helper():
                    pass
                return _helper()
        """))
        echoes = check_placeholder_code(str(f))
        # Only 'outer' (module-level) should be flagged, not _helper (nested)
        assert len(echoes) == 1
        assert echoes[0].line == 1


class TestJsPlaceholder:
    def test_throw_not_implemented(self, tmp_path):
        f = tmp_path / "stub.ts"
        f.write_text('function doSomething() {\n  throw new Error("Not implemented");\n}\n')
        echoes = check_placeholder_code_js(str(f))
        assert len(echoes) == 1
        assert echoes[0].check == "placeholder-code"

    def test_throw_todo(self, tmp_path):
        f = tmp_path / "stub.ts"
        f.write_text('function doSomething() {\n  throw new Error("TODO");\n}\n')
        echoes = check_placeholder_code_js(str(f))
        assert len(echoes) == 1

    def test_throw_real_error_clean(self, tmp_path):
        f = tmp_path / "real.ts"
        f.write_text('function doSomething() {\n  throw new Error("Invalid input");\n}\n')
        echoes = check_placeholder_code_js(str(f))
        assert echoes == []

    def test_commented_throw_skipped(self, tmp_path):
        f = tmp_path / "commented.ts"
        f.write_text('// throw new Error("Not implemented");\n')
        echoes = check_placeholder_code_js(str(f))
        assert echoes == []

    def test_block_comment_skipped(self, tmp_path):
        f = tmp_path / "block.ts"
        f.write_text('/*\n  throw new Error("Not implemented");\n*/\n')
        echoes = check_placeholder_code_js(str(f))
        assert echoes == []

    def test_nonexistent_file(self):
        echoes = check_placeholder_code_js("/nonexistent/file.ts")
        assert echoes == []
