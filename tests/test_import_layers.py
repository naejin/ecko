"""Tests for import layer enforcement (Step 9)."""

from __future__ import annotations

from pathlib import Path

from checks.config import _parse_yaml_subset, get_import_rules
from checks.custom.import_layers import check_import_layers

FIXTURES = Path(__file__).parent / "fixtures"


class TestImportLayersPython:
    def test_deny_matches(self, tmp_path):
        f = tmp_path / "routes" / "api.py"
        f.parent.mkdir()
        f.write_text("from repositories.user import UserRepo\n", encoding="utf-8")
        rules = [{"files": "routes/*.py", "deny_import": ["repositories"], "message": "No data layer"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert len(echoes) == 1
        assert echoes[0].check == "import-layer"
        assert "repositories" in echoes[0].message

    def test_file_no_match_glob(self, tmp_path):
        f = tmp_path / "utils" / "helper.py"
        f.parent.mkdir()
        f.write_text("from repositories import base\n", encoding="utf-8")
        rules = [{"files": "routes/*.py", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert echoes == []

    def test_dot_prefix_match(self, tmp_path):
        """'from repositories.user import X' should match deny 'repositories' (dot-separated)."""
        f = tmp_path / "routes" / "v2.py"
        f.parent.mkdir()
        f.write_text("from repositories.user import get_user\n", encoding="utf-8")
        rules = [{"files": "routes/*.py", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert len(echoes) == 1

    def test_no_false_prefix(self, tmp_path):
        """'import my_repositories' should NOT match deny 'repositories'."""
        f = tmp_path / "routes" / "v3.py"
        f.parent.mkdir()
        f.write_text("import my_repositories\n", encoding="utf-8")
        rules = [{"files": "routes/*.py", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert echoes == []

    def test_empty_rules(self, tmp_path):
        f = tmp_path / "app.py"
        f.write_text("import os\n", encoding="utf-8")
        echoes = check_import_layers(str(f), [], str(tmp_path))
        assert echoes == []

    def test_fixture_file(self):
        rules = [{"files": "*.py", "deny_import": ["repositories", "sqlalchemy"], "message": "No data layer"}]
        echoes = check_import_layers(str(FIXTURES / "route_bad_import.py"), rules, str(FIXTURES.parent))
        assert len(echoes) == 2
        checks = {e.message for e in echoes}
        assert any("repositories" in m for m in checks)
        assert any("sqlalchemy" in m for m in checks)


class TestImportLayersJS:
    def test_js_slash_prefix(self, tmp_path):
        f = tmp_path / "routes" / "api.ts"
        f.parent.mkdir()
        f.write_text("import { User } from 'repositories/user';\n", encoding="utf-8")
        rules = [{"files": "routes/*.ts", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert len(echoes) == 1

    def test_js_no_false_prefix(self, tmp_path):
        f = tmp_path / "routes" / "api.ts"
        f.parent.mkdir()
        f.write_text("import X from 'my_repositories';\n", encoding="utf-8")
        rules = [{"files": "routes/*.ts", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert echoes == []

    def test_require_match(self, tmp_path):
        f = tmp_path / "routes" / "api.js"
        f.parent.mkdir()
        f.write_text("const db = require('repositories');\n", encoding="utf-8")
        rules = [{"files": "routes/*.js", "deny_import": ["repositories"], "message": "bad"}]
        echoes = check_import_layers(str(f), rules, str(tmp_path))
        assert len(echoes) == 1


class TestImportRulesConfig:
    def test_get_import_rules_default(self):
        assert get_import_rules({}) == []

    def test_get_import_rules_from_config(self):
        config = {"import_rules": [{"files": "*.py", "deny_import": ["foo"]}]}
        rules = get_import_rules(config)
        assert len(rules) == 1

    def test_import_rules_yaml_parsing(self):
        text = """import_rules:
  - files: "routes/*.py"
    deny_import:
      - repositories
      - sqlalchemy
    message: "No data layer in routes"
"""
        config = _parse_yaml_subset(text)
        rules = get_import_rules(config)
        assert len(rules) == 1
        assert rules[0]["files"] == "routes/*.py"
        assert rules[0]["deny_import"] == ["repositories", "sqlalchemy"]
        assert rules[0]["message"] == "No data layer in routes"
