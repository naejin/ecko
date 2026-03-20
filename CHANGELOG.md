# Changelog

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

### Learnings nudge

When the stop hook finds echoes (something went wrong), it nudges the agent to write a brief learnings file at `.ecko-learnings/`. Opt-in via `learnings.enabled: true` in `ecko.yaml`.

### `/ecko:tune` command

Analyzes `.ecko-learnings/` files and codebase patterns, then recommends specific `ecko.yaml` rules: banned patterns, obsolete terms, blocked commands, and CLAUDE.md improvements.

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
