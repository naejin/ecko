# Ecko — Claude Code Plugin

## What this is
A Claude Code plugin providing deterministic code quality checks via hooks.
Three layers: silent auto-fix (Layer 1), per-file echoes (Layer 2), deep analysis on stop (Layer 3).

## Structure
- `.claude-plugin/plugin.json` — plugin manifest
- `hooks/hooks.json` — PostToolUse(Write|Edit) + Stop hook wiring
- `hooks/*.sh` — shell entry points that delegate to `checks/runner.py`
- `checks/` — Python package: runner, config, result, formatter, tools/, custom/
- `commands/` — slash commands (ping, status, setup)
- `config/biome.json` — biome lint config (only ecko's rules enabled)
- `scripts/` — install scripts (bash + powershell)
- `tests/` — pytest suite with fixtures/

## Design constraints
- Zero Python dependencies — config.py has a minimal YAML subset parser (no PyYAML)
- Tools auto-resolve via `checks/tools/resolve.py`: PATH first → `uvx`/`pipx run` (Python) → `npx`/`pnpx` (Node)
- When binary != package (e.g. `tsc` from `typescript`), use `resolve_node_tool("tsc", package="typescript")`
- Hook output goes to stderr (`result.emit()`) — that's how Claude Code reads it
- Exit code 1 = echoes found (agent self-corrects), exit code 0 = clean
- Noise filters live in adapters/custom checks, not in runner.py — filter at the source
- Prose files (.md, .txt, .rst, .adoc, .rdoc) are skipped by unicode-artifact (em dashes are normal punctuation)
- Pyright "could not be resolved" imports are filtered (missing deps, not code defects)
- Vulture framework-injected params (`_ALWAYS_SKIP`) filtered everywhere; pytest fixtures (`_PYTEST_SKIP`) filtered only in test/conftest files

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

## Testing
- Smoke test: `python3 checks/runner.py --file <path> --mode post-tool-use --cwd <dir> --plugin-root .`
- All imports: `python3 -c "from checks.runner import main"`
- Stop mode: `python3 checks/runner.py --file <any> --mode stop --cwd <dir> --plugin-root .`
- Run tests: `python3 -m pytest tests/`
- Use temp files for testing checks (e.g., write a .py with unused imports, run runner, verify output)

## Not part of the plugin
- `docs/ideas/` — internal ideation (gitignored)
- `openspec/`, `.claude/` — dev workflow tooling, not distributed
