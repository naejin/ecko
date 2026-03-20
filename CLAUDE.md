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
- Vulture framework-injected params (`_ALWAYS_SKIP`) filtered everywhere; pytest fixtures (`_PYTEST_SKIP` + dynamic conftest scan) filtered only in test/conftest files
- Vulture yield-after-raise filtered in both custom check and vulture adapter (generator protocol pattern)
- Builtin-shadowing filtered by configurable allowlist in ruff adapter (20-name default)
- Echo cap per check per file applied in result formatters, not in runner — cap is display-only, doesn't affect detection
- Test file detection is filename-only (`test_*.py`, `*_test.py`, `conftest.py`) — never directory-based (avoids running test checks on `tests/helpers.py`)
- AST checks on test functions use `_iter_test_functions` (module + class level only) — never `ast.walk(tree)` which finds nested `test_*`-prefixed helpers
- `.pyi` type stubs are skipped from all linting (they exist for type checkers, not runtime)

## Noise reduction (v0.5.0)
- `builtin-shadowing` (ruff A001/A002): 20-name default allowlist filters idiomatic API params (`type`, `help`, `input`, `format`, `id`, `repr`, `ascii`, etc.). Configurable via `builtin_shadow_allowlist` in ecko.yaml — user list replaces default entirely
- `var-declarations` / echo avalanches: capped at 5 per check per file (configurable via `echo_cap_per_check`). Overflow summarized as "... and N more"
- `empty-block-statements` (biome noEmptyBlockStatements): renamed from `empty-error-handlers` to reflect actual scope. Python ruff S110 keeps `empty-error-handlers` name (it IS specific to try/except/pass)
- `unreachable-code`: yield-after-raise skipped in generators and `@contextmanager` functions (both custom check and vulture adapter)
- `dead-code` (vulture): dunder-prefix filter (`__n`, `__get__`, etc.), expanded `_ALWAYS_SKIP` (`objtype`, `owner`, `sender`), dynamic pytest fixture collection from conftest.py files

## Known remaining FP patterns (tracked for future work)
- `builtin-shadowing`: `object`, `print`, `all` intentionally NOT in default allowlist — users can add via config
- `singleton-comparison` in test files: `== True`/`== False` in test assertions is intentional equality testing
- Pyright "unknown import symbol": not yet filtered (only "could not be resolved" is filtered)
- Vulture FastAPI DI params: route handler params injected by framework are flagged as unused
- Chalk `.test-d.ts` files: tsd type assertion files have intentionally "unused" imports

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
- Validation results: `docs/ideas/validation-results.md` (52 repos tested — 42 from v0.4.0, 10 from v0.5.0 pre-validation)
- 10-repo validation command: `python3 checks/runner.py --file /tmp/ecko-test-{repo}/{file} --mode post-tool-use --cwd /tmp/ecko-test-{repo} --plugin-root /home/daylon/projects/ecko`
- 10-repo validation suite: Flask, FastAPI, httpx, Rich, Click, Pydantic, Express, Preact, Zod, Chalk
- Use parallel subagents for multi-repo validation (5 agents x 2 repos each works well)
- CI matrix: `{ubuntu, macos, windows} × {Python 3.10, 3.12}` — 6 jobs total (`.github/workflows/test.yml`)

## Releasing
- Bump `version` in `.claude-plugin/plugin.json` — marketplace reads version from here, not git tags
- Update version badge in `README.md`
- Add entry to `CHANGELOG.md`
- Push and wait for CI green on all 6 matrix jobs before tagging
- Tag, push tag, `gh release create`
- Verify with: `curl -fsSL https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.sh | bash`

## Current version and next milestone
- Current: v0.5.0 (noise reduction + architecture enforcement)
- Previous: v0.4.0 (codeleash-inspired guardrails)

## Not part of the plugin
- `docs/ideas/` — internal ideation (gitignored)
- `openspec/`, `.claude/` — dev workflow tooling, not distributed
