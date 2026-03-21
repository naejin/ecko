# Changelog

## v1.3.0

Intelligence — project fingerprinting and dry-run introspection.

### New features

- **Project fingerprinting** (`checks/fingerprint.py`) — detects frameworks from marker files
  (requirements.txt, pyproject.toml, package.json). Feeds vulture adapter with framework-specific
  skip lists to reduce false positives (FastAPI DI params, Flask globals, Django metaclass attrs).
  No auto-configuration — conservative scope.
- **`_FRAMEWORK_VULTURE_SKIPS`** in vulture adapter — FastAPI (`db`, `session`, `request`,
  `response`, `Depends`), Flask (`app`, `g`, `request`, `session`), Django (`request`, `queryset`,
  `Meta`, `verbose_name`). Applied automatically when framework detected.
- **`--mode dry-run`** — lists which checks would run for a given file without executing any tools.
  Shows language detection, tool availability (found/not found), disabled checks. Always returns 0.

### Tests

- 568 total (up from 548). New: fingerprint (11), dry-run (7), format updates (2).

## v1.2.0

Go + Rust — Layer 3 deep analysis for Go and Rust projects.

### New features

- **Go support via golangci-lint** — `checks/tools/golangci_adapter.py`. Runs `golangci-lint run
  --out-format json ./...`. Check names: `go-{linter}` (e.g., `go-errcheck`, `go-staticcheck`).
  Post-filters to modified files. Timeout: 120s. Tool resolution: `shutil.which` (Go binary).
- **Rust support via clippy** — `checks/tools/clippy_adapter.py`. Runs `cargo clippy
  --message-format=json`. Streaming JSON parsing (one object per line). Check names: `rust-{code}`
  (e.g., `rust-clippy::needless_return`). Gated on `Cargo.toml`. Timeout: 120s.
- `.go` and `.rs` added to `LANG_MAP` for language detection.
- `golangci-lint` and `clippy` added to `_INSTALL_HINTS`.
- Both dispatched in Layer 3 thread pool (stop mode only).

### Tests

- 548 total (up from 526). New: golangci adapter (12), clippy adapter (10).

## v1.1.0

Severity + Machine Output — echo severity levels and structured JSON output for tooling.

### New features

- **Severity on Echo** — `severity` field on the Echo dataclass. Default `"warn"`. Error-severity
  echoes get `[error]` prefix in text output. Internal defaults only:
  - `"error"`: bare-except (E722), star-imports (F403), unreachable-code, type-error (pyright),
    biome error-category diagnostics
  - `"warn"`: everything else
- **`output_format` config key** — set to `"json"` for machine-readable JSON output on stderr.
  Schema version 1. No echo caps applied in JSON mode (machine consumers need complete data).
  Exit codes unchanged regardless of format.
- **`has_errors()` helper** — `has_errors(echoes)` returns `True` if any echo has error severity.

### Eliminated

- **`ruff_disabled_rules`** — `disabled_checks` already handles suppression via ecko check names.
  Documented the interaction in `ecko.yaml.example`.

### Tests

- 526 total (up from 488). New: severity (16), JSON output (22).

## v1.0.0

Project Config — bring your own ruff and biome configurations into ecko's detect-echo-correct loop.

### New features

- **`ruff_use_project_config`** — boolean config key. When `true`, ruff defers to your project's
  `ruff.toml` / `pyproject.toml [tool.ruff]` instead of ecko's built-in rule selection. `--no-fix`
  is always enforced (safety invariant — project `fix = true` would create infinite hook loop).
  Echoes are still filtered by `disabled_checks` using ecko check names.
- **`biome_use_project_config`** — boolean config key. When `true`, biome defers to your project's
  `biome.json` / `biome.jsonc` instead of ecko's bundled config. Unknown biome rules are auto-mapped
  to kebab-case ecko check names via `_to_kebab()` (e.g., `noDoubleEquals` → `no-double-equals`)
  and can be disabled via `disabled_checks`. Falls back to ecko's config with a note if no project
  config is found.
- **Session stats in stop output** — after the self-correction summary, a one-line session summary:
  `~~ ecko ~~ session: 47 echoes across 8 files, 12 self-corrected`
- **`/ecko:session` command** — slash command that reads the session ledger and presents a structured
  summary: files touched, total echoes, top 5 checks, self-correction count, clean-first-pass rate.
- **Runner section comments** — `# --- Filtering ---`, `# --- Tool availability ---`,
  `# --- Layer 2 dispatch ---`, `# --- Layer 3 dispatch ---` for navigability. No extraction.

### Internal

- `checks/session_stats.py` — standalone script for `/ecko:session` command
- `commands/session.md` — slash command definition
- 488 total tests (up from 448). New: ruff project config (12), biome project config (19),
  session stats (5+4), adapter behavior change (2).

## v0.9.1

Noise reduction — two false positive patterns eliminated at the source.

### Noise fixes

- **`empty-error-handlers` (S110) removed from built-in ruff rules** — `try/except Exception: pass`
  is a legitimate guard pattern (best-effort I/O, optional imports, graceful degradation). E722
  (`bare-except`) already catches the truly dangerous case. Users who want strict "no try-except-pass"
  can re-enable via `ruff_extra_rules: [S110]`. Note: re-enabled S110 echoes use check name `s110`
  (not `empty-error-handlers`), so update `disabled_checks` accordingly.
- **`test-conditional` no longer flags data-filtering `if` inside loops** — `if line.strip():` inside
  a `for` loop with no assertions in the body is data filtering, not test branching. `if` inside a
  loop WITH assertions is still flagged.

### Tests

- 448 total (up from 438). New: S110 removal verification, test-conditional loop filtering
  (data filter skip, while loop skip, async for skip, loop-with-assert preserved, outside-loop
  preserved, nested-function-assert edge case), ASYNC prefix validation.

## v0.9.0

Your Rules — bring your lint standards into ecko's detect-echo-correct loop.

### New features

- **Extra ruff rules** (`ruff_extra_rules`) — add any ruff rule code to ecko's checks via
  `ecko.yaml`. Accepts full codes (`C901`, `N801`) and category prefixes (`UP`, `SIM`).
  Unmapped codes use their lowercased code as the check name (e.g., `C901` becomes `c901`).
  Invalid codes are warned and skipped at config load.
- **Ledger pruning** — `.ecko-session/ledger.jsonl` is automatically compacted when >50%
  of entries are stale and the file exceeds 50KB. Atomic rewrite via temp file.
- **Per-tool timing** — Layer 3 tool timing visible in debug mode (`ECKO_DEBUG=1`).

### Bug fixes

- **`_get_modified_files` hardcoded `--since=4h`** — the git log lookback window now
  respects the `session_hours` config value instead of always using 4 hours.

### Architecture

- **Runner decomposition** — `checks/bash_guard.py` (~80 lines) and `checks/git.py`
  (~55 lines) extracted from `runner.py`. Runner drops from 737 to ~600 lines. All
  existing imports continue to work via re-exports.

### Tests

- 438 total (up from 399). 39 new: New: extra ruff rules config/adapter/integration, bash guard
  module extraction, git module extraction with session_hours bug fix, ledger pruning lifecycle.

## v0.8.0

Session awareness — ecko remembers what happened and proves its value.

### New features

- **Session ledger** — ecko records echo counts per file in `.ecko-session/ledger.jsonl`
  (JSONL format). Each post-tool-use invocation appends one entry. Entries older than
  the session window (default 4h, configurable via `session_hours`) are pruned automatically.
- **Self-correction tracking** — stop hook reads the session ledger and reports how many
  echoes the agent fixed: `~~ ecko ~~ self-corrections: 3 fixed (3 unused-imports)`.
  Compares first vs last entry per (file, check).
- **Cross-file echo cap** (`echo_cap_cross_file`) — limits repeated echoes of the same check
  across all files in stop mode. Default 0 (unlimited). Prevents info overload when a single
  check floods output across many files.

### Tests

- 399 total (up from 347). New: ledger operations, self-correction computation, cross-file cap,
  config getters, integration tests for ledger lifecycle.

## v0.7.0

Observability — ecko always tells you what it did.

### Bug fixes

- **`_get_modified_files` blind spot** — files committed during a session were invisible to the
  stop hook. Now also checks recently committed files via `git log --since=4h`.
- **`echo` → `printf` in exit_plan_mode.sh** — cross-platform escape sequence consistency.

### New features

- **Debug mode** (`ECKO_DEBUG=1`) — emits tool resolution, file detection, config loading, and
  timing to stderr. Off by default.
- **Clean-sweep message** — stop hook now emits `~~ ecko ~~ clean sweep — 0 echoes across N files`
  when all checks pass, instead of silent nothing.
- **Stop-mode timing** — stop hook reports total elapsed time.
- **`--files` argument** for stop mode — explicit file list, bypasses git detection.

### New check

- **`placeholder-code`** — flags Python functions whose body is only `pass`, `...`, or
  `raise NotImplementedError` (excluding abstract methods, protocols, overloads, type stubs,
  test files). JS/TS: flags `throw new Error("Not implemented")`.

### Tests

- 347 total (up from 314). New: debug mode, placeholder detection, `_get_modified_files` fix,
  clean-sweep, `--files` argument, debug integration, nested function exclusion, block comment handling.

## v0.6.1

Tech debt reduction plus reverb/tune UX fixes.

### Commands

- **`/ecko:reverb` (new)** — dedicated slash command to capture what went wrong; writes a structured note to `.ecko-reverb/`
- **`/ecko:tune` (rewritten)** — presents recommendations as an interactive numbered list; user selects which items to apply; processed reverb notes are cleaned up automatically
- **Reverb tip simplified** — stop-mode reverb nudge replaced with a single-line tip (`tip: run /ecko:reverb to capture what went wrong`) to prevent agent write loops

### Internals

- **Shared `checks/regex_utils.py`** — unified ReDoS-safe `safe_regex_compile`, `safe_regex_search`, `safe_regex_finditer` with thread-based timeout. Replaces two independent implementations in `runner.py` and `banned_patterns.py`.
- **Shared `checks/fileutil.py`** — canonical `is_test_file()` predicate. Now includes `conftest.pyi` (previously only in vulture adapter).
- **Thread-explosion fix** — `check_banned_patterns` now uses `finditer` over the full source (1 thread per pattern) instead of per-line search (1 thread per line per pattern). 500-line file × 3 patterns: 1,500 threads → 3 threads.
- **Bash guard broadened** — `git push --force`, `git reset --hard`, `git clean -f` patterns now catch any git global options (`--git-dir`, `--work-tree`, `-c`, `--bare`, etc.), not just `-C`.
- **Config warning dedup** — `_emit_config_warnings` now emits once per cwd per session instead of on every hook invocation.
- **Tri-state removal** — `_run_layer2_checks` signature changed from `bool | None` to `bool` for `ruff_available`/`biome_available`. Callers pre-resolve availability.
- **Config validation ReDoS-safe** — `validate_config()` uses `safe_regex_compile()` instead of bare `re.compile()` for user-supplied patterns.
- **CRLF preservation** — `_strip_trailing_whitespace` now preserves `\r\n` and `\r` line endings instead of silently converting to `\n`.
- **Timeout cache fix** — `safe_regex_compile` no longer permanently caches `None` for timed-out patterns; only genuine `re.error` failures are cached.
- **`_run_with_timeout` helper** — all three regex utility functions share a single thread-management implementation.

### Tests

- New `tests/test_regex_utils.py` — 11 tests covering compile, search, finditer, caching, ReDoS timeout.
- Bash guard: 7 new tests for `--git-dir`, `--work-tree`, `-c`, `--bare` bypass variants.
- Banned patterns: finditer line number accuracy, empty file, single-line edge case.
- Config: ReDoS pattern validation, dedup same/different cwd.
- Total: 314 tests (up from 285).

## v0.6.0

Transparency & trust — when ecko runs, users always know what happened. Silent failures are gone, duplicate code is eliminated, and tool adapters have unit tests.

### Transparency (P0 — 6/10 agent consensus)

- **Adapter-level failure reporting** — all 6 tool adapters (ruff, biome, pyright, tsc, knip, vulture) now separately catch `TimeoutExpired` vs `OSError` and emit `~~ ecko ~~ warning: {tool} timed out on {file} ({N}s limit)` or `~~ ecko ~~ warning: {tool} failed: {error}` to stderr. Users always know when a check didn't run.
- **Thread pool error reporting** — `run_stop()` no longer silently swallows exceptions from Layer 3 futures. Failed tools emit `~~ ecko ~~ warning: {tool} failed during deep analysis: {error}`.
- **Hook JSON parse failure reporting** — `pre_tool_use_bash.sh` and `post_tool_use.sh` now emit a warning to stderr on JSON parse failure instead of silently producing an empty string.
- **Skipped-tool install hints** — instead of `ruff (not found)`, ecko now emits `ruff not found — try: pip install ruff (or uvx ruff)` with tool-specific install suggestions.
- **Echo cap transparency** — when echoes are capped, output now includes `(capped at N per check — set echo_cap_per_check: 0 in ecko.yaml to see all)` so users understand the limit is configurable.

### Architecture (P1 — 4/10 agent consensus)

- **Extracted Layer 2 dispatch** — `_run_layer2_checks()` replaces ~80 lines of duplicated check dispatch logic that existed in both `run_post_tool_use()` and `run_stop()`. New checks now only need to be added in one place.

### UX wins

- **Import-layer line numbers** — `check_import_layers` now reports the actual line number of the violating import (via AST node for Python, regex match offset for JS/TS) instead of `line=0`.
- **`.test-d.ts` exclusion** — tsd type assertion files are now skipped from all linting, fixing known false positives on TypeScript repos like Chalk.
- **Bash guard: full-path bypass protection** — patterns now match `/bin/rm`, `/usr/bin/rm`, `command rm`, `\rm`, and `git -C /path push --force` variants.
- **ReDoS: `re.compile()` protected** — user-supplied regex in `banned_patterns` is now compiled inside the same timeout protection as `re.search()`. A pathological regex like `(a+)+b` can no longer hang at compile time.
- **Bash guard block messages** — blocked commands already showed the reason (e.g., "use --force-with-lease instead"), but git commands with `-C /path` prefix are now caught too.

### Test coverage (P1 — 3/10 agent consensus)

- **New `tests/test_tool_adapters.py`** — 30 unit tests covering output parsing for all 6 tool adapters: JSON parsing, allowlist filtering, timeout/crash warning emission, edge cases (empty output, invalid JSON, unresolved tools).
- **Bash guard tests** — bypass variants (`/bin/rm`, `\rm`, `command rm`, `git -C`), `git push -f` short form, `--force-with-lease` combined ordering, multiple `-C` flags.
- **Import-layer line number tests** — multiline Python + JS, commented-out JS import skipping, block comment skipping.
- **ReDoS tests** — pathological regex `(a+)+b` tested in both bash guard and banned_patterns with wall-clock assertions.
- **285 total tests** (234 → 285, +51 new).

### Config UX

- **Effective config in `/ecko:status`** — status command now shows computed effective config including defaults: echo_cap, shadow_allowlist size, disabled_checks, exclude patterns, import_rules count, blocked_commands count, and reverb status.

### Bug fixes

- **Pyright resolver mismatch** — `run_stop()` used `resolve_node_tool("pyright")` but the adapter uses `resolve_python_tool("pyright")`. This caused pyright to silently not run when installed via pip/uvx. Fixed to use `resolve_python_tool`.
- **`git push -f` not blocked** — the `-f` short form for `--force` was not caught by the bash guard. Now matched alongside `--force`.
- **`--force-with-lease --force` false positive** — combining both flags in any order was incorrectly blocked. Now uses a command-wide negative lookahead so `--force-with-lease` anywhere in the command prevents blocking.
- **JS import-layer false positives from comments** — commented-out imports (`// import ...` and `/* import ... */`) were incorrectly flagged. Now skipped via `_is_in_js_comment()` heuristic.
- **`_walk_shallow` O(n^2)** — `list.pop(0)` in the BFS queue replaced with `collections.deque.popleft()` for O(1) per operation.
- **Fixture cache stale on new conftests** — `_fixture_cache` could not detect newly added `conftest.py` files. Now re-globs and compares path lists on each invocation.
- **Regex compile cache** — `_safe_regex_compile()` now caches compiled patterns by string, avoiding redundant thread spawns across files.

## v0.5.1

Trust, safety, and performance — based on independent consensus from 10 code review agents.

### Trust

- **Skipped-tool reporting** — when a tool is unavailable (not installed, no uvx/npx), ecko now emits `~~ ecko ~~ note: ruff (not found) — install for deeper checks` instead of silent nothing. Transforms the "is it working?" experience.
- **Config validation** — unknown keys in `ecko.yaml` produce warnings with "did you mean?" suggestions. Invalid regex patterns in `banned_patterns` and `blocked_commands` are reported at config load time.

### Safety

- **Expanded bash guard** — now blocks `git push --force` (suggests `--force-with-lease`), `git reset --hard` (suggests stash/revert), and `git clean -f` (suggests `-n` preview) in addition to existing `--no-verify` and `rm -rf /~` blocks.
- **ReDoS protection** — user-supplied regex patterns in `banned_patterns` and `blocked_commands` now run with a 500ms thread-based timeout, preventing catastrophic backtracking from hanging ecko.

### Performance

- **Parallel Layer 3** — tsc, pyright, vulture, and knip now run concurrently via `ThreadPoolExecutor`. 2-3x speedup when multiple tools are enabled.
- **Vulture scoped to modified files** — vulture now receives the modified file list instead of scanning `.`, dramatically faster on large repos.
- **tsc/knip post-filtered** — Layer 3 results from tsc and knip are filtered to modified files only, reducing noise from pre-existing issues.
- **Fixture collection cache** — `_collect_fixture_names()` results are cached per cwd, avoiding redundant conftest.py AST parsing.

### Rename

- **Learnings renamed to Reverb** — `learnings` config key is now `reverb`, `.ecko-learnings/` directory is now `.ecko-reverb/`. The name fits ecko's acoustic metaphor — reverb is what lingers after echoes fade. **Breaking:** users with `learnings.enabled: true` must change to `reverb.enabled: true`.

### Bug fixes

- **`encoding="utf-8"` in 3 files** — `formatter.py`, `banned_patterns.py`, `duplicate_keys.py` now use `encoding="utf-8"` for all `open()` calls. Fixes silent corruption on Windows where Python defaults to cp1252.
- **Config loop hoist** — `banned` and `obsolete` config values moved before the per-file loop in `run_stop()`, matching the constraint documented in CLAUDE.md.

## v0.5.0

Noise reduction and architecture enforcement — fewer false positives, smarter filtering, and import layer rules.

### Noise reduction

- **Builtin-shadowing allowlist** — 20 common names (`type`, `help`, `input`, `format`, `id`, `repr`, `ascii`, etc.) are now allowed by default when used as function parameters (ruff A001/A002). Customizable via `builtin_shadow_allowlist` in `ecko.yaml`. User list replaces the default entirely.
- **Echo cap per check per file** — repeated echoes of the same check are capped at 5 per file (configurable via `echo_cap_per_check`). Overflow is summarized as "... and N more". Prevents avalanches from noisy checks like `var-declarations` in legacy JS codebases.
- **Biome check rename** — `empty-error-handlers` renamed to `empty-block-statements` to accurately reflect the check's scope (all empty blocks, not just catch). **Breaking:** users with `empty-error-handlers` in `disabled_checks` must update to `empty-block-statements`.
- **Unreachable yield-after-raise** — `yield` after `raise` in generators and `@contextmanager` functions is no longer flagged as unreachable code. These patterns establish the generator protocol and are intentional.
- **Vulture dunder filter** — unused variables/arguments starting with `__` (e.g., `__n`, `__limit`) and unused methods starting with `__` (e.g., `__get__`, `__set__`) are now filtered. Covers protocol interface parameters and descriptor methods.
- **Vulture descriptor params** — `objtype`, `owner` (descriptor `__get__`), and `sender` (signal handlers) added to the always-skip list.
- **Vulture dynamic fixture collection** — scans `conftest.py` files for `@pytest.fixture` decorated functions and adds their names to the test-file skip list. Eliminates FPs on project-specific fixtures without manual configuration.
- **Vulture yield-after-raise filter** — vulture's own "unreachable code after 'raise'" detection now skips yield statements in generators/async generators (same pattern as the custom check, applied to vulture output).
- **Encoding fix** — `unreachable_code.py` now uses `encoding="utf-8"` for file reads (CLAUDE.md requirement).

### Architecture enforcement

- **Import layer rules** — new `import_rules` config enforces architectural boundaries. Each rule specifies which files (by glob) are denied from importing which modules. Separator-aware prefix matching prevents false positives (`repositories.user` matches deny `repositories`, but `my_repositories` does not). Supports Python (AST-based) and JS/TS (regex-based).

### Tune command

- `/ecko:tune` now recommends `import_rules` (scans directory structure for layer patterns), `builtin_shadow_allowlist` (checks for A001/A002 hits), and `echo_cap_per_check` (based on project type).

### YAML parser

- Nested lists in list-of-dict config blocks are now supported (required for `import_rules` with `deny_import` sub-lists).

### Tests

New test files: `test_noise_reduction.py`, `test_fixture_collection.py`, `test_import_layers.py`. New fixture: `route_bad_import.py`.

## v0.4.0

CodeLeash-inspired workflow guardrails — deterministic constraints that catch mistakes before the developer sees them.

### Test quality checks

Three new AST-based checks for Python test files (`test_*.py`, `*_test.py`, `conftest.py`):

- **`test-conditional`** — flags `if`/`else` inside test functions. Tests should control state, not branch on it. Guard clauses (platform checks, `pytest.skip`, early return) are automatically excluded.
- **`fixed-wait`** — flags `time.sleep()`, `asyncio.sleep()`, and `wait_for_timeout()` in tests. Fixed waits are flaky — use polling or event-based assertions. `sleep(0)` (idiomatic yield) is excluded.
- **`mock-spec-bypass`** — flags attribute assignment on `Mock(spec=...)` / `MagicMock(spec=...)` objects that bypasses spec validation. Standard mock attributes (`return_value`, `side_effect`, etc.) are excluded.

### Bash command blocking

New PreToolUse hook blocks dangerous bash commands before execution (exit code 2 = block):

- **Built-in blocks** (always active): `git commit --no-verify`, `rm -rf /`, `rm -rf ~`
- **User-configurable** via `blocked_commands` in `ecko.yaml` — add project-specific patterns

### Plan-mode awareness

ExitPlanMode hook reminds the agent to include test steps for all code changes in the plan.

### Reverb nudge

When the stop hook finds echoes (something went wrong), it nudges the agent to leave a reverb note at `.ecko-reverb/`. Opt-in via `reverb.enabled: true` in `ecko.yaml`.

### `/ecko:tune` command

Analyzes `.ecko-reverb/` notes and codebase patterns, then recommends specific `ecko.yaml` rules: banned patterns, obsolete terms, blocked commands, and CLAUDE.md improvements.

### Other improvements

- **`.pyi` exclusion** — type stubs are skipped from all linting (they exist for type checkers, not runtime)
- **UTF-8 encoding** — `config.py` and `runner.py` now explicitly use `encoding="utf-8"` for all file reads (prevents cp1252 failures on Windows)

### Tests

167 total (54 new) — covering test quality checks, bash guard blocking, and edge cases. Validated across 42 open-source repos.

## v0.3.1

### Bug fixes

- **Fix Windows CI failures** — 5 test failures on `windows-latest` resolved:
  - `TestNormalizePath` (4 tests): Wrapped expected values in `os.path.normpath()` so assertions use platform-correct path separators
  - `TestUnicodeArtifacts::test_js_mixed_strings_and_code` (1 test): Fixed `open()` call to use `encoding="utf-8"` — on Windows Python 3.10/3.12, the default cp1252 encoding cannot decode byte `0x9d` from UTF-8 smart quotes, causing a silent `UnicodeDecodeError`
- **Fix CRLF line offset calculation** in `_scan_js_skip_regions` — replaced LF-only offset loop with one that scans raw source for `\n`, correct for both LF and CRLF line endings

## v0.3.0

Three noise-reduction filters that eliminate the most common false positives across all repos.

### Skip unicode-artifact check on prose files

Em dashes, smart quotes, and other Unicode punctuation are normal in markdown and documentation. The `unicode-artifact` check now skips `.md`, `.txt`, `.rst`, `.adoc`, and `.rdoc` files entirely. A new hash-comment-aware scanner also correctly handles `#`-style comments in shell scripts, YAML, and TOML — artifacts inside comments are no longer flagged.

### Filter pyright unresolved import errors

When dependencies aren't installed (the common case for code review), pyright floods output with `Import "X" could not be resolved` errors. These are now filtered — real type errors (attribute access, type mismatches, unknown symbols) still come through.

### Filter vulture framework-injected parameters

Protocol parameters (`exc_type`, `exc_val`, `exc_tb` from `__exit__`; `signum`, `frame` from signal handlers) are filtered everywhere. Pytest built-in fixtures (`tmp_path`, `capsys`, `monkeypatch`, etc.) are filtered only in `test_*`, `*_test.py`, and `conftest.py` files — the same names in non-test code are still flagged. Fixture definitions in conftest files (`unused function 'fixture_name'`) are also handled.

## v0.2.0

### Bug fixes

- **Fix suppression leak** — `ecko:ignore` on line N no longer silently suppresses line N+1. Inline ignores (`import os  # ecko:ignore`) now only apply to their own line. Standalone comment ignores (`# ecko:ignore` on its own line) still correctly suppress the line below.
- **Fix stop hook path duplication** — Layer 3 tools returned relative paths while Layer 2 used absolute paths, causing the same file to appear twice in the final sweep output. All paths are now normalized before merging.
- **Fix Layer 3 suppression bypass** — `ecko:ignore` comments previously only worked on Layer 2 echoes. Suppression now applies uniformly across all layers.
- **Fix banned_patterns glob matching basename only** — `glob: "src/*.tsx"` silently matched nothing. Globs now match against both the file basename and the path relative to the project root.
- **Fix unicode false positive on Python 3.12+ f-strings** — The Python 3.12 tokenizer emits `FSTRING_START`/`FSTRING_MIDDLE`/`FSTRING_END` instead of a single `STRING` token, causing unicode artifacts inside f-string literals to be incorrectly flagged.

### Tests

30 new tests (83 → 113): suppression leak scenarios, standalone comment detection, stop mode, autofix, banned_patterns relative path globs, path normalization, f-string unicode skip.

## v0.1.2

### Fixes

- **Unicode artifact false positives** — JS/TS/CSS/JSON files no longer flag unicode inside string literals, template literals, or comments. Replaced the naive line-comment heuristic with a proper state-machine scanner that tracks `//`, `/* */`, `"`, `'`, and `` ` `` regions with column-level precision.
- **Unterminated block comments** — an unclosed `/*` no longer silently suppresses all unicode checks for the rest of the file.

### New

- **Path exclusions** — `fixtures`, `__fixtures__`, `__snapshots__`, `vendor`, `node_modules`, `.git`, `dist`, `build`, and `__pycache__` directories are now automatically excluded from all checks at any depth.
- **User-configurable `exclude`** — add custom glob patterns in `ecko.yaml` to skip project-specific paths:
  ```yaml
  exclude:
    - "generated/*"
    - "*.min.js"
  ```

### Tests

83 total (22 new) — covering the unicode scanner, path exclusions, and integration tests.

## v0.1.1

### Zero-install tool resolution

Tools now auto-resolve via `uvx` (Python) and `npx` (Node) — no global installs needed. If you already have tools installed locally, ecko uses those first.

Resolution order: PATH → `uvx`/`pipx run` → `npx`/`pnpx`

### Biome v2 support

Updated biome config and adapter for biome v2 schema. Only ecko's configured rules are reported.

### Cross-platform install scripts

```bash
# Linux/macOS
curl -fsSL https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.ps1 | iex
```

### Test suite & CI

61 tests covering config parser, result formatter, tool resolver, custom checks, and full integration. CI runs on `{ubuntu, macos, windows} × {Python 3.10, 3.12}`.

## v0.1.0

Initial release — deterministic code quality checks for AI agents. Echoes back mistakes at write-time so the agent self-corrects before the developer ever sees the code.

### Three-layer architecture

- **Layer 1 (Silent auto-fix):** black, isort, prettier, trailing whitespace removal
- **Layer 2 (Echoes):** ruff (9 rules), biome (7 rules), plus custom AST checks for duplicate dict keys, unreachable code, unicode artifacts, and banned patterns
- **Layer 3 (Deep analysis):** tsc, pyright, vulture, knip + Layer 2 re-sweep on all modified files

### Slash commands

- `/ecko:ping [file]` — manually trigger checks on a file
- `/ecko:status` — show installed tools and config
- `/ecko:setup` — install missing tools

### Key design decisions

- Zero Python dependencies — minimal YAML subset parser, no PyYAML needed
- All external tools optional — gracefully skips anything not installed
- Inline suppression — `# ecko:ignore` or `# ecko:ignore[check-name]`
- Project config — `ecko.yaml` for banned patterns, obsolete terms, disabling checks
