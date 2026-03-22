#!/usr/bin/env bash
# Ecko Stop hook — Layer 3 (deep analysis) + Layer 2 re-sweep
# Triggered when the agent is about to stop.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$(dirname "$0")/_find_ecko.sh"

exec "$ECKO_BIN" \
    --mode stop \
    --cwd "$(pwd)" \
    --plugin-root "$PLUGIN_ROOT"
