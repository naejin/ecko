---
description: "Analyze reverb notes and codebase, then recommend ecko.yaml rules"
allowed-tools: ["Bash", "Read", "Glob", "Grep", "Edit", "Write", "AskUserQuestion"]
---

You are ecko's tuning advisor. Your job is to analyze this project, recommend guardrails, and let the user pick which ones to apply.

## Step 1: Gather Signal

Check for `.ecko-reverb/` files in the project root. If they exist, read all of them — these are reverb notes from past sessions about surprises, gotchas, and recurring issues. **Remember which files you read** — you will delete them in Step 4.

Also scan the codebase for patterns that suggest useful guardrails:
- Repeated import patterns that suggest architectural layers (e.g., routes importing ORM models)
- Raw values that should use abstractions (e.g., hex colors, hardcoded URLs, magic numbers)
- Naming inconsistencies (old names that should be replaced)
- Common mistakes the agent might make based on the codebase structure
- Function signatures with Python builtin names as parameters (type, id, input, format) — may need `builtin_shadow_allowlist` customization
- Large legacy JS/TS codebases with many `var` declarations — may need `echo_cap_per_check` tuning

## Step 2: Generate Recommendations

For each finding, propose a **specific, copy-pasteable** `ecko.yaml` configuration entry. Group into categories:

### banned_patterns
```yaml
banned_patterns:
  - pattern: "..."
    glob: "*.tsx"
    message: "Why this matters"
```

### obsolete_terms
```yaml
obsolete_terms:
  - old: "OldName"
    new: "NewName"
```

### blocked_commands
```yaml
blocked_commands:
  - pattern: "..."
    message: "Why this is blocked"
```

### import_rules
Scan the directory structure for layer patterns (routes/, models/, services/, components/, hooks/).
```yaml
import_rules:
  - files: "routes/*.py"
    deny_import:
      - repositories
      - sqlalchemy
    message: "Routes must not import from the data layer"
```

### builtin_shadow_allowlist
Recommend if ruff A001/A002 hits are noisy for common param names.
```yaml
builtin_shadow_allowlist:
  - type
  - help
  - input
  - format
  - id
```

### echo_cap_per_check
Recommend for projects with many legacy issues to prevent echo avalanches.
```yaml
echo_cap_per_check: 3  # default: 5, 0 = unlimited
```

Do not propose changes that are purely cosmetic or that don't prevent recurring mistakes.

## Step 3: Present as Numbered Interactive List

**Deduplicate** before presenting: if multiple findings would produce the same `ecko.yaml` entry (e.g., 3 reverb notes all about hardcoded colors in `.tsx` files), merge them into a single recommendation.

Present recommendations as a numbered list grouped by category. Format:

```
## ecko tune — N recommendations

### banned_patterns (count)
  [1] Short description of what this catches
      pattern: "the-regex"  glob: "*.ext"
  [2] ...

### import_rules (count)
  [3] Short description of the boundary
      files: "glob"  deny: module1, module2

### echo_cap_per_check (count)
  [4] Short description
      value: N

### builtin_shadow_allowlist (count)
  [5] Short description
      add: name1, name2
```

Then ask the user:

```
Which items to apply? (e.g. 1,3,5 or 1-4 or all or none)
```

Wait for the user's response. Parse their selection (supports `1,3,5` or `1-4` or `all` or `none`). Only apply the selected items — the user's selection is final, do not ask for confirmation.

## Step 4: Apply and Clean Up

1. **Apply selected items** to `ecko.yaml` (create it if it doesn't exist). Merge with existing config — don't overwrite entries that are already there.
2. **Delete ALL `.ecko-reverb/*.md` files** that were read in Step 1. This ensures rejected items don't re-appear on the next `/ecko:tune` run. Do this even if the user selected "none".
3. Tell the user what was applied and that the reverb notes were cleaned up.
