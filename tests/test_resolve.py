"""Tests for tool resolution logic."""

from __future__ import annotations

from unittest.mock import patch

from checks.tools.resolve import resolve_node_tool, resolve_python_tool


class TestResolvePythonTool:
    def test_found_on_path(self):
        with patch("shutil.which", side_effect=lambda x: f"/usr/bin/{x}" if x == "ruff" else None):
            result = resolve_python_tool("ruff")
            assert result == ["ruff"]

    def test_fallback_to_uvx(self):
        def which(name):
            if name == "uvx":
                return "/usr/bin/uvx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_python_tool("ruff")
            assert result == ["uvx", "ruff"]

    def test_fallback_to_pipx(self):
        def which(name):
            if name == "pipx":
                return "/usr/bin/pipx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_python_tool("ruff")
            assert result == ["pipx", "run", "ruff"]

    def test_custom_package_name(self):
        def which(name):
            if name == "uvx":
                return "/usr/bin/uvx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_python_tool("pyright", package="pyright")
            assert result == ["uvx", "pyright"]

    def test_nothing_available(self):
        with patch("shutil.which", return_value=None):
            result = resolve_python_tool("ruff")
            assert result is None

    def test_uvx_preferred_over_pipx(self):
        def which(name):
            if name in ("uvx", "pipx"):
                return f"/usr/bin/{name}"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_python_tool("ruff")
            assert result == ["uvx", "ruff"]


class TestResolveNodeTool:
    def test_found_on_path(self):
        with patch("shutil.which", side_effect=lambda x: f"/usr/bin/{x}" if x == "biome" else None):
            result = resolve_node_tool("biome")
            assert result == ["biome"]

    def test_fallback_to_npx(self):
        def which(name):
            if name == "npx":
                return "/usr/bin/npx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_node_tool("prettier")
            assert result == ["npx", "--yes", "prettier"]

    def test_package_differs_from_binary(self):
        def which(name):
            if name == "npx":
                return "/usr/bin/npx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_node_tool("biome", package="@biomejs/biome")
            assert result == ["npx", "--yes", "--package", "@biomejs/biome", "biome"]

    def test_tsc_from_typescript(self):
        def which(name):
            if name == "npx":
                return "/usr/bin/npx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_node_tool("tsc", package="typescript")
            assert result == ["npx", "--yes", "--package", "typescript", "tsc"]

    def test_fallback_to_pnpx(self):
        def which(name):
            if name == "pnpx":
                return "/usr/bin/pnpx"
            return None

        with patch("shutil.which", side_effect=which):
            result = resolve_node_tool("prettier")
            assert result == ["pnpx", "prettier"]

    def test_nothing_available(self):
        with patch("shutil.which", return_value=None):
            result = resolve_node_tool("biome")
            assert result is None
