#!/usr/bin/env bash
# Ecko PreToolUse hook for Bash — blocks dangerous command patterns.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Read tool input JSON from stdin and extract the command
INPUT=$(cat)
COMMAND=$(printf '%s' "$INPUT" | python3 -c "
import json, sys
data = json.load(sys.stdin)
print(data.get('command', ''))
" 2>/dev/null || { printf '%s\n' "~~ ecko ~~ warning: failed to parse hook input JSON" >&2; printf ''; })

if [ -z "$COMMAND" ]; then
    exit 0
fi

printf '%s' "$COMMAND" | exec python3 "$PLUGIN_ROOT/checks/runner.py" \
    --mode pre-tool-use-bash \
    --cwd "$(pwd)" \
    --plugin-root "$PLUGIN_ROOT"
