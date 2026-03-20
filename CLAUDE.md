# Ecko — Claude Code Plugin

## What this is
A Claude Code plugin providing deterministic code quality checks via hooks.
Three layers: silent auto-fix (Layer 1), per-file echoes (Layer 2), deep analysis on stop (Layer 3).

## Structure
- `.claude-plugin/plugin.json` — plugin manifest
- `hooks/hooks.json` — PreToolUse(Bash, ExitPlanMode) + PostToolUse(Write|Edit) + Stop hook wiring
- `hooks/*.sh` — shell entry points that delegate to `checks/runner.py`
- `checks/` — Python package: runner, config, result, formatter, regex_utils, fileutil, tools/, custom/
- `commands/` — slash commands (ping, status, setup, tune, reverb)
- `config/biome.json` — biome lint config (only ecko's rules enabled)
- `scripts/` — install scripts (bash + powershell)
- `tests/` — pytest suite with fixtures/
- `ecko.yaml.example` — full config reference (check names, banned patterns, etc.)
- `CHANGELOG.md` — version history for all releases

## Design constraints
- Stop hook output must NEVER contain file-write instructions or paths — only short tips. The stop hook fires PostToolUse hooks on Write/Edit; instructing Claude to write from stop output creates an infinite loop. `/ecko:reverb` exists so file creation is user-initiated only.
- Slash commands (commands/*.md) use `$ARGUMENTS` for user input and `${CLAUDE_PLUGIN_ROOT}` for plugin paths — both set by Claude Code at invocation time
- `checks/regex_utils.py` is a pure utility (imports only `re` + `threading` + `typing`) — no `emit()`, no I/O. All ReDoS-safe regex goes through it. `_run_with_timeout` is the shared thread helper; `_SENTINEL` distinguishes timeout from function-returned-None.
- `checks/fileutil.py` is a pure utility (imports only `os`) — canonical test-file predicate, single source of truth
- `banned_patterns` uses `safe_regex_finditer` over full source (1 thread per pattern, not per line) with `bisect` for line numbers
- Config warnings deduplicated per cwd via `_config_warned` set in runner.py
- Zero Python dependencies — config.py has a minimal YAML subset parser (no PyYAML)
- YAML parser `_parse_list_block` supports nested lists via `last_empty_key` tracking — highest-risk code path, add regression tests for any changes (test all config sections parse correctly after edits)
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
- Config values (`shadow_allowlist`, `echo_cap`, `import_rules`) computed once before file loops in runner.py — never inside per-file loops (same config, no need to recompute)
- Test file detection is filename-only (`test_*.py`, `*_test.py`, `conftest.py`, `conftest.pyi`) via `checks/fileutil.is_test_file()` — never directory-based (avoids running test checks on `tests/helpers.py`)
- AST checks on test functions use `_iter_test_functions` (module + class level only) — never `ast.walk(tree)` which finds nested `test_*`-prefixed helpers
- `.pyi` type stubs are skipped from all linting (they exist for type checkers, not runtime)
- `.test-d.ts` tsd assertion files are skipped via `_is_skippable_stub()` in runner.py — same skip as `.pyi`
- All adapters use `emit()` from `result.py` for stderr output — never `sys.stderr.write` directly
- Pyright is a Python tool: use `resolve_python_tool("pyright")`, NOT `resolve_node_tool` (despite npm availability)
- Layer 2 checks live in `_run_layer2_checks()` — add new checks there, not in `run_post_tool_use()` or `run_stop()` separately
- JS/TS import extraction (`_extract_js_imports`) skips commented-out imports via `_is_in_js_comment()` heuristic
- `safe_regex_compile()` in `checks/regex_utils.py` caches compiled patterns in `_compiled_cache` — each pattern compiled at most once per process. Timeouts are NOT cached (allow retry); only `re.error` failures cache `None`.
- Fixture cache (`_fixture_cache`) stores `(paths, mtime, names)` — compares path lists to detect new conftest.py files

## Noise reduction (v0.5.0)
- `builtin-shadowing` (ruff A001/A002): 20-name default allowlist filters idiomatic API params (`type`, `help`, `input`, `format`, `id`, `repr`, `ascii`, etc.). Configurable via `builtin_shadow_allowlist` in ecko.yaml — user list replaces default entirely
- `var-declarations` / echo avalanches: capped at 5 per check per file (configurable via `echo_cap_per_check`). Overflow summarized as "... and N more"
- `empty-block-statements` (biome noEmptyBlockStatements): renamed from `empty-error-handlers` to reflect actual scope. Python ruff S110 keeps `empty-error-handlers` name (it IS specific to try/except/pass)
- `unreachable-code`: yield-after-raise skipped in generators and `@contextmanager` functions (both custom check and vulture adapter)
- `dead-code` (vulture): dunder-prefix filter (`__n`, `__get__`, etc.), expanded `_ALWAYS_SKIP` (`objtype`, `owner`, `sender`), dynamic pytest fixture collection from conftest.py files

## Trust + safety (v0.5.1)
- Skipped-tool reporting: when a tool is unavailable, ecko emits `~~ ecko ~~ note: <tool> (not found)` instead of silent nothing
- Config validation: `validate_config()` warns on unknown keys (with "did you mean?" suggestions) and invalid regex patterns
- Bash guard expanded: blocks `git push --force`, `git reset --hard`, `git clean -f` in addition to existing patterns
- ReDoS protection: user-supplied regex in `banned_patterns` and `blocked_commands` runs with thread-based timeout (500ms)
- Layer 3 runs tools in parallel via `ThreadPoolExecutor` (2-3x speedup)
- Vulture scoped to modified files only (`run_vulture(cwd, modified_files=...)`)
- tsc/knip results post-filtered to modified files in runner.py
- Vulture fixture collection cached per cwd (`_fixture_cache`)
- Config values `banned` and `obsolete` hoisted before per-file loop in `run_stop()`
- `encoding="utf-8"` added to all `open()` calls in formatter.py, banned_patterns.py, duplicate_keys.py

## Known remaining FP patterns (tracked for future work)
- `builtin-shadowing`: `object`, `print`, `all` intentionally NOT in default allowlist — users can add via config
- `singleton-comparison` in test files: `== True`/`== False` in test assertions is intentional equality testing
- Pyright "unknown import symbol": not yet filtered (only "could not be resolved" is filtered)
- Vulture FastAPI DI params: route handler params injected by framework are flagged as unused

## Cross-platform gotchas
- Always `open()` with `encoding="utf-8"` — Windows Python 3.10/3.12 defaults to cp1252, which silently fails on UTF-8 multi-byte chars (e.g. smart quotes contain byte 0x9d, undefined in cp1252)
- Test assertions on paths must use `os.path.normpath()` — Windows normalizes `/` to `\`
- Line offset math must find `\n` in raw source, not assume `len(line) + 1` — CRLF-safe
- `_strip_trailing_whitespace` must check `\r\n` before `\n` (`.rstrip()` strips `\r` too) — preserves original line endings
- Shell hooks: use `printf '%s' "$VAR"` not `echo "$VAR"` — echo handles escape sequences inconsistently across platforms
- Shell hooks: always include `set -euo pipefail` for consistency, even in trivial scripts
- Integration tests that assert on clean output (`output == ""`) must tolerate tool warnings — on Windows, tools may resolve via npx but fail with WinError 2

## Code style
- All modules use `from __future__ import annotations`
- Check names are kebab-case: `unused-imports`, `unicode-artifact`, `dead-code`
- Tool adapters follow a pattern: `run_<tool>(args) -> list[Echo]` (per-file) or `-> dict[str, list[Echo]]` (multi-file)
- Custom checks follow: `check_<name>(file_path) -> list[Echo]`
- Graceful skip: resolver returns None → return empty list, never error. Never call `shutil.which()` directly in adapters.
- Adapter error handling: catch `TimeoutExpired` and `OSError` separately, call `emit()` with descriptive warning, return empty
- BFS queues: use `collections.deque` with `popleft()`, never `list.pop(0)` (O(n) per pop)

## Adding a new check
- Tool adapter: add `checks/tools/<name>_adapter.py`, wire into `_run_layer2_checks()` in runner.py
- Custom check: add `checks/custom/<name>.py`, wire into `_run_layer2_checks()` in runner.py
- Register the check name in `ecko.yaml.example` disabled_checks comment
- For AST-based checks on test functions: use `_iter_test_functions()` + `_walk_shallow()` to avoid nested function/class false positives
- Guard clause filters (in `_is_guard_clause`): skip `self.skipTest`, `pytest.skip/fail`, `raise pytest.skip`, early return, platform guards (`os.name`, `sys.version_info`, `sys.platform`)
- Regex patterns in bash guard: avoid `$` anchors (bypassed by trailing args), use `(\s|$|;|&|\|)` terminators instead
- Bash guard `--force` pattern: must match both `--force` and `-f`; use command-wide `(?!.*--force-with-lease)` lookahead, not position-specific `(?!-with-lease)`
- ReDoS test inputs: `"a" * 25 + "!"` triggers catastrophic backtracking for `(a+)+b`; `"a" * N + "c"` does NOT (engine fails fast). Always add wall-clock assertion with `time.monotonic()`

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
- Validation results: `docs/ideas/validation-results.md` (52 repos + v0.5.0 release validation)
- 10-repo validation command: `python3 checks/runner.py --file /tmp/ecko-test-{repo}/{file} --mode post-tool-use --cwd /tmp/ecko-test-{repo} --plugin-root /home/daylon/projects/ecko`
- 10-repo validation suite: Flask, FastAPI, httpx, Rich, Click, Pydantic, Express, Preact, Zod, Chalk
- Stop-mode validation: copy source to tmp dir, `git init` + commit all, modify files (append newline), then run `--mode stop`. Must copy WITHOUT `.git` dir (`shutil.copytree` with `ignore_patterns('.git')`) or nested git confuses `_get_modified_files()`
- Use parallel subagents for multi-repo validation (5 agents x 2 repos each works well)
- CI matrix: `{ubuntu, macos, windows} × {Python 3.10, 3.12}` — 6 jobs total (`.github/workflows/test.yml`)

## Releasing
- Bump `version` in `.claude-plugin/plugin.json` — marketplace reads version from here, not git tags
- Update version badge in `README.md`
- Add entry to `CHANGELOG.md`
- Push and wait for CI green on all 6 matrix jobs before tagging
- If CI fails, fix and push again — do NOT tag until all 6 jobs are green (the v0.6.0 release needed a Windows CI fix before tagging)
- Tag, push tag, `gh release create v{X} --title "..." --notes-file /tmp/release-notes.md` (flag is `-F`/`--notes-file`, NOT `--body`)
- Verify with: `curl -fsSL https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.sh | bash`
- Update `commands/` listing in Structure section of CLAUDE.md if adding/removing commands
- Update commands table in README.md if adding/removing commands
- Update `docs/ideas/ideas-done.md` and `ideas-todo.md`
- CHANGELOG test count must match actual `pytest` output (currently 314)

## Transparency (v0.6.0)
- Tool adapter failure reporting: all adapters catch `TimeoutExpired` vs `OSError` separately, emit `~~ ecko ~~ warning: {tool} timed out/failed` to stderr
- Thread pool error reporting in `run_stop()`: failed futures emit tool name + exception
- Skipped-tool messages include install hints (`ruff not found — try: pip install ruff`)
- Echo cap overflow messages explain how to configure the limit
- Layer 2 check dispatch extracted to `_run_layer2_checks()` — single place to add new checks
- `.test-d.ts` files skipped from all linting (tsd type assertion files)
- Bash guard catches full-path (`/bin/rm`), backslash-escaped (`\rm`), `command rm`, and `git -C` prefix bypass variants
- `banned_patterns` `re.compile()` runs inside timeout protection (same as `re.search()`)
- Import-layer echoes report actual line numbers (AST lineno for Python, regex offset for JS/TS)

## Reverb/Tune lifecycle (v0.6.1)
- Stop hook (runner.py): emits `~~ ecko ~~ tip: run /ecko:reverb` when `reverb: enabled` and echoes found — single line only, no file ops
- `/ecko:reverb`: user-initiated, creates `.ecko-reverb/{YYYY-MM-DD}-{slug}.md` with echo summary + reflection
- `/ecko:tune`: reads all `.ecko-reverb/*.md`, deduplicates, presents numbered interactive list, applies user selection to `ecko.yaml`, deletes ALL read reverb notes (even on "none")
- `.ecko-reverb/` is in `_DEFAULT_EXCLUDE_DIRS` — reverb notes are never linted

## Current version and next milestone
- Current: v0.6.1 (tech debt + reverb/tune UX)
- Previous: v0.6.0 (transparency + trust)

## Not part of the plugin
- `docs/ideas/` — internal ideation (gitignored)
- `openspec/`, `.claude/` — dev workflow tooling, not distributed
