# ecko

[![v2.1.0](https://img.shields.io/badge/version-2.0.0-blue)](https://github.com/naejin/ecko/releases/tag/v2.1.0)
[![Claude Code Plugin](https://img.shields.io/badge/Claude_Code-plugin-7c3aed)](https://docs.anthropic.com/en/docs/claude-code)
[![Rust](https://img.shields.io/badge/rust-native-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org)
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
     Em dash found in source code. Likely from copy-pasting LLM output.
     Replace with -- or a regular hyphen.
```

Clean code = silence. Problems = echoes.

## Install

```bash
curl -fsSL https://github.com/naejin/ecko/releases/latest/download/install.sh | bash
```

No Python or Node.js required -- ecko is a single native binary.

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

**Before every Bash command** -- dangerous commands are blocked before execution:

```
+---------------------------------------------+
|  Bash guard (PreToolUse)                    |
|  Blocks: --no-verify, rm -rf /, rm -rf ~,  |
|  git push --force, git reset --hard,        |
|  git clean -f (including git -C variants)   |
|  + user blocked_commands.                   |
|  Agent never executes the command.          |
+---------------------------------------------+
```

**After every Write/Edit** -- your file gets cleaned up and checked:

```
+---------------------------------------------+
|  Layer 1: Auto-fix (silent)                 |
|  black -> isort -> prettier -> strip        |
|  trailing whitespace. Modifies file.        |
|  No output.                                 |
+---------------------------------------------+
|  Layer 2: Echoes (per-file)                 |
|  28 native tree-sitter checks:              |
|  unused imports, singleton comparison,      |
|  bare except, duplicate keys, unreachable   |
|  code, unicode artifacts, placeholder       |
|  code, banned patterns, and more.           |
|  Reports to agent.                          |
+---------------------------------------------+
```

**When the agent exits plan mode** -- a nudge to include test steps:

```
+---------------------------------------------+
|  Plan check (PreToolUse)                    |
|  Reminds agent to include test steps        |
|  for all code changes in the plan.          |
+---------------------------------------------+
```

**When the agent tries to stop** -- a final sweep catches what per-file checks can't:

```
+---------------------------------------------+
|  Layer 3: Deep analysis                     |
|  Dead-code analysis + Layer 2 re-sweep on   |
|  all modified files. Optional external      |
|  adapters: pyright, tsc, golangci-lint,     |
|  clippy. Blocks agent until issues fixed.   |
+---------------------------------------------+
```

### When Does Each Layer Run?

| Layer | Trigger | Scope | Speed |
|-------|---------|-------|-------|
| Bash guard | Before every Bash command | Single command | Instant |
| Layer 1 (auto-fix) | After every Write/Edit | Single file | <1s |
| Layer 2 (echoes) | After every Write/Edit | Single file | <1s |
| Plan check | When agent exits plan mode | Plan content | Instant |
| Layer 3 (deep analysis) | When agent tries to stop | All modified files | 2-10s |

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
| `test-conditional` | py | `if`/`else` inside test functions -- tests should not branch |
| `fixed-wait` | py | `time.sleep` / `asyncio.sleep` -- use polling instead |
| `mock-spec-bypass` | py | Setting attributes on `Mock(spec=...)` -- bypasses spec validation |
| `debugger-statement` | js / ts | `debugger` left in code |
| `no-var` | js / ts | `var` usage (use `const`/`let`) |
| `empty-block-statements` | js / ts | Empty `{}` blocks |
| `useless-catch` | js / ts | `catch(e) { throw e }` |
| `empty-error-check` | go | `if err != nil {}` with empty body |
| `todo-macro` | rs | `todo!()` / `unimplemented!()` -- panics at runtime |

### Universal (all languages)

| Check | What it catches |
|-------|-----------------|
| `unicode-artifacts` | Em dashes, smart quotes, zero-width chars from LLM output |
| `banned-patterns` | Custom regex patterns from `ecko.yaml` |
| `import-layers` | Import boundary violations from `import_rules` config |

### Layer 3 -- Deep Analysis

| Check | What it catches |
|-------|-----------------|
| `dead-code` | Unused functions, classes, variables (native analysis) |
| `unused-exports` | Exported symbols never imported by other files (JS/TS) |

### External Adapters (optional)

Core checks are native -- no external dependencies required. For deeper project-wide analysis, ecko can optionally run these external tools:

| Tool | Language | Install |
|------|----------|---------|
| [pyright](https://github.com/microsoft/pyright) | Python | `pip install pyright` |
| [tsc](https://github.com/microsoft/TypeScript) | TypeScript | `npm install -g typescript` |
| [golangci-lint](https://github.com/golangci/golangci-lint) | Go | `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest` |
| [clippy](https://github.com/rust-lang/rust-clippy) | Rust | `rustup component add clippy` |

External adapters run during Layer 3 (stop hook) only. When a tool is not installed, ecko skips it silently -- core checks always run.

## MCP Server

Ecko includes a built-in MCP server, allowing other tools and agents to invoke checks programmatically.

| Tool | Description |
|------|-------------|
| `ecko_check_file` | Run checks on a single file and return echoes with fix suggestions |
| `ecko_check_workspace` | Run checks on all modified files in the workspace |
| `ecko_status` | Show configuration, available checks, and language support |
| `ecko_dry_run` | List which checks would run on a file without executing them |
| `ecko_explain` | Explain what a specific check does and why it matters |

The MCP server runs over stdio transport. It is configured automatically when ecko is installed as a Claude Code plugin.

## Reverb -> Tune

Enable reverb to capture session insights when echoes are found:

```yaml
reverb:
  enabled: true
```

When the stop hook fires with echoes, it tips you to run `/ecko:reverb`. That command captures a structured note at `.ecko-reverb/`. Then `/ecko:tune` reads those notes alongside codebase patterns and recommends `ecko.yaml` rules -- closing the feedback loop.

## Commands

| Command | Description |
|---------|-------------|
| `/ecko:ping [file]` | Run checks on a file manually |
| `/ecko:status` | Show installed tools and config |
| `/ecko:setup` | Install missing tools interactively |
| `/ecko:reverb` | Capture a session note about what went wrong |
| `/ecko:tune` | Analyze reverb notes and codebase, recommend ecko.yaml rules |
| `/ecko:session` | Show session echo summary -- files, top checks, self-corrections |

## Configuration

Create `ecko.yaml` in your project root. Everything is optional.

```yaml
# Disable specific auto-fixers
autofix:
  black: false

# Disable specific deep analysis tools
deep_analysis:
  pyright: false

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
# Built-in: --no-verify, rm -rf /, rm -rf ~, git push --force,
#           git reset --hard, git clean -f
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
    query: '(macro_invocation macro: (identifier) @name (#eq? @name "println"))'
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
- Works with `//` comments too (TypeScript/JavaScript)
- Place on the same line or the line above

## Troubleshooting

**Ecko runs but reports nothing** -- On stop, ecko emits `~~ ecko ~~ clean sweep -- 0 echoes across N files` when all checks pass, so you can tell it ran. Check your config: run `/ecko:status`. For deeper visibility, set `ECKO_DEBUG=1` to see config loading, file detection, and timing.

**Ecko binary not found** -- The install script places the ecko binary inside the plugin directory. If hooks fail with "command not found", re-run the install script. Verify the binary exists: `ls ~/.claude/plugins/ecko/ecko` (or the equivalent path on your system). You can also run `ecko --help` to confirm it is on your PATH.

**Config changes aren't taking effect** -- Verify your `ecko.yaml` is in the project root (same directory as `.git`). Ecko validates config and warns about unknown keys:
```
~~ ecko ~~ warning: failed to parse ecko.yaml: unknown field `disabled_check`
```

**A check is too noisy** -- Disable it in `ecko.yaml`: `disabled_checks: [check-name]`. Or suppress per-line with `# ecko:ignore[check-name]`. You can also reduce repeated echoes with `echo_cap_per_check`.

**Layer 3 is slow** -- Deep analysis runs in parallel on stop. To disable a slow external adapter: `deep_analysis: { pyright: false }`. Core checks are native and typically complete in under 2 seconds.

## License

MIT
