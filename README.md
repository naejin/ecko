# ecko

[![v0.7.0](https://img.shields.io/badge/version-0.7.0-blue)](https://github.com/naejin/ecko/releases/tag/v0.7.0)
[![Claude Code Plugin](https://img.shields.io/badge/Claude_Code-plugin-7c3aed)](https://docs.anthropic.com/en/docs/claude-code)
[![Python](https://img.shields.io/badge/python-3.10+-3776ab?logo=python&logoColor=white)](https://python.org)
[![TypeScript](https://img.shields.io/badge/typescript-supported-3178c6?logo=typescript&logoColor=white)](https://typescriptlang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

**Deterministic code quality checks for AI agents.**

Ecko echoes mistakes back to the agent at write-time so it self-corrects before you ever see the code. Three layers: silent auto-fix, per-file echoes, and deep analysis on stop.

```
~~ ecko ~~  3 echoes in src/auth/handler.py

  1. unused-imports (line 3)
     `import hashlib` is imported but never used.
     Remove it.

  2. bare-except (line 45)
     Bare `except:` catches everything including KeyboardInterrupt.
     Specify an exception type.

  3. unicode-artifact (line 12)
     Em dash (—) found in source code. Likely from copy-pasting LLM output.
     Replace with -- or a regular hyphen.
```

Clean code = silence. Problems = echoes.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.sh | bash
```

<details>
<summary>Windows (PowerShell)</summary>

```powershell
irm https://raw.githubusercontent.com/naejin/ecko/main/scripts/install.ps1 | iex
```
</details>

<details>
<summary>Manual install</summary>

```bash
claude plugin marketplace add naejin/monet-plugins
claude plugin install ecko
```
</details>

Restart your Claude Code session for the hooks to take effect.

**No tool installation needed.** Ecko auto-runs tools via [`uvx`](https://docs.astral.sh/uv) (Python tools) and [`npx`](https://docs.npmjs.com/cli/commands/npx) (Node tools) — no global installs, no environment pollution. If you already have tools installed locally, ecko uses those first.

## How It Works

Ecko hooks into four moments in a Claude Code session:

**Before every Bash command** — dangerous commands are blocked before execution:

```
┌─────────────────────────────────────────────┐
│  Bash guard (PreToolUse)                    │
│  Blocks: --no-verify, rm -rf /, rm -rf ~,  │
│  git push --force, git reset --hard,        │
│  git clean -f + user blocked_commands.      │
│  Agent never executes the command.          │
└─────────────────────────────────────────────┘
```

**After every Write/Edit** — your file gets cleaned up and checked:

```
┌─────────────────────────────────────────────┐
│  Layer 1: Auto-fix (silent)                 │
│  black → isort → prettier → strip trailing  │
│  whitespace. Modifies file. No output.      │
├─────────────────────────────────────────────┤
│  Layer 2: Echoes (per-file)                 │
│  ruff · biome · duplicate keys ·            │
│  unreachable code · unicode artifacts ·     │
│  placeholder code · banned patterns ·       │
│  test quality.                              │
│  Reports to agent.                          │
└─────────────────────────────────────────────┘
```

**When the agent exits plan mode** — a nudge to include test steps:

```
┌─────────────────────────────────────────────┐
│  Plan check (PreToolUse)                    │
│  Reminds agent to include test steps        │
│  for all code changes in the plan.          │
└─────────────────────────────────────────────┘
```

**When the agent tries to stop** — a final sweep catches what per-file checks can't:

```
┌─────────────────────────────────────────────┐
│  Layer 3: Deep analysis                     │
│  tsc --noEmit · pyright · vulture · knip    │
│  + Layer 2 re-sweep on all modified files.  │
│  Blocks agent until issues are fixed.       │
└─────────────────────────────────────────────┘
```

### When Does Each Layer Run?

| Layer | Trigger | Scope | Speed |
|-------|---------|-------|-------|
| Bash guard | Before every Bash command | Single command | Instant |
| Layer 1 (auto-fix) | After every Write/Edit | Single file | <1s |
| Layer 2 (echoes) | After every Write/Edit | Single file | <2s |
| Plan check | When agent exits plan mode | Plan content | Instant |
| Layer 3 (deep analysis) | When agent tries to stop | All modified files | 3-15s |

## Checks

### Layer 2 — Tool Checks

| Check | Tool | Language | What it catches |
|-------|------|----------|-----------------|
| `unused-imports` | ruff / biome | py / ts | Unused imports |
| `singleton-comparison` | ruff | py | `== None` instead of `is None` |
| `bare-except` | ruff | py | Bare `except:` |
| `star-imports` | ruff | py | `from x import *` |
| `mutable-default-args` | ruff | py | `def f(x=[])` |
| `builtin-shadowing` | ruff | py | Variable shadows builtin (filtered by allowlist) |
| `empty-error-handlers` | ruff | py | `except: pass` |
| `empty-block-statements` | biome | ts | Empty `{}` blocks |
| `unreachable-code` | biome | ts | Code after return/throw |
| `debugger-statements` | biome | ts | `debugger` |
| `var-declarations` | biome | ts | `var` usage |
| `duplicate-keys` | biome | ts | `{a: 1, a: 2}` |
| `useless-catch` | biome | ts | `catch(e) { throw e }` |

### Layer 2 — Custom Checks (no dependencies)

| Check | Language | What it catches |
|-------|----------|-----------------|
| `duplicate-keys` | py | Duplicate `dict` keys via AST |
| `unreachable-code` | py | Statements after `return`/`raise`/`break`/`continue` |
| `unicode-artifact` | all | Em dashes, smart quotes, zero-width chars from LLM output |
| `banned-pattern` | all | Custom regex patterns from `ecko.yaml` |
| `obsolete-term` | all | Old names that should be renamed |
| `import-layer` | py / ts | Import boundary violations from `import_rules` config |
| `placeholder-code` | py / ts | `pass`/`...`/`raise NotImplementedError` sole-body functions; JS `throw new Error("not implemented")` |

### Layer 2 — Test Quality (Python test files only)

| Check | What it catches |
|-------|-----------------|
| `test-conditional` | `if`/`else` inside test functions — tests should not branch |
| `fixed-wait` | `time.sleep` / `asyncio.sleep` / `wait_for_timeout` — use polling instead |
| `mock-spec-bypass` | Setting attributes on `Mock(spec=...)` objects — bypasses spec validation |

### Layer 3 — Deep Analysis

| Check | Tool | What it catches |
|-------|------|-----------------|
| `type-error` | tsc / pyright | Type errors across the project |
| `dead-code` | vulture | Unused functions, classes, variables (80% confidence) |
| `unused-export` | knip | Unused exports, imports, dependencies |

## Reverb → Tune

Enable reverb to capture session insights when echoes are found:

```yaml
reverb:
  enabled: true
```

When the stop hook fires with echoes, it tips you to run `/ecko:reverb`. That command captures a structured note at `.ecko-reverb/`. Then `/ecko:tune` reads those notes alongside codebase patterns and recommends `ecko.yaml` rules — closing the feedback loop.

## Commands

| Command | Description |
|---------|-------------|
| `/ecko:ping [file]` | Run checks on a file manually |
| `/ecko:status` | Show installed tools and config |
| `/ecko:setup` | Install missing tools interactively |
| `/ecko:reverb` | Capture a session note about what went wrong |
| `/ecko:tune` | Analyze reverb notes and codebase, recommend ecko.yaml rules |

## Configuration

Create `ecko.yaml` in your project root. Everything is optional.

```yaml
# Disable specific auto-fixers
autofix:
  black: false

# Disable specific deep analysis tools
deep_analysis:
  vulture: false

# Flag patterns that shouldn't appear
banned_patterns:
  - pattern: "bg-(blue|red|green)-\\d+"
    glob: "*.tsx"
    message: "Use brand color utilities instead of raw Tailwind colors"

# Flag old names that should be renamed
obsolete_terms:
  - old: "UserProfile"
    new: "Account"

# Enforce architecture boundaries
import_rules:
  - files: "routes/*.py"
    deny_import:
      - repositories
      - sqlalchemy
    message: "Routes must not import from the data layer"

# Block dangerous bash commands (in addition to built-in blocks)
# Built-in: --no-verify, rm -rf /, rm -rf ~, git push --force,
#           git reset --hard, git clean -f
blocked_commands:
  - pattern: "(pytest|npm test).*\\|"
    message: "Do not pipe test output — run tests directly"

# Cap repeated echoes per check per file (default: 5, 0 = unlimited)
echo_cap_per_check: 5

# Nudge agent to leave reverb notes when echoes are found on stop
reverb:
  enabled: true

# Disable specific checks entirely
disabled_checks:
  - builtin-shadowing
```

See [`ecko.yaml.example`](ecko.yaml.example) for the full reference.

## Inline Suppression

```python
import os  # ecko:ignore

x = None
if x == None:  # ecko:ignore[singleton-comparison]
    pass
```

- `# ecko:ignore` — suppress all checks on this line
- `# ecko:ignore[check-name,other-check]` — suppress specific checks
- Works with `//` comments too (TypeScript/JavaScript)
- Place on the same line or the line above

## Tools

| Tool | Layer | Resolved via |
|------|-------|-------------|
| [black](https://github.com/psf/black) | auto-fix | `uvx` / `pipx run` / PATH |
| [isort](https://github.com/PyCQA/isort) | auto-fix | `uvx` / `pipx run` / PATH |
| [prettier](https://github.com/prettier/prettier) | auto-fix | `npx` / PATH |
| [ruff](https://github.com/astral-sh/ruff) | echoes | `uvx` / `pipx run` / PATH |
| [biome](https://github.com/biomejs/biome) | echoes | `npx` / PATH |
| [tsc](https://github.com/microsoft/TypeScript) | deep | `npx` / PATH |
| [pyright](https://github.com/microsoft/pyright) | deep | `uvx` / `pipx run` / PATH |
| [vulture](https://github.com/jendrikseipp/vulture) | deep | `uvx` / `pipx run` / PATH |
| [knip](https://github.com/webpro-nl/knip) | deep | `npx` / PATH |

All tools are resolved automatically — no manual installation required. Ecko checks PATH first (uses your local install if present), then falls back to `uvx`/`npx` which download and cache tools on demand.

When tools are unavailable, ecko reports what was skipped with install hints:
```
~~ ecko ~~ note: ruff not found — try: pip install ruff (or uvx ruff)
~~ ecko ~~ note: vulture not found — try: pip install vulture (or uvx vulture)
```

## Troubleshooting

**Ecko runs but reports nothing** — On stop, ecko now emits `~~ ecko ~~ clean sweep — 0 echoes across N files` when all checks pass, so you can tell it ran. Check if your tools are installed: run `/ecko:status`. For deeper visibility, set `ECKO_DEBUG=1` to see config loading, tool resolution, file detection, and timing. Ecko reports skipped tools with install hints (e.g., `ruff not found — try: pip install ruff`). If a tool times out or crashes, ecko emits a warning instead of silently returning nothing.

**Config changes aren't taking effect** — Verify your `ecko.yaml` is in the project root (same directory as `.git`). Ecko validates config and warns about unknown keys:
```
~~ ecko ~~ warning: unknown config key 'disabled_check' (did you mean 'disabled_checks'?)
```

**A check is too noisy** — Disable it in `ecko.yaml`: `disabled_checks: [check-name]`. Or suppress per-line with `# ecko:ignore[check-name]`. You can also reduce repeated echoes with `echo_cap_per_check`.

**Layer 3 is slow** — Deep analysis tools (tsc, pyright, vulture, knip) run in parallel on stop. To disable a slow tool: `deep_analysis: { vulture: false }`.

## License

MIT
