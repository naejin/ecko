#!/usr/bin/env bash
# Ecko PreToolUse hook for ExitPlanMode — nudge to include test steps.
set -euo pipefail
echo "~~ ecko ~~ Remember to include test steps for all code changes in this plan." >&2
exit 0
