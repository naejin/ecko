# Ecko — Claude Code Plugin

## What this is
A Claude Code plugin providing deterministic code quality checks via hooks.
Three layers: silent auto-fix (Layer 1), per-file echoes (Layer 2), deep analysis on stop (Layer 3).

## Structure
- `.claude-plugin/plugin.json` — plugin manifest
- `hooks/hooks.json` — PreToolUse(Bash, ExitPlanMode) + PostToolUse(Write|Edit) + Stop hook wiring
- `hooks/*.sh` — shell entry points that delegate to `checks/runner.py`
- `checks/` — Python package: runner, config, result, formatter, tools/, custom/
- `commands/` — slash commands (ping, status, setup, tune)
- `config/biome.json` — biome lint config (only ecko's rules enabled)
- `scripts/` — install scripts (bash + powershell)
- `tests/` — pytest suite with fixtures/
- `ecko.yaml.example` — full config reference (check names, banned patterns, etc.)
- `CHANGELOG.md` — version history for all releases

## Design constraints
- Zero Python dependencies — config.py has a minimal YAML subset parser (no PyYAML)
- Tools auto-resolve via `checks/tools/resolve.py`: PATH first → `uvx`/`pipx run` (Python) → `npx`/`pnpx` (Node)
- When binary != package (e.g. `tsc` from `typescript`), use `resolve_node_tool("tsc", package="typescript")`
- Hook output goes to stderr (`result.emit()`) — that's how Claude Code reads it
- Exit code 1 = echoes found (agent self-corrects), exit code 0 = clean, exit code 2 = block (PreToolUse)
- Noise filters live in adapters/custom checks, not in runner.py — filter at the source
- Prose files (.md, .txt, .rst, .adoc, .rdoc) are skipped by unicode-artifact (em dashes are normal punctuation)
- Pyright "could not be resolved" imports are filtered (missing deps, not code defects)
- Vulture framework-injected params (`_ALWAYS_SKIP`) filtered everywhere; pytest fixtures (`_PYTEST_SKIP`) filtered only in test/conftest files
- Test file detection is filename-only (`test_*.py`, `*_test.py`, `conftest.py`) — never directory-based (avoids running test checks on `tests/helpers.py`)
- AST checks on test functions use `_iter_test_functions` (module + class level only) — never `ast.walk(tree)` which finds nested `test_*`-prefixed helpers
- `.pyi` type stubs are skipped from all linting (they exist for type checkers, not runtime)

## Cross-platform gotchas
- Always `open()` with `encoding="utf-8"` — Windows Python 3.10/3.12 defaults to cp1252, which silently fails on UTF-8 multi-byte chars (e.g. smart quotes contain byte 0x9d, undefined in cp1252)
- Test assertions on paths must use `os.path.normpath()` — Windows normalizes `/` to `\`
- Line offset math must find `\n` in raw source, not assume `len(line) + 1` — CRLF-safe
- Shell hooks: use `printf '%s' "$VAR"` not `echo "$VAR"` — echo handles escape sequences inconsistently across platforms
- Shell hooks: always include `set -euo pipefail` for consistency, even in trivial scripts

## Code style
- All modules use `from __future__ import annotations`
- Check names are kebab-case: `unused-imports`, `unicode-artifact`, `dead-code`
- Tool adapters follow a pattern: `run_<tool>(args) -> list[Echo]` (per-file) or `-> dict[str, list[Echo]]` (multi-file)
- Custom checks follow: `check_<name>(file_path) -> list[Echo]`
- Graceful skip: resolver returns None → return empty list, never error. Never call `shutil.which()` directly in adapters.

## Adding a new check
- Tool adapter: add `checks/tools/<name>_adapter.py`, wire into `runner.py` per-file or stop mode
- Custom check: add `checks/custom/<name>.py`, wire into `runner.py` under Layer 2
- Register the check name in `ecko.yaml.example` disabled_checks comment
- For AST-based checks on test functions: use `_iter_test_functions()` + `_walk_shallow()` to avoid nested function/class false positives
- Guard clause filters (in `_is_guard_clause`): skip `self.skipTest`, `pytest.skip/fail`, `raise pytest.skip`, early return, platform guards (`os.name`, `sys.version_info`, `sys.platform`)
- Regex patterns in bash guard: avoid `$` anchors (bypassed by trailing args), use `(\s|$|;|&|\|)` terminators instead

## Testing
- Smoke test: `python3 checks/runner.py --file <path> --mode post-tool-use --cwd <dir> --plugin-root .`
- All imports: `python3 -c "from checks.runner import main"`
- Stop mode: `python3 checks/runner.py --file <any> --mode stop --cwd <dir> --plugin-root .`
- Run tests: `python3 -m pytest tests/`
- If pytest not installed: `uvx pytest tests/ -v`
- Use temp files for testing checks (e.g., write a .py with unused imports, run runner, verify output)
- Bash guard: `echo 'COMMAND' | python3 checks/runner.py --mode pre-tool-use-bash --cwd . --plugin-root .` (exit 2 = block, 0 = allow)
- Test fixtures in `tests/fixtures/` must NOT start with `test_` prefix unless they are intentionally bad test files (conftest.py `collect_ignore_glob` excludes them)
- Real-world validation: clone repos to `/tmp/`, run checks via `check_test_quality()` or `run_post_tool_use()` directly, assess TP/FP rates
- Validation results for v0.4.0: `docs/ideas/validation-results.md` (42 repos tested)
- CI matrix: `{ubuntu, macos, windows} × {Python 3.10, 3.12}` — 6 jobs total (`.github/workflows/test.yml`)

## Releasing
- Bump `version` in `.claude-plugin/plugin.json` — marketplace reads version from here, not git tags
- Update version badge in `README.md`
- Add entry to `CHANGELOG.md`
- Push and wait for CI green on all 6 matrix jobs before tagging
- Tag, push tag, `gh release create`
- Verify with: `curl -fsSL https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.sh | bash`

## Not part of the plugin
- `docs/ideas/` — internal ideation (gitignored)
- `openspec/`, `.claude/` — dev workflow tooling, not distributed
