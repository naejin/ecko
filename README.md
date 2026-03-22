# ecko

[![v2.2.0](https://img.shields.io/badge/version-2.2.0-blue)](https://github.com/naejin/ecko/releases/tag/v2.2.0)
[![Claude Code Plugin](https://img.shields.io/badge/Claude_Code-plugin-7c3aed)](https://docs.anthropic.com/en/docs/claude-code)
[![Rust](https://img.shields.io/badge/rust-native-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

**Deterministic code quality checks for AI agents.**

Ecko echoes mistakes back to the agent at write-time so it self-corrects before you ever see the code.

```
~~ ecko ~~ src/auth/handler.py -- unused-imports (L3), [error] bare-except (L45), unicode-artifacts (L12)
```

Clean code = silence. Problems = echoes. The agent fixes them before you review.

## Install

```bash
curl -fsSL https://github.com/naejin/ecko/releases/latest/download/install.sh | bash
```

No Python or Node.js required -- ecko is a single native binary (8MB, <120ms startup).

<details>
<summary>Windows (PowerShell)</summary>

```powershell
irm https://github.com/naejin/ecko/releases/latest/download/install.ps1 | iex
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

## How It Works

Ecko hooks into four moments in a Claude Code session:

| When | What | Speed |
|------|------|-------|
| **Before every Bash command** | Blocks dangerous commands (`git push --force`, `rm -rf /`, `--no-verify`, etc.) + user-configured `blocked_commands` | Instant |
| **After every Write/Edit** | Layer 1: silent auto-fix (trailing whitespace, formatters). Layer 2: 28 native checks via tree-sitter | <120ms |
| **When exiting plan mode** | Nudges agent to include test steps | Instant |
| **When agent tries to stop** | Layer 3: deep analysis across all modified files -- dead code, external adapters (pyright, tsc, clippy, golangci-lint) | 2-10s |

## Checks

All 28 core checks are native -- powered by tree-sitter, with zero external dependencies.

### By Language

| Check | Language | What it catches |
|-------|----------|-----------------|
| `unused-imports` | py / js / ts / go / rs | Unused imports |
| `singleton-comparison` | py | `== None` instead of `is None` |
| `bare-except` | py | Bare `except:` |
| `star-imports` | py | `from x import *` |
| `mutable-default-args` | py | `def f(x=[])` |
| `builtin-shadowing` | py | Variable shadows builtin (filtered by allowlist) |
| `placeholder-code` | py / js / ts / go / rs | `pass`/`...`/`raise NotImplementedError`/`todo!()`/`unimplemented!()` sole-body functions |
| `unreachable-code` | py / js / ts / go / rs | Code after return/raise/break/panic |
| `duplicate-keys` | py / js / ts | Duplicate dict/object keys |
| `test-conditional` | py | `if`/`else` inside test functions |
| `fixed-wait` | py | `time.sleep` / `asyncio.sleep` in tests |
| `mock-spec-bypass` | py | Setting attributes on `Mock(spec=...)` |
| `debugger-statement` | js / ts | `debugger` left in code |
| `no-var` | js / ts | `var` usage (use `const`/`let`) |
| `empty-block-statements` | js / ts | Empty `{}` blocks |
| `useless-catch` | js / ts | `catch(e) { throw e }` |
| `empty-error-check` | go | `if err != nil {}` with empty body |
| `todo-macro` | rs | `todo!()` / `unimplemented!()` |

### Universal (all languages)

| Check | What it catches |
|-------|-----------------|
| `unicode-artifacts` | Em dashes, smart quotes, zero-width chars from LLM output |
| `banned-patterns` | Custom regex patterns from `ecko.yaml` |
| `import-layers` | Import boundary violations from `import_rules` config |

### Layer 3 -- Deep Analysis (Stop hook)

| Check | What it catches |
|-------|-----------------|
| `dead-code` | Unused functions, classes, variables (cross-file) |
| `unused-exports` | Exported symbols never imported (JS/TS) |

### External Adapters (optional)

Core checks need no external tools. For deeper analysis, ecko can optionally run:

| Tool | Language | Install |
|------|----------|---------|
| [pyright](https://github.com/microsoft/pyright) | Python | `pip install pyright` |
| [tsc](https://github.com/microsoft/TypeScript) | TypeScript | `npm install -g typescript` |
| [golangci-lint](https://github.com/golangci/golangci-lint) | Go | `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest` |
| [clippy](https://github.com/rust-lang/rust-clippy) | Rust | `rustup component add clippy` |

External adapters run during Layer 3 only. Missing tools are skipped silently.

## Architecture Guard

Enforce architectural decisions during code execution with temporary guardrails.

```
/ecko:guard components must use hooks for API calls, no direct fetch in tsx files
```

Ecko generates `.ecko-guard.yaml` rules (import boundaries, banned patterns, custom checks) that are enforced on every Write/Edit. Rules live in a separate gitignored file -- never mixed with permanent `ecko.yaml` config.

**Lifecycle management:**
- **Age nudge**: stop hook warns when guard rules are older than 7 days
- **Friction detection**: stop hook warns when guard rules fire on 3+ files in a session (signal they may be stale)
- **Review**: `/ecko:guard --review` shows active rules with usage stats
- **Clear**: `/ecko:guard --clear` removes all guard rules instantly

## MCP Server

Ecko includes a built-in MCP server for programmatic access:

| Tool | Description |
|------|-------------|
| `ecko_check_file` | Check a single file, return echoes with fix suggestions |
| `ecko_check_workspace` | Check all modified files (same logic as stop hook) |
| `ecko_status` | Show config, checks, language support |
| `ecko_dry_run` | List applicable checks without running them |
| `ecko_explain` | Explain what a check does and why |

## Session Tracking

Ecko tracks echoes across your session via an append-only ledger (`.ecko-session/`):

- **Self-correction**: measures whether the agent fixed issues after being told
- **Session stats**: `/ecko:session` shows files touched, top checks, correction rate
- **Ledger scoping**: stop hook only checks files the agent actually touched (prevents flooding on legacy codebases)

## Reverb / Tune

Capture session insights and turn them into permanent rules:

1. Enable reverb: `reverb: { enabled: true }` in `ecko.yaml`
2. When the stop hook fires with echoes, run `/ecko:reverb` to capture a structured note
3. Run `/ecko:tune` to analyze reverb notes + codebase patterns and propose `ecko.yaml` rules

## Commands

| Command | Description |
|---------|-------------|
| `/ecko:ping [file]` | Run checks on a file manually |
| `/ecko:status` | Show installed tools and config |
| `/ecko:setup` | Install missing tools interactively |
| `/ecko:reverb` | Capture a session note about what went wrong |
| `/ecko:tune` | Analyze reverb notes, recommend ecko.yaml rules |
| `/ecko:session` | Show session echo summary |
| `/ecko:guard [desc]` | Generate architecture guardrails from plan context |
| `/ecko:guard --review` | Review active guard rules with usage stats |
| `/ecko:guard --clear` | Remove all guard rules |

## Configuration

Create `ecko.yaml` in your project root. Everything is optional.

```yaml
# Disable specific auto-fixers
autofix:
  black: false

# Enable deep analysis tools
deep_analysis:
  pyright: true

# Enable fix suggestions in echo output (default: true)
fix_suggestions: true

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
    deny:
      - repositories
      - sqlalchemy
    message: "Routes must not import from the data layer"

# Block dangerous bash commands (in addition to built-in blocks)
blocked_commands:
  - pattern: "(pytest|npm test).*\\|"
    message: "Do not pipe test output -- run tests directly"

# Cap repeated echoes per check per file (default: 5, 0 = unlimited)
echo_cap_per_check: 5

# Nudge agent to leave reverb notes when echoes are found on stop
reverb:
  enabled: true

# Custom tree-sitter query checks
custom_checks:
  - name: no-println
    languages: [rust]
    query: '(macro_invocation macro: (identifier) @name (#eq? @name "println")) @match'
    message: "Use tracing instead of println"
    severity: warn

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

- `# ecko:ignore` -- suppress all checks on this line
- `# ecko:ignore[check-name,other-check]` -- suppress specific checks
- Works with `//` comments too (JS/TS/Go/Rust)
- Place on the same line or the line above

## Troubleshooting

**Ecko runs but reports nothing** -- On stop, ecko emits `~~ ecko ~~ clean sweep -- 0 echoes across N files` when all checks pass. Run `/ecko:status` to verify config. Set `ECKO_DEBUG=1` for detailed diagnostics.

**Ecko binary not found** -- Re-run the install script. The binary is placed inside the plugin directory. Verify: `ecko --help`.

**Config changes aren't taking effect** -- Verify `ecko.yaml` is in the project root (same directory as `.git`). Ecko warns about invalid config:
```
~~ ecko ~~ warning: failed to parse ecko.yaml: unknown field `disabled_check`
```

**A check is too noisy** -- Disable it: `disabled_checks: [check-name]`. Or suppress per-line: `# ecko:ignore[check-name]`. Reduce repeated echoes: `echo_cap_per_check: 3`.

**Layer 3 is slow** -- Disable slow external adapters: `deep_analysis: { pyright: false }`. Core checks are native and complete in under 2 seconds.

## License

MIT
