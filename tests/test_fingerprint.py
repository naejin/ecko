"""Tests for project fingerprinting."""

from __future__ import annotations

import json

from checks.fingerprint import detect_frameworks


class TestDetectFrameworks:
    def test_django_requirements_txt(self, tmp_path):
        (tmp_path / "requirements.txt").write_text("django>=4.0\nrequests\n")
        result = detect_frameworks(str(tmp_path))
        assert "django" in result

    def test_flask_pyproject_toml(self, tmp_path):
        content = '[project]\ndependencies = ["flask>=2.0"]\n'
        (tmp_path / "pyproject.toml").write_text(content)
        result = detect_frameworks(str(tmp_path))
        assert "flask" in result

    def test_fastapi_requirements_txt(self, tmp_path):
        (tmp_path / "requirements.txt").write_text("fastapi\nuvicorn\n")
        result = detect_frameworks(str(tmp_path))
        assert "fastapi" in result

    def test_nextjs_package_json(self, tmp_path):
        pkg = {"dependencies": {"next": "^13.0.0", "react": "^18.0.0"}}
        (tmp_path / "package.json").write_text(json.dumps(pkg))
        result = detect_frameworks(str(tmp_path))
        assert "nextjs" in result
        assert "react" in result

    def test_express_package_json(self, tmp_path):
        pkg = {"dependencies": {"express": "^4.0.0"}}
        (tmp_path / "package.json").write_text(json.dumps(pkg))
        result = detect_frameworks(str(tmp_path))
        assert "express" in result

    def test_no_markers(self, tmp_path):
        result = detect_frameworks(str(tmp_path))
        assert result == set()

    def test_missing_files(self, tmp_path):
        # Non-existent dir content — should not crash
        result = detect_frameworks(str(tmp_path / "nonexistent"))
        assert result == set()

    def test_empty_requirements(self, tmp_path):
        (tmp_path / "requirements.txt").write_text("")
        result = detect_frameworks(str(tmp_path))
        assert result == set()

    def test_dev_dependencies(self, tmp_path):
        pkg = {"devDependencies": {"vue": "^3.0.0"}}
        (tmp_path / "package.json").write_text(json.dumps(pkg))
        result = detect_frameworks(str(tmp_path))
        assert "vue" in result

    def test_max_file_size_respected(self, tmp_path):
        """Large files should still be read (up to limit) without crashing."""
        # Write a file larger than _MAX_FILE_SIZE
        content = "django\n" * 5000  # ~35KB
        (tmp_path / "requirements.txt").write_text(content)
        result = detect_frameworks(str(tmp_path))
        # Django should still be found in the first 10KB
        assert "django" in result

    def test_multiple_frameworks(self, tmp_path):
        (tmp_path / "requirements.txt").write_text("django\nflask\n")
        result = detect_frameworks(str(tmp_path))
        assert "django" in result
        assert "flask" in result
