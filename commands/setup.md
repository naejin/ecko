---
description: "Install all ecko tools (linters, formatters, type checkers)"
allowed-tools: ["Bash", "Read", "AskUserQuestion"]
---

Help the user install all tools that ecko can use.

Steps:
1. First check which tools are already installed (same as /ecko:status).
2. For any missing tools, ask the user which ones they want to install.
3. Install the requested tools:

   Python tools (pip):
   - `pip install black isort ruff pyright vulture`

   Node tools (npm):
   - `npm install -g prettier @biomejs/biome typescript`
   - knip runs via npx, no global install needed

4. After installation, re-check tool availability and show a summary.

Important: Ask before installing anything. Some users may prefer `pipx`, `uv`, or project-local installs.
