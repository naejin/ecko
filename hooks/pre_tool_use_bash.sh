#!/usr/bin/env bash
# Ecko PreToolUse hook for Bash — blocks dangerous command patterns.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$(dirname "$0")/_find_ecko.sh"

# Pipe stdin JSON directly to ecko (it parses the command field internally)
INPUT=$(cat)
printf '%s' "$INPUT" | exec "$ECKO_BIN" \
    --mode pre-tool-use-bash \
    --cwd "$(pwd)" \
    --plugin-root "$PLUGIN_ROOT"
