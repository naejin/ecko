#!/usr/bin/env bash
# Shared binary resolution for ecko hooks.
# Source this file from hook scripts: . "$(dirname "$0")/_find_ecko.sh"
#
# Sets ECKO_BIN to the ecko binary path if found.
# Exits 0 (graceful skip) with a warning if the binary is not found.

ECKO_BIN=""
if [ -x "$PLUGIN_ROOT/ecko" ]; then
    ECKO_BIN="$PLUGIN_ROOT/ecko"
elif [ -x "$PLUGIN_ROOT/target/release/ecko" ]; then
    ECKO_BIN="$PLUGIN_ROOT/target/release/ecko"
elif [ -x "$PLUGIN_ROOT/target/debug/ecko" ]; then
    ECKO_BIN="$PLUGIN_ROOT/target/debug/ecko"
elif command -v ecko >/dev/null 2>&1; then
    ECKO_BIN="ecko"
else
    printf '%s\n' "~~ ecko ~~ warning: ecko binary not found" >&2
    exit 0
fi
