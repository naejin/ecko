#!/usr/bin/env bash
# Ecko PostToolUse hook — Layer 1 (auto-fix) + Layer 2 (echoes)
# Triggered on Write/Edit tool use. Receives tool input via stdin.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$(dirname "$0")/_find_ecko.sh"

# Pipe stdin JSON directly to ecko (it parses file_path internally)
INPUT=$(cat)
printf '%s' "$INPUT" | exec "$ECKO_BIN" \
    --mode post-tool-use \
    --cwd "$(pwd)" \
    --plugin-root "$PLUGIN_ROOT"
