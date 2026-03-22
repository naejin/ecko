---
description: "Show ecko status -- available checks, config, and language support"
allowed-tools: ["Bash", "Read"]
---

Show the current ecko configuration and which checks are available.

Steps:
1. Run ecko dry-run to list available checks:
   ```bash
   ${CLAUDE_PLUGIN_ROOT}/scripts/run.sh --mode dry-run --file dummy.py --cwd $(pwd) --plugin-root ${CLAUDE_PLUGIN_ROOT} 2>&1
   ```

2. Check which optional external deep analysis tools are installed:
   - `pyright` (Python type checking)
   - `tsc` (TypeScript type checking)
   - `golangci-lint` (Go linting)
   - `clippy` (Rust linting)

3. Check if `ecko.yaml` exists in the current directory. If so, read it and summarize:
   - Disabled checks
   - Banned patterns count
   - Obsolete terms count
   - Import rules count
   - Blocked commands count
   - Custom checks count
   - Session hours
   - Output format

4. Show the **effective config** including defaults:

```
~~ ecko status ~~

Version: 2.0.0
Core checks: 28 native (tree-sitter)
Languages: Python, JavaScript, TypeScript, Go, Rust

External adapters (optional):
  pyright     [installed/not found]
  tsc         [installed/not found]
  golangci    [installed/not found]
  clippy      [installed/not found]

Config: ecko.yaml [found/not found]
  disabled_checks: []
  banned_patterns: 0
  echo_cap_per_check: 5 (default)
```

Missing external tools are fine -- ecko's 28 core checks are native and always available. Suggest `/ecko:setup` if the user wants to install external adapters.
