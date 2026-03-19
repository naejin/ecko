#!/usr/bin/env bash
# Ecko PostToolUse hook — Layer 1 (auto-fix) + Layer 2 (echoes)
# Triggered on Write/Edit tool use. Receives tool input via stdin.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Parse the file path from the hook input JSON
# Claude Code passes tool input as JSON on stdin
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | python3 -c "
import json, sys
data = json.load(sys.stdin)
print(data.get('file_path', ''))
" 2>/dev/null || echo "")

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

# Resolve to absolute path
if [[ "$FILE_PATH" != /* ]]; then
    FILE_PATH="$(pwd)/$FILE_PATH"
fi

if [ ! -f "$FILE_PATH" ]; then
    exit 0
fi

exec python3 "$PLUGIN_ROOT/checks/runner.py" \
    --file "$FILE_PATH" \
    --mode post-tool-use \
    --cwd "$(pwd)" \
    --plugin-root "$PLUGIN_ROOT"
