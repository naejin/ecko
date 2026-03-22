#!/usr/bin/env bash
# Ecko validation runner — verifies all validation fixtures.
#
# Usage: ./validation/run.sh [language]
#   ./validation/run.sh          # run all languages
#   ./validation/run.sh python   # run only Python
#
# Exit code 0 = all pass, 1 = failures found.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ECKO="$REPO_ROOT/target/release/ecko"

if [ ! -x "$ECKO" ]; then
    printf 'error: ecko binary not found at %s -- run cargo build --release\n' "$ECKO" >&2
    exit 1
fi

PASS=0
FAIL=0
ERRORS=""

check_file() {
    local file="$1"
    local expected_exit="$2"
    local expected_check="${3:-}"
    local lang_dir="$4"

    local output
    output=$("$ECKO" --mode post-tool-use --file "$file" --cwd "$lang_dir" --plugin-root "$REPO_ROOT" 2>&1) || true
    local actual_exit=$?

    # ecko exits 1 for echoes, 0 for clean
    if [ "$actual_exit" -ne 0 ] && [ "$actual_exit" -ne 1 ]; then
        actual_exit=1  # treat other non-zero as "echoes found"
    fi
    # Determine exit from output presence (ecko exit codes can be tricky in pipes)
    if [ -n "$output" ]; then
        actual_exit=1
    else
        actual_exit=0
    fi

    local rel="${file#$SCRIPT_DIR/}"
    local status="PASS"

    if [ "$actual_exit" -ne "$expected_exit" ]; then
        status="FAIL"
        ERRORS="$ERRORS\n  $rel: expected exit $expected_exit, got $actual_exit"
        if [ -n "$output" ]; then
            ERRORS="$ERRORS\n    output: $output"
        fi
    fi

    # If we expect echoes, verify the right check name appears
    if [ "$expected_exit" -eq 1 ] && [ -n "$expected_check" ] && [ "$actual_exit" -eq 1 ]; then
        if ! printf '%s' "$output" | grep -q "$expected_check"; then
            status="FAIL"
            ERRORS="$ERRORS\n  $rel: expected check '$expected_check' not found in output"
            ERRORS="$ERRORS\n    output: $output"
        fi
    fi

    if [ "$status" = "PASS" ]; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
    fi
    printf '  %s %s\n' "$status" "$rel"
}

run_language() {
    local lang="$1"
    local lang_dir="$SCRIPT_DIR/$lang"

    if [ ! -d "$lang_dir" ]; then
        printf 'skip: %s (directory not found)\n' "$lang"
        return
    fi

    printf '%s:\n' "$lang"

    # bad/ files — must trigger exit 1
    if [ -d "$lang_dir/bad" ]; then
        for file in "$lang_dir/bad"/*; do
            [ -f "$file" ] || continue
            # Extract expected check name from header comment
            local check_name
            check_name=$(grep -m1 'check=' "$file" 2>/dev/null | sed 's/.*check=//;s/[[:space:]].*//' || true)
            check_file "$file" 1 "$check_name" "$lang_dir"
        done
    fi

    # clean/ files — must produce exit 0
    if [ -d "$lang_dir/clean" ]; then
        for file in "$lang_dir/clean"/*; do
            [ -f "$file" ] || continue
            check_file "$file" 0 "" "$lang_dir"
        done
    fi

    # boundary/ files — read expected exit from header
    if [ -d "$lang_dir/boundary" ]; then
        for file in "$lang_dir/boundary"/*; do
            [ -f "$file" ] || continue
            local expected
            expected=$(grep -m1 'Expected: exit' "$file" 2>/dev/null | sed 's/.*exit \([0-9]*\).*/\1/' || echo "0")
            local check_name
            check_name=$(grep -m1 'check=' "$file" 2>/dev/null | sed 's/.*check=//;s/[[:space:]].*//' || true)
            check_file "$file" "$expected" "$check_name" "$lang_dir"
        done
    fi
}

# Determine which languages to run
LANGS="${1:-python typescript go rust}"

for lang in $LANGS; do
    run_language "$lang"
done

printf '\n--- Results: %d pass, %d fail ---\n' "$PASS" "$FAIL"

if [ "$FAIL" -gt 0 ]; then
    printf '\nFailures:%b\n' "$ERRORS"
    exit 1
fi
