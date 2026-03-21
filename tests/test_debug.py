"""Tests for the debug module."""

from __future__ import annotations

import importlib
import os
import sys
from io import StringIO
from unittest.mock import patch

import pytest

import checks.debug as debug_mod
from checks.debug import debug


@pytest.fixture()
def _force_debug(request):
    """Temporarily set checks.debug._DEBUG to the given value, restore after test."""
    value = request.param
    original = debug_mod._DEBUG
    debug_mod._DEBUG = value
    yield
    debug_mod._DEBUG = original


class TestDebugMode:
    @pytest.mark.parametrize("_force_debug", [False], indirect=True)
    def test_debug_disabled_by_default(self, _force_debug):
        """Debug messages should not be emitted when _DEBUG is False."""
        buf = StringIO()
        with patch.object(sys, "stderr", buf):
            debug("should not appear")
        assert buf.getvalue() == ""

    @pytest.mark.parametrize("_force_debug", [True], indirect=True)
    def test_debug_enabled(self, _force_debug):
        """Debug messages should be emitted when _DEBUG is True."""
        buf = StringIO()
        with patch.object(sys, "stderr", buf):
            debug("test message")
        output = buf.getvalue()
        assert output == "~~ ecko ~~ debug: test message\n"
        assert output.startswith("~~ ecko ~~ debug: ")

    def test_debug_respects_env_var(self):
        """ECKO_DEBUG=1 should enable debug mode at import time."""
        original = debug_mod._DEBUG

        with patch.dict(os.environ, {"ECKO_DEBUG": "1"}):
            importlib.reload(debug_mod)
            assert debug_mod._DEBUG is True

        with patch.dict(os.environ, {"ECKO_DEBUG": "0"}):
            importlib.reload(debug_mod)
            assert debug_mod._DEBUG is False

        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("ECKO_DEBUG", None)
            importlib.reload(debug_mod)
            assert debug_mod._DEBUG is False

        debug_mod._DEBUG = original
