---
description: "Generate architectural guardrails from plan context into .ecko-guard.yaml"
allowed-tools: ["Bash", "Read", "Write", "Glob", "Grep", "AskUserQuestion"]
---

Generate or manage architectural guardrails that enforce design decisions during code execution.

Arguments: $ARGUMENTS

## Handling --clear

If arguments contain `--clear`:
1. Delete `.ecko-guard.yaml` if it exists
2. Confirm deletion to user: "Guard rules cleared."
3. Done -- no further steps

## Handling --review

If arguments contain `--review`:
1. Read `.ecko-guard.yaml`. If not found, tell user: "No guard rules active."
2. Parse the `created` timestamp and `task` field from the file header
3. For each rule, show:
   - Rule type (import_rules, banned_patterns, custom_checks, blocked_commands)
   - What it enforces (short summary)
   - Age (days since `created`)
   - Task context (from `task` field)
4. Present as numbered list:
   ```
   ## ecko guard review -- N rules (task: auth-refactor, 5 days old)

   ### import_rules
     [1] components/*.tsx -> deny: api, fetch
         "Components must use hooks for API calls"

   ### banned_patterns
     [2] pattern: "from.*models.*import" glob: routes/*.py
         "No direct database imports in routes"
   ```
5. Ask: "Keep all, remove some, or clear all? (e.g., 2 or 1,3 or clear)"
6. Wait for the user's response. Parse their selection.
7. If "clear": delete file. If specific numbers: remove those rules, rewrite file (or delete if empty).
8. Confirm changes

## Generating New Guard Rules

If no flags (or custom arguments describing constraints):

1. **Analyze context.** Use conversation context and $ARGUMENTS to understand the architectural decisions the user wants to enforce. If arguments are empty, infer constraints from the plan discussion. If unclear, ask the user what constraints to enforce.

2. **Scan project structure.** Look at directory layout to understand modules and layers:
   ```bash
   find . -type d -maxdepth 3 -not -path '*/node_modules/*' -not -path '*/.git/*' -not -path '*/target/*' -not -path '*/.venv/*' -not -path '*/venv/*'
   ```

3. **Generate rules.** For each architectural decision, produce specific ecko.yaml-compatible rules:
   - Layer boundaries -> `import_rules` (files glob + deny list)
   - API patterns / deprecated usage -> `banned_patterns` (regex + glob filter)
   - Code patterns -> `custom_checks` (tree-sitter query if language-specific)
   - Command safety -> `blocked_commands` (regex)

4. **Present as numbered interactive list** (same UX as /ecko:tune):
   ```
   ## ecko guard -- N proposed rules

   ### import_rules
     [1] Components must use hooks for API calls
         files: "components/*.tsx"  deny: api, fetch

   ### banned_patterns
     [2] No direct database imports in routes
         pattern: "from.*models.*import"  glob: "routes/*.py"

   Which rules to apply? (e.g., 1,2 or all or none)
   ```

5. Wait for the user's response. Parse their selection (supports `1,3,5` or `1-4` or `all` or `none`). Only apply the selected items.

6. **Write `.ecko-guard.yaml`.** If the file already exists, merge new rules with existing ones (don't overwrite). Generate the `created` timestamp as Unix epoch seconds. Include the `task` field derived from the conversation or arguments.

   Format:
   ```yaml
   created: 1711111800.0
   task: "auth-refactor"

   import_rules:
     - files: "components/*.tsx"
       deny: [api, fetch]
       message: "Components must use hooks for API calls, not direct imports"

   banned_patterns:
     - pattern: "from.*models.*import"
       glob: "routes/*.py"
       message: "No direct database imports in routes"
   ```

7. **Confirm** what was written and tell the user:
   - Rules are active immediately on the next Write/Edit
   - Add `.ecko-guard.yaml` to `.gitignore` if not already present (guard rules are per-developer, not shared)
   - Run `/ecko:guard --review` to review or remove rules later
   - Run `/ecko:guard --clear` to remove all guard rules
   - The stop hook will nudge after 7 days if rules are still active
