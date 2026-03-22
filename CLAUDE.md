# Ecko — Claude Code Plugin

## What this is
A Claude Code plugin providing deterministic code quality checks via hooks.
Three layers: silent auto-fix (Layer 1), per-file echoes (Layer 2), deep analysis on stop (Layer 3).

## Structure
- `.claude-plugin/plugin.json` — plugin manifest (includes inline `mcpServers` for MCP server entry point)
- `hooks/hooks.json` — PreToolUse(Bash, ExitPlanMode) + PostToolUse(Write|Edit) + Stop hook wiring
- `hooks/*.sh` — shell entry points that find and invoke the Rust binary (binary resolution shared via `hooks/_find_ecko.sh`)
- `scripts/run.sh` / `scripts/run.cmd` — 3-tier binary launcher (pre-built -> cargo build -> GitHub Release download)
- `scripts/install.sh` / `scripts/install.ps1` — marketplace installer + binary acquisition
- `src/` — Rust source (see "Rust structure" section below)
- `queries/` — tree-sitter `.scm` query files (embedded at compile time)
- `commands/` — slash commands (ping, status, setup, tune, reverb, session, guard)
- `checks/` — legacy Python package (v1, hooks no longer use this)
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
- YAML subset parser only supports one level of nesting — new config keys must be flat (e.g., `ruff_extra_rules`, not `ruff: extra_rules:`)
- Tools auto-resolve via `checks/tools/resolve.py`: PATH first → `uvx`/`pipx run` (Python) → `npx`/`pnpx` (Node)
- When binary != package (e.g. `tsc` from `typescript`), use `resolve_node_tool("tsc", package="typescript")`
- `resolve_binary_tool()` in resolve.py for system binaries (Go, Rust) — PATH only, no package manager fallback
- `checks/debug.py` is a pure utility (imports only `os` + `sys`) — module-level `_DEBUG` flag from `ECKO_DEBUG` env var, single `debug()` function
- `checks/debug.py` uses `sys.stderr.write` directly (not `emit()`) — importing result.py would add a dependency to what must be importable from anywhere
- Hook output goes to stderr (`result.emit()`) — that's how Claude Code reads it
- Exit code 1 = echoes found (agent self-corrects), exit code 0 = clean, exit code 2 = block (PreToolUse)
- Noise filters live in adapters/custom checks, not in runner.py — filter at the source
- Prose files (.md, .txt, .rst, .adoc, .rdoc) are skipped by unicode-artifact (em dashes are normal punctuation)
- Pyright "could not be resolved" imports are filtered (missing deps, not code defects)
- Vulture framework-injected params (`_ALWAYS_SKIP`) filtered everywhere; pytest fixtures (`_PYTEST_SKIP` + dynamic conftest scan) filtered only in test/conftest files
- Vulture yield-after-raise filtered in both custom check and vulture adapter (generator protocol pattern)
- Builtin-shadowing filtered by configurable allowlist in ruff adapter (20-name default)
- Compact echo format: 1 line per file (`~~ ecko ~~ file — check (L1, L2 +N)`). `_COMPACT_LINES_PER_CHECK = 3` line numbers shown per check before overflow.
- `format_file_echoes()` and `format_stop_echoes()` no longer accept `echo_cap` parameter — compact format handles overflow via +N notation
- Echo `severity` field: `"warn"` (default) or `"error"`. Only `[error]` prefix shown in text output; warn is implicit. Set in adapters, not runner.
- `_ERROR_CODES` in ruff_adapter.py: frozenset of ruff codes that get `severity="error"` (E722, F403)
- `output_format: json` — schema v1 JSON output to stderr. No echo caps applied (machine consumers need complete data). Exit codes unchanged.
- Stop-mode ledger scoping: when session ledger has post-tool-use entries, stop mode intersects `get_modified_files()` with ledger-tracked files — prevents flooding with pre-existing issues on first use of existing projects
- `session_entries` read once in `run_stop()` — used for both ledger scoping AND correction/stats. Never call `read_session()` twice.
- Config values (`shadow_allowlist`, `echo_cap`, `import_rules`) computed once before file loops in runner.py — never inside per-file loops (same config, no need to recompute)
- Test file detection is filename-only (`test_*.py`, `*_test.py`, `conftest.py`, `conftest.pyi`) via `checks/fileutil.is_test_file()` — never directory-based (avoids running test checks on `tests/helpers.py`)
- AST checks on test functions use `_iter_test_functions` (module + class level only) — never `ast.walk(tree)` which finds nested `test_*`-prefixed helpers
- `.pyi` type stubs are skipped from all linting (they exist for type checkers, not runtime)
- `.test-d.ts` tsd assertion files are skipped via `_is_skippable_stub()` in runner.py — same skip as `.pyi`
- All adapters use `emit()` from `result.py` for stderr output — never `sys.stderr.write` directly
- All output formatting functions live in `result.py` — data/persistence modules (`ledger.py`, `fileutil.py`, `regex_utils.py`) must never format output for stderr
- Pyright is a Python tool: use `resolve_python_tool("pyright")`, NOT `resolve_node_tool` (despite npm availability)
- Layer 2 checks live in `_run_layer2_checks()` — add new checks there, not in `run_post_tool_use()` or `run_stop()` separately
- JS/TS import extraction (`_extract_js_imports`) skips commented-out imports via `_is_in_js_comment()` heuristic
- JS/TS unused-imports detects both ESM `import` and CJS `const x = require()` patterns via `collect_require_imports()` and `is_require_call()` in javascript.rs
- `safe_regex_compile()` in `checks/regex_utils.py` caches compiled patterns in `_compiled_cache` — each pattern compiled at most once per process. Timeouts are NOT cached (allow retry); only `re.error` failures cache `None`.
- Fixture cache (`_fixture_cache`) stores `(paths, mtime, names)` — compares path lists to detect new conftest.py files
- `checks/fingerprint.py` is a pure utility (imports only `os`, `json`) — `detect_frameworks(cwd)` returns set of framework identifiers, no caching yet
- `_FRAMEWORK_VULTURE_SKIPS` in vulture_adapter.py: per-framework skip sets (FastAPI, Flask, Django) applied when fingerprint detects the framework
- Dunder methods (`__enter__`, `__exit__`, etc.) skipped by placeholder-code check — protocol stubs, not placeholders
- Dunder-prefixed params (`__doc__`, `__name__`) skipped by builtin-shadowing filter — intentional API design, not accidental shadowing
- `ruff_use_project_config` always passes `--no-fix` (safety invariant) and drops `--select`. Emits warning (once) if `ruff_extra_rules` is also set.
- `biome_use_project_config` uses `_to_kebab()` for unknown rule name mapping (only when project config active). `_find_project_biome_config()` walks up from file dir.
- `checks/session_stats.py`: standalone script for `/ecko:session` command (prints to stdout, not stderr)

## Noise reduction (v0.5.0)
- `builtin-shadowing`: 20-name default allowlist (`type`, `help`, `input`, `id`, etc.). Configurable via `builtin_shadow_allowlist` -- user list replaces default entirely
- Echo cap: 5 per check per file (configurable via `echo_cap_per_check`). Overflow summarized as "... and N more"
- FP-free patterns verified via `validation/` directory -- run `./validation/run.sh` to check all edge cases

## Trust + safety (v0.5.1)
- Skipped-tool reporting: `~~ ecko ~~ note: <tool> (not found)` instead of silent nothing
- Config validation: `validate_config()` warns on unknown keys (with "did you mean?" suggestions)
- Bash guard: blocks `git push --force`, `git reset --hard`, `git clean -f` (including `git -C` prefix variants)
- ReDoS protection: user-supplied regex runs with thread-based timeout (500ms)
- `encoding="utf-8"` on all `open()` calls (Windows cp1252 safety)

## Known remaining FP patterns
- Tracked as `boundary/` files in `validation/` repos -- see `validation/python/boundary/singleton_test_assertion.py`, etc.
- When a new FP pattern is discovered, add it to the appropriate `validation/{lang}/boundary/` file BEFORE fixing

## Cross-platform gotchas
- Always `open()` with `encoding="utf-8"` — Windows Python 3.10/3.12 defaults to cp1252, which silently fails on UTF-8 multi-byte chars (e.g. smart quotes contain byte 0x9d, undefined in cp1252)
- Test assertions on paths must use `os.path.normpath()` — Windows normalizes `/` to `\`
- Line offset math must find `\n` in raw source, not assume `len(line) + 1` — CRLF-safe
- `_strip_trailing_whitespace` must check `\r\n` before `\n` (`.rstrip()` strips `\r` too) — preserves original line endings
- Shell hooks: use `printf '%s' "$VAR"` not `echo "$VAR"` — echo handles escape sequences inconsistently across platforms
- Shell hooks: always include `set -euo pipefail` for consistency, even in trivial scripts
- Integration tests that assert on clean output (`output == ""`) must tolerate tool warnings — on Windows, tools may resolve via npx but fail with WinError 2
- Adapter post-filter tests: use `os.path.normpath()` on both cwd and modified_files paths — Windows normalizes `/tmp/project` to `\tmp\project`
- `_get_modified_files` includes recently committed files — tests asserting `output == ""` after commit must account for clean-sweep message
- `.gitignore` must include `target/` -- Rust build artifacts are large and must never be committed
- Tests using Unix paths (`/tmp/...`) must use `#[cfg(not(windows))]` guard with Windows equivalent (`C:\\temp\\...`) -- Windows `Path::is_absolute()` requires drive letter
- MCP stdio smoke tests: `{ printf '...'; sleep 2; } | binary` -- keep stdin open so async tokio runtime processes all messages before EOF (Rosetta/Windows are slower to start)
- MCP stdio smoke tests in zsh: `{ ...; sleep 2; }` brace grouping fails in zsh eval -- use `printf '...\n' | timeout 3 binary` instead
- Debug mode smoke test: `ECKO_DEBUG=1 python3 checks/runner.py --file <path> --mode post-tool-use --cwd <dir> --plugin-root .`
- Stop mode with explicit files: `python3 checks/runner.py --file x --mode stop --cwd <dir> --plugin-root . --files file1.py,file2.py`

## Code style
- All modules use `from __future__ import annotations`
- Check names are kebab-case: `unused-imports`, `unicode-artifact`, `dead-code`
- Tool adapters follow a pattern: `run_<tool>(args) -> list[Echo]` (per-file) or `-> dict[str, list[Echo]]` (multi-file)
- Custom checks follow: `check_<name>(file_path) -> list[Echo]`
- Graceful skip: resolver returns None → return empty list, never error. Never call `shutil.which()` directly in adapters — use `resolve_binary_tool()` for system binaries.
- Adapter error handling: catch `TimeoutExpired` and `OSError` separately, call `emit()` with descriptive warning, return empty
- BFS queues: use `collections.deque` with `popleft()`, never `list.pop(0)` (O(n) per pop)

## Adding a new check
- Tool adapter: add `checks/tools/<name>_adapter.py`, wire into `_run_layer2_checks()` in runner.py
- Custom check: add `checks/custom/<name>.py`, wire into `_run_layer2_checks()` in runner.py
- Register the check name in `ecko.yaml.example` disabled_checks comment
- For AST-based checks on test functions: use `_iter_test_functions()` + `_walk_shallow()` to avoid nested function/class false positives
- For AST-based checks on any functions: use `ast.iter_child_nodes(tree)` for module-level + `ast.iter_child_nodes(cls)` for class-level — never `ast.walk(tree)` which visits nested functions and corrupts parent tracking
- Guard clause filters (in `_is_guard_clause`): skip `self.skipTest`, `pytest.skip/fail`, `raise pytest.skip`, early return, platform guards (`os.name`, `sys.version_info`, `sys.platform`)
- `test-conditional` skips `if` inside `for`/`while`/`async for` loops when the `if` body contains no assertions — data-filtering pattern, not test branching. Loop-with-assert still flagged.
- `_if_body_has_assert()` in test_quality.py uses shallow BFS (same pattern as `_walk_shallow`), NOT `ast.walk()` — avoids false positives from asserts inside nested function defs
- `_LOOP_TYPES` and other check-specific constants are module-level, not function-local (consistency with `_CONSTANT_GUARD_NAMES`, `_SLEEP_NAMES`, etc.)
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
- Dry-run smoke test: `python3 checks/runner.py --file <path> --mode dry-run --cwd <dir> --plugin-root .`
- Stop-mode validation: copy source to tmp dir, `git init` + commit all, modify files (append newline), then run `--mode stop`. Must copy WITHOUT `.git` dir (`shutil.copytree` with `ignore_patterns('.git')`) or nested git confuses `_get_modified_files()`
- Guard integration: create `.ecko-guard.yaml` with test rules in tmp dir, run `--mode post-tool-use`, verify guard rules are enforced alongside ecko.yaml rules
- Config integration tests verify every config field affects runtime behavior -- not just parsing
- Use parallel subagents for multi-repo validation (5 agents x 2 repos each works well)
- CI matrix: `{ubuntu, macos, windows} × {Python 3.10, 3.12}` -- 6 jobs total (`.github/workflows/test.yml`)

## Validation repos (CRITICAL -- always keep up to date)
- **Every FP fixed or missed TP discovered MUST be added to `validation/` before the fix is considered complete.** This is the single most important testing practice.
- Committed at `validation/{python,typescript,go,rust}/` with `bad/`, `clean/`, `boundary/` per language
- Run all: `./validation/run.sh` -- run one: `./validation/run.sh python`
- Each file has a header comment with expected exit code and check name
- `bad/` files: MUST trigger exit 1 + expected check name (TP targets)
- `clean/` files: MUST produce exit 0 (FP guards -- the most important files)
- `boundary/` files: edge cases with documented expected outcomes (known limitations)
- When fixing a FP: add the triggering pattern to `clean/` FIRST, verify it fails, fix the check, verify it passes
- When fixing a missed TP: add the pattern to `bad/` FIRST, verify it doesn't trigger, fix the check, verify it triggers
- `run.sh` parses file headers: `# Expected: exit N` for exit code, `check=name` for check assertion
- Always run `./validation/run.sh` after any check modification -- faster than cargo test for FP/TP verification
- Self-check: all ecko source files (`src/**/*.rs`) must produce 0 echoes

## Releasing
- Bump `version` in both `.claude-plugin/plugin.json` AND `Cargo.toml`
- Update version badge in `README.md`
- Add entry to `CHANGELOG.md`
- Push and wait for CI green on all 3 Rust matrix jobs before tagging
- If CI fails, fix and push again -- do NOT tag until all 3 jobs are green
- Delete tag and re-tag if release CI fails: `git tag -d vX && git push origin :refs/tags/vX`, fix, push, re-tag
- Tag, push tag, `gh release create v{X} --title "..." --notes-file /tmp/release-notes.md` (flag is `-F`/`--notes-file`, NOT `--body`)
- Release CI MCP smoke test requires `sleep 2` after piped JSON-RPC messages for cross-platform reliability
- Release CI (release.yml) triggers on tag push: builds 5 targets, validates artifacts, publishes GitHub Release with checksums
- Verify with: `curl -fsSL https://github.com/naejin/ecko/releases/latest/download/install.sh | bash`
- Update local plugin: `claude plugins update ecko@monet-plugins` (full marketplace qualifier required)
- Update `commands/` listing in Structure section of CLAUDE.md if adding/removing commands
- Update commands table in README.md if adding/removing commands
- CHANGELOG test count must match actual `cargo test` output (currently 350)
- Update test count in both CHANGELOG.md AND CLAUDE.md after final stabilization, not after initial implementation (review rounds add tests)
- Update release notes retroactively: `gh release edit vX --repo naejin/ecko --notes-file /tmp/notes.md`
- Update README.md checks tables when adding new checks

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
- `/ecko:tune`: reads all `.ecko-reverb/*.md`, deduplicates, presents numbered interactive list, applies user selection to `ecko.yaml`, deletes read reverb notes only when items applied (preserves on "none")
- `.ecko-reverb/` is in `_DEFAULT_EXCLUDE_DIRS` — reverb notes are never linted

## Observability (v0.7.0)
- Debug mode: `ECKO_DEBUG=1` env var emits tool resolution, file detection, config, and timing to stderr via `checks/debug.py`
- `_get_modified_files` now includes recently committed files via `git log --since=4h --diff-filter=ACMR`
- `--files` CLI argument for stop mode overrides git detection (comma-separated file list)
- Clean-sweep message: `~~ ecko ~~ clean sweep — 0 echoes across N files (Xs)` when stop finds no issues
- Stop-mode timing: `~~ ecko ~~ finished in Xs` when echoes found
- `placeholder-code` check: flags Python `pass`/`...`/`raise NotImplementedError` sole-body functions (skips abstractmethod, overload, Protocol, test files, .pyi, dunder methods) and JS/TS `throw new Error("not implemented")`
- Shell hooks use `printf` not `echo` for cross-platform consistency

## Session ledger (v0.8.0)
- `.ecko-session/ledger.jsonl` — true append-only (`open("a")`), never read-modify-write. Stale entries filtered at read time by `_read_raw`'s cutoff
- Session boundary: 4h default (matches `--since=4h`), configurable via `session_hours`
- Schema: `{"ts": float, "file": "rel/path", "mode": "post-tool-use"|"stop", "echoes": {"check": count}}`
- Clean files recorded as `{"echoes": {}}` — enables future first-pass-clean rate
- `checks/ledger.py` is a pure data module (imports only `json`, `os`, `time`, `typing`) — no `emit()`, no I/O to stderr
- Self-correction: per-(file, check) count delta between first and last post-tool-use entry — stop hook only, single line output
- Cross-file echo cap: per-check across files in stop mode, default 0 (off), applied in `format_stop_echoes()`
- `record_echoes()` called from `run_post_tool_use()` — records every file including clean ones
- All ledger I/O is try/except guarded — failure never blocks or crashes hooks
- `.ecko-session/` is in `_DEFAULT_EXCLUDE_DIRS` — never linted
- Config keys: `session_hours` (flat, default 4), `echo_cap_cross_file` (flat, default 0)
- `format_correction_summary()` lives in `result.py` — all output formatting goes through `result.py`, not data modules
- When `cross_file_cap` is active, `format_stop_echoes` header includes "(display capped at N per check)" so total count doesn't mislead

## Configurable rules (v0.9.0)
- `ruff_extra_rules` config key: flat string list of ruff rule codes appended to ecko's built-in `--select`
- Valid format: `^[A-Z]+[0-9]*$` (accepts full codes like C901, prefixes like UP, and longer prefixes like ASYNC)
- Unmapped codes use lowercased code as check name (e.g., C901 -> c901)
- `disabled_checks` already handles suppression — no `ruff_disabled_rules` needed
- Runner decomposition: `checks/bash_guard.py` (bash command guard) + `checks/git.py` (git file detection) extracted from runner.py
- `checks/bash_guard.py` is self-contained (imports config, regex_utils, result) — same concern boundary as `checks/debug.py`
- `checks/git.py` contains `get_modified_files()` and `normalize_path()` — pure subprocess/os utilities
- Re-exports in runner.py for backward compat: `_normalize_path = normalize_path`, `_get_modified_files = get_modified_files`
- `get_modified_files()` uses `session_hours` config (not hardcoded 4h) — format: `--since={minutes}m`
- Ledger pruning: `_maybe_prune()` called from `read_session()`, dual-condition (>50% stale AND >50KB), atomic via `os.replace()`
- Per-tool timing: debug-mode only, `layer3: {tool} completed in {N}s`
- `get_modified_files()` uses minutes format (`--since={N}m`) for sub-hour `session_hours` precision
- Swarm reports for version planning live in `swarm/` — future versions should reference prior swarm deferred-item tables

## Project config (v1.0)
- `ruff_use_project_config`: bool, drops `--select`, defers to project ruff.toml/pyproject.toml
- `biome_use_project_config`: bool, drops `--config-path`, uses project biome.json/biome.jsonc
- `/ecko:session` command: reads session ledger, shows files touched, top checks, self-correction rate
- `format_session_stats()` in result.py: one-line session summary after stop hook output
- Runner section comments: `# --- Filtering ---`, `# --- Tool availability ---`, `# --- Layer 2 dispatch ---`, `# --- Layer 3 dispatch ---`

## Severity + machine output (v1.1)
- Echo dataclass gains `severity: str = "warn"` field
- `has_errors(echoes)` helper in result.py
- `format_file_echoes_json()` and `format_stop_echoes_json()` in result.py — schema v1
- `output_format` config key: `"text"` (default) or `"json"`, read once in runner
- `_KNOWN_KEYS` updated: `output_format`, `ruff_use_project_config`, `biome_use_project_config`

## Go + Rust (v1.2)
- `checks/tools/golangci_adapter.py`: `run_golangci(cwd, modified_files) -> dict[str, list[Echo]]`
- `checks/tools/clippy_adapter.py`: `run_clippy(cwd, modified_files) -> dict[str, list[Echo]]`
- Both Layer 3 only, dispatched in `run_stop()` thread pool
- golangci-lint: `--out-format json`, capital-I `"Issues"`, check names `go-{linter}`
- clippy: `--message-format=json`, streaming JSON (one object per line), check names `rust-{code}`, uses primary span (`is_primary: true`)
- LANG_MAP extended: `.go` → `"go"`, `.rs` → `"rust"`
- Both use `resolve_binary_tool()` (not `shutil.which` directly)
- Severity mapped from tool output: golangci `Severity` field, clippy `level` field
- Go blank imports (`_ "pkg"`): skip in unused-imports. Alias imports: use alias name, not path segment
- Edge cases verified in `validation/go/clean/` (blank imports, alias imports, nil guards)

## Fingerprinting + dry-run (v1.3)
- `checks/fingerprint.py`: detects Django, Flask, FastAPI, Express, Next.js, React, Vue from requirements.txt/pyproject.toml/package.json
- 10KB cap on dependency files, no dev/main dep distinction (known limitation: FastAPI test deps can detect flask)
- `--mode dry-run`: lists applicable checks without executing tools, always returns 0
- `run_dry_run()` in runner.py, outputs to stdout (informational, not a hook)

## Architecture Guard (v2.2.0)
- `.ecko-guard.yaml` — temporary architectural guardrails, gitignored, separate from permanent `ecko.yaml`
- Same `EckoConfig` rule types: `banned_patterns`, `import_rules`, `custom_checks`, `blocked_commands`
- Plus metadata: `created` (f64 Unix epoch seconds), `task` (string)
- `EckoGuardConfig` struct in `config.rs` — deserialization target for guard file
- `GuardMeta` struct in `config.rs` — lifecycle metadata stored in `EckoConfig.guard_meta` (`#[serde(skip)]`)
- `guard_check_names: HashSet<String>` in `GuardMeta` — tracks which check names came from guard file (for friction detection)
- `merge_guard_config()` in `config.rs` — loads `.ecko-guard.yaml`, extends rule arrays, populates `guard_meta`
- `load_config()` calls `merge_guard_config()` after loading `ecko.yaml`
- `emit_guard_lifecycle()` in `runner.rs` — age nudge (>=7 days) + friction detection (3+ files per guard check)
- Called from `run_stop()` after all output, not from `run_stop_inner()` (lifecycle is CLI-only, not MCP)
- `/ecko:guard` command: `commands/guard.md` — generate rules from conversation context, `--review`, `--clear`
- `.ecko-guard.yaml` excluded from linting by language detection (`.yaml` returns `Lang::Unknown`)

## Deep module pattern (v2.2.0)
- `StopResult` struct in `runner.rs` — carries all stop-mode results (echoes, elapsed, corrections, session entries, file count, config)
- `run_stop_inner()` — core logic, returns `StopResult`. Used by both CLI hook and MCP tool.
- `run_stop()` — thin wrapper that formats output and returns exit code
- `tools::check_workspace()` in `mcp/tools.rs` — delegates to `run_stop_inner()`, formats as JSON. Single codepath for workspace checks.
- MCP `status()` uses `env!("CARGO_PKG_VERSION")` — version can never drift from Cargo.toml

## Current version and next milestone
- Current: v2.3.0 (6 bug fixes, obsolete-terms check, structural prevention)
- Previous: v2.2.1 (reverb note preservation on tune "none")
- Previous: v2.2.0 (deep modules, architecture guard, README rewrite)
- Previous: v2.1.0 (validation suite, FP fixes, Go alias/blank imports, guard hardening)
- Previous: v2.0.0 (Rust rewrite with tree-sitter + MCP server)
- Previous: v1.3.0 (Python, fingerprinting + dry-run)

## Not part of the plugin
- `docs/ideas/` — internal ideation (gitignored)
- `openspec/`, `.claude/` — dev workflow tooling, not distributed

---

# Ecko v2 — Rust Rewrite

## What changed
v2.0.0 rewrites ecko from Python to Rust with tree-sitter as the analysis engine.
All checks are native (no external tool dependencies for core checks). MCP server mode added.
The Python code in `checks/` still exists but hooks now point to the Rust binary.

## Rust structure
- `Cargo.toml` — dependencies: tree-sitter grammars (py/js/ts/go/rs), regex, serde, clap, rayon, rmcp
- `src/main.rs` — CLI: `--mode {post-tool-use,stop,pre-tool-use-bash,dry-run,mcp-server}`
- `src/runner.rs` — orchestrator: `run_post_tool_use()`, `run_stop()`, `run_dry_run()`
- `src/config.rs` — `ecko.yaml` via serde_yaml, `EckoConfig` struct with all accessors
- `src/echo.rs` — `Echo` struct (with `Fix` + `Severity`), compact text + JSON formatters, `emit()`, `apply_per_check_cap()`
- `src/lang.rs` — `Lang` enum, `detect_language()`, `parse_for_checks()`, `is_test_file()`
- `src/query_engine.rs` — `QueryCheck` struct, `compile_query()`, `run_query()`, `capture_index_or_skip()`
- `src/checks/` — per-language check modules: `python.rs`, `javascript.rs`, `go.rs`, `rust_checks.rs`, `universal.rs`, `custom.rs`, `dead_code.rs`
- `src/external/` — optional subprocess adapters: `pyright.rs`, `tsc.rs`, `golangci.rs`, `clippy.rs`
- `src/mcp/` — MCP server via rmcp: `mod.rs` (ServerHandler), `tools.rs` (check_file, status, explain, etc.)
- `src/ledger.rs` — session JSONL ledger (append-only, same schema as Python)
- `src/guard.rs` — bash command guard (regex, lazy-compiled via `LazyLock`)
- `src/git.rs` — `get_modified_files()` via git subprocess
- `src/fix.rs` — fix suggestion generation (byte-range replacements)
- `src/fingerprint.rs` — framework detection from dependency files
- `src/formatter.rs` — Layer 1 autofix (trailing whitespace + optional black/prettier)
- `src/suppress.rs` — `ecko:ignore` inline comment suppression; supports both space-separated (`# ecko:ignore unused-imports`) and bracket notation (`# ecko:ignore[unused-imports]`)
- `src/debug.rs` — `ECKO_DEBUG=1` stderr output via `OnceLock`
- `queries/` — tree-sitter `.scm` files embedded at compile time via `include_str!()`
- `.claude-plugin/.mcp.json` — MCP server config for Claude Code

## Rust build + test
- Always run `cargo fmt` before committing -- CI runs `cargo fmt --check` and rejects unformatted code (common issue: multi-item-per-line const arrays)
- Build: `cargo build --release` (8.2MB binary, ~30s)
- Test: `cargo test` (350 tests, ~1s)
- Check: `cargo check` (fast type-check without codegen)
- Smoke test: `target/release/ecko --mode post-tool-use --file <path> --cwd <dir> --plugin-root .`
- Bash guard: `echo "COMMAND" | target/release/ecko --mode pre-tool-use-bash --cwd . --plugin-root .`
- Dry-run: `target/release/ecko --mode dry-run --file <path> --cwd <dir> --plugin-root .`
- Session stats: `target/release/ecko --mode session-stats --cwd <dir> --plugin-root .`
- JSON output: set `output_format: json` in ecko.yaml
- Debug mode: `ECKO_DEBUG=1 target/release/ecko --mode post-tool-use ...`
- MCP smoke test: `printf '...' | target/release/ecko --mode mcp-server` (should register 5 tools)

## Binary distribution
- `scripts/run.sh` / `scripts/run.cmd`: 3-tier launcher (pre-built binary -> cargo build -> GitHub Release download)
- Hooks do NOT use run.sh (tight timeouts, graceful degradation to exit 0 if binary missing)
- run.sh is for: MCP server entry point (plugin.json mcpServers), slash commands, install script
- plugin.json has inline `mcpServers` (no separate .mcp.json -- avoids path conflicts)
- Release CI: 5-target matrix (linux x86_64/aarch64, macos x86_64/aarch64, windows x86_64)
- Checksum verification: run.sh (sha256sum/shasum) and run.cmd (PowerShell Get-FileHash) verify checksums before executing downloaded binary
- Hooks have tight timeouts (PostToolUse: 30s, PreToolUseBash: 10s, Stop: 120s) -- never add network I/O or cargo builds to hook scripts
- Windows release CI overwrites plugin.json `mcpServers.ecko.command` to point to `run.cmd` instead of `run.sh`
- CI lint job: `cargo fmt --check` + `cargo clippy -- -D warnings` runs on every push/PR (`.github/workflows/test.yml`)

## Rust design constraints
- `EckoConfig` fields are public (`cfg.session_hours`, `cfg.disabled_checks`) -- access directly, no getter methods
- tree-sitter queries embedded via `include_str!()` — single binary, no external files
- `parse_for_checks(lang, source)` in lang.rs — use this instead of manually creating parser+tree
- `capture_index_or_skip(query, name)` in query_engine.rs — returns `usize::MAX` on missing capture (safe no-match sentinel), never `unwrap_or(0)` which silently uses wrong capture
- `Severity` derives `Copy` — use `Severity::Warn` / `Severity::Error` directly, never `.clone()`
- Rust `regex` crate is inherently ReDoS-safe — no thread-based timeouts needed (unlike Python)
- Timestamp math: use `std::time::SystemTime` + `UNIX_EPOCH` for age calculations — never add `chrono` (zero-dependency constraint)
- MCP tools must delegate to `runner.rs` functions — never reimplement check/stop logic in `mcp/tools.rs` (Deep Module pattern)
- Version strings in Rust code: use `env!("CARGO_PKG_VERSION")` — never hardcode version numbers
- Guard patterns lazy-compiled via `LazyLock<Vec<(Regex, &str)>>` — compiled once per process
- `run_with_timeout(cmd, timeout, tool_name)` in external/mod.rs drains stdout/stderr via threads to prevent pipe buffer deadlock; emits user-facing "not found" vs "timed out" messages
- GlobSet for user excludes pre-compiled once in `run_stop()`, passed to filter functions
- Session stats only emitted when ledger has entries (silent on first run)
- All output goes to stderr via `echo::emit()` — stdout only for dry-run/informational modes
- `--force-with-lease --force` bypass: strip `--force-with-lease` then check for standalone `--force`
- Guard regex patterns for git subcommands use `git\b.*\bsubcommand` (not `git\s+subcommand`) to match regardless of intervening args like `-C /dir`

## Rust check inventory (29 native checks)
- Python (12): unused-imports, singleton-comparison, bare-except, star-imports, mutable-default-args, builtin-shadowing, placeholder-code, unreachable-code, duplicate-keys, test-conditional, fixed-wait, mock-spec-bypass
- JS/TS (8): unused-imports, unreachable-code, debugger-statement, no-var, duplicate-keys, empty-block-statements, useless-catch, placeholder-code
- Go (4): unused-imports, empty-error-check, unreachable-code, placeholder-code
- Rust (4): unused-imports, todo-macro, unreachable-code, placeholder-code
- Universal (4+): unicode-artifacts, banned-patterns, import-layers, obsolete-terms
- External optional: pyright, tsc, golangci-lint, clippy

## Adding a Rust check
1. Create tree-sitter query in `queries/<lang>/<check>.scm`
2. Add check function in `src/checks/<lang>.rs` using `parse_for_checks()` + `compile_query()` + `capture_index_or_skip()`
3. Wire into `run_checks()` in that module
4. Add check name to `list_applicable_checks()` in `src/checks/mod.rs`
5. Add check name to `mcp/tools.rs` `status()` and `explain()` functions -- EVERY check in `list_applicable_checks()` must have an explain entry
6. Add unit test with inline source string

## Rust v2 config changes (vs Python v1)
- Removed: `ruff_use_project_config`, `biome_use_project_config`, `ruff_extra_rules`
- Added: `custom_checks` (tree-sitter query checks in ecko.yaml), `fix_suggestions` (bool, default true)
- Added: `.ecko-guard.yaml` (temporary guard rules, merged by `merge_guard_config()` in config.rs)
- Kept: `disabled_checks`, `exclude`, `banned_patterns`, `obsolete_terms`, `blocked_commands`, `autofix`, `deep_analysis`, `echo_cap_per_check`, `echo_cap_cross_file`, `session_hours`, `output_format`, `reverb`, `builtin_shadow_allowlist`, `import_rules`
- `obsolete_terms` now has a native Rust check: `ObsoleteTermRule` struct (`{old: String, new: String, glob: String}`), matched via regex with glob-based file filtering
- `ecko.yaml.example` uses `deny` (not `deny_import`) for import rule action field -- validation tests prevent example/config drift

## Incomplete / future work (v2.1)
- Incremental parsing in MCP server mode (tree-sitter supports it, not implemented)
- Diff-aware checking (only check changed subtrees)

## Rust code quality gotchas
- Rust unused-imports: check usage per-import across entire file (before AND after), not just after last import -- `#[cfg(test)]` modules push `import_end` past main code
- Rust unreachable-code: skip `line_comment`/`block_comment` nodes -- tree-sitter parses them as named block children
- Rust trait imports: `TRAIT_IMPORTS` allowlist in `rust_checks.rs` -- add new traits as needed
- Edge cases verified in `validation/rust/clean/` (derive macros, trait imports, test modules, comments after return)
- AI agents insert unicode (em dashes, arrows) in doc comments -- always `sed -i 's/\xe2\x80\x94/--/g'` after bulk code generation
- Check name strings must match between echo emission and `list_applicable_checks()`/`status()`/`explain()` -- always verify against the actual `check:` field in the check implementation. Universal checks use plural: `"banned-patterns"`, `"import-layers"`. Language checks use singular: `"unused-imports"`, `"bare-except"`.
- `status()` and `explain()` in `mcp/tools.rs` must stay in sync -- every entry in `all_checks` array must have a corresponding `explain()` match arm
- `relative_path()` lives in `git.rs` -- single source of truth, never duplicate in other modules
- `canonicalize_or_normalize()` lives in `git.rs` -- shared by clippy and golangci adapters for path matching, never duplicate
- `explain()` in mcp/tools.rs uses `match` not `HashMap::from` -- zero allocation per call
- `list_applicable_checks()` uses const arrays per language -- add new checks to the array, not individual pushes
- `run_stop()` delegates external adapter dispatch to `run_external_adapters()` -- keep orchestration under ~200 lines
- `collect_all_list()` in dead_code.rs scans from `__all__` position only -- prevents false "used" from strings before `__all__`
- `run_with_timeout()` joins reader threads on timeout path -- prevents thread leaks
- `echo::emit()` uses `writeln!` internally -- callers must NOT include trailing `\n` (causes blank lines)
- Custom check queries validated at config load time via `validate_custom_checks()` -- invalid queries emit immediate warning
- Self-check: `target/release/ecko --mode post-tool-use --file src/runner.rs --cwd . --plugin-root .` should produce 0 echoes

## Review-fix-converge workflow
- Launch code-reviewer, code-architect, and code-simplifier agents in parallel
- Apply fixes, then run convergence review (same 3 agents checking only for regressions)
- Typically converges in 2 rounds (Round 1: ~25 findings, Round 2: ~8 findings, Round 3: converged)
- Always rebuild release binary and run self-check after convergence

## rmcp MCP server patterns
- Parameter structs derive `JsonSchema` from `rmcp::schemars` (v1), NOT standalone `schemars` (v0.8)
- Use `Parameters<T>` wrapper for tool function params, not `#[tool(param)]` on individual args
- `ServerHandler::get_info()` returns `InitializeResult`, not `ServerInfo`
- `InitializeResult::new()` sets `server_info` via `Implementation::from_build_env()` which uses rmcp's crate name/version, NOT the consuming crate's -- always set `result.server_info.name` and `.version` explicitly with `env!("CARGO_PKG_VERSION")`
- `rmcp::serde` re-export for Serialize/Deserialize on MCP param types
- Features needed: `server`, `macros`, `transport-io`
- `#[tool_handler]` macro on `impl ServerHandler` is REQUIRED for tools/list to work -- without it, `list_tools` returns empty and `call_tool` is a no-op. Import via `use rmcp::tool_handler;`
- `#[tool_router]` on the impl block defines tools; `#[tool_handler]` on the ServerHandler impl wires them to the protocol -- both are needed

## tree-sitter patterns
- `streaming_iterator::StreamingIterator` trait needed for `QueryCursor::matches()` iteration (`while let Some(m) = matches.next()`)
- Capture name comparison: `*name == "match"` (double-deref because `capture_names()` returns `&[&str]`)
- Grammar crate constants: `tree_sitter_python::LANGUAGE`, `tree_sitter_typescript::LANGUAGE_TYPESCRIPT` / `LANGUAGE_TSX`
- Convert to `tree_sitter::Language` with `.into()`
- Node kinds vary by grammar — always debug with `tree.root_node().to_sexp()` when writing new queries
- Post-filter pattern: query broadly, then filter matches in Rust code (e.g., `except_clause` query → filter to only bare `except:`)
