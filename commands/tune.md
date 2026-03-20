---
description: "Analyze learnings and codebase, then recommend ecko.yaml rules and project improvements"
allowed-tools: ["Bash", "Read", "Glob", "Grep", "Edit", "Write"]
---

Enter plan mode. You are ecko's tuning advisor. Your job is to analyze this project and recommend guardrails.

## Step 1: Gather Signal

Check for `.ecko-learnings/` files in the project root. If they exist, read all of them — these are notes from past sessions about surprises, gotchas, and recurring issues.

Also scan the codebase for patterns that suggest useful guardrails:
- Repeated import patterns that suggest architectural layers (e.g., routes importing ORM models)
- Raw values that should use abstractions (e.g., hex colors, hardcoded URLs, magic numbers)
- Naming inconsistencies (old names that should be replaced)
- Common mistakes the agent might make based on the codebase structure

## Step 2: Generate Recommendations

For each finding, propose a **specific, copy-pasteable** `ecko.yaml` configuration entry. Group into categories:

### banned_patterns
Regex patterns that catch anti-patterns before they're committed.
```yaml
banned_patterns:
  - pattern: "..."
    glob: "*.tsx"
    message: "Why this matters"
```

### obsolete_terms
Old names that should be replaced with new ones.
```yaml
obsolete_terms:
  - old: "OldName"
    new: "NewName"
```

### blocked_commands
Bash commands that should be blocked or flagged.
```yaml
blocked_commands:
  - pattern: "..."
    message: "Why this is blocked"
```

### import_rules (v0.5.0)
Architecture boundary enforcement — which modules should not import from which.
```yaml
import_rules:
  - files: "routes/*.py"
    deny_import:
      - repositories
      - sqlalchemy
    message: "Routes must not import from the data layer"
```

### CLAUDE.md improvements
Naming conventions, gotchas, or patterns that should be documented for the agent.

### Code improvements
Renaming confusing variables, removing duplication, simplifying architecture — only changes that reduce future surprises.

## Step 3: Present and Implement

Present all recommendations to the user as a numbered list. For each:
- Show the exact config to add
- Explain why it prevents a recurring issue
- Mark as [RECOMMEND] or [OPTIONAL] based on confidence

Wait for user approval before implementing any changes. Delete processed `.ecko-learnings/` files at the end.

Do not propose changes that are purely cosmetic or that don't prevent recurring mistakes.
