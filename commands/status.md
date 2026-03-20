---
description: "Show ecko status — available tools, config, and check list"
allowed-tools: ["Bash", "Read"]
---

Show the current ecko configuration and which tools are available.

Steps:
1. Check which tools are installed by running `which` for each:
   - Layer 1: `black`, `isort`, `prettier`
   - Layer 2: `ruff`, `biome`
   - Layer 3: `tsc`, `pyright`, `vulture`
   - Also check: `npx` (for knip)

2. Check if `ecko.yaml` exists in the current directory. If so, read it and summarize:
   - Autofix settings
   - Deep analysis settings
   - Number of banned patterns
   - Number of obsolete terms
   - Disabled checks

3. Show the **effective config** including defaults, so users know what's active:

```
~~ ecko status ~~

Layer 1 (auto-fix):
  black       ✓ installed
  isort       ✓ installed
  prettier    ✗ not found

Layer 2 (echoes):
  ruff        ✓ installed
  biome       ✗ not found

Layer 3 (deep analysis):
  tsc         ✓ installed
  pyright     ✓ installed
  vulture     ✗ not found
  knip (npx)  ✓ available

Config: ecko.yaml found
  Autofix: enabled
  Disabled checks: none
  Banned patterns: 2
  Obsolete terms: 1

Effective config:
  echo_cap_per_check: 5 (default)
  builtin_shadow_allowlist: [type, help, input, ...] (20 items, default)
  disabled_checks: [] (none)
  exclude: [fixtures/*, node_modules/*] + built-in defaults
  import_rules: 0 rules
  blocked_commands: 0 user patterns (+ 6 built-in)
  reverb: disabled
```

Missing tools are fine — ecko gracefully skips them. Suggest `/ecko:setup` if many tools are missing.
