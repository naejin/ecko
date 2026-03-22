#!/usr/bin/env bash
set -euo pipefail

MARKETPLACE_REPO="naejin/monet-plugins"
MARKETPLACE_NAME="monet-plugins"
PLUGIN_NAME="ecko"

# Colors (only if terminal supports it)
if [ -t 1 ]; then
  BOLD='\033[1m' GREEN='\033[0;32m' RED='\033[0;31m' RESET='\033[0m'
else
  BOLD='' GREEN='' RED='' RESET=''
fi

info()  { echo -e "${BOLD}ecko:${RESET} $1"; }
error() { echo -e "${RED}error:${RESET} $1" >&2; }

# Require Claude Code
if ! command -v claude >/dev/null 2>&1; then
  error "Claude Code not found on PATH."
  error "Install it first: https://docs.anthropic.com/en/docs/claude-code"
  error ""
  error "Then run this script again, or install manually:"
  error "  claude plugin marketplace add ${MARKETPLACE_REPO}"
  error "  claude plugin install ${PLUGIN_NAME}@${MARKETPLACE_NAME}"
  exit 1
fi

# Add marketplace if not already registered
if ! claude plugin marketplace list 2>/dev/null | grep -q "$MARKETPLACE_NAME"; then
  info "Adding marketplace..."
  if ! claude plugin marketplace add "$MARKETPLACE_REPO" 2>&1; then
    error "Failed to add marketplace. Try manually:"
    error "  claude plugin marketplace add ${MARKETPLACE_REPO}"
    exit 1
  fi
fi

# Install or update plugin
if claude plugin list 2>/dev/null | grep -q "${PLUGIN_NAME}@${MARKETPLACE_NAME}"; then
  info "Updating plugin..."
  claude plugin marketplace update "$MARKETPLACE_NAME" 2>&1
  claude plugin update "${PLUGIN_NAME}@${MARKETPLACE_NAME}" 2>&1
else
  info "Installing plugin..."
  if ! claude plugin install "${PLUGIN_NAME}@${MARKETPLACE_NAME}" 2>&1; then
    error "Failed to install plugin. Try manually:"
    error "  claude plugin install ${PLUGIN_NAME}@${MARKETPLACE_NAME}"
    exit 1
  fi
fi

# Ensure binary is available (download/build on first run)
PLUGIN_DIR=""
if command -v python3 >/dev/null 2>&1; then
  PLUGIN_DIR=$(python3 -c "
import json, os, pathlib
config_dir = pathlib.Path.home() / '.claude'
plugins_file = config_dir / 'plugins.json'
if plugins_file.exists():
    for p in json.load(open(plugins_file)):
        if p.get('name') == 'ecko':
            print(p.get('directory', ''))
            break
" 2>/dev/null || true)
fi

if [ -n "$PLUGIN_DIR" ] && [ -f "$PLUGIN_DIR/scripts/run.sh" ]; then
  info "Downloading ecko binary..."
  if "$PLUGIN_DIR/scripts/run.sh" --version >/dev/null 2>&1; then
    info "Binary ready."
  else
    info "Binary download skipped. It will be fetched on first use."
  fi
fi

echo ""
info "${GREEN}Ecko installed!${RESET}"
info "No external tools needed — ecko v2 checks are native."
info "Optional: install pyright, tsc, golangci-lint, or clippy for deep analysis."
info "Restart Claude Code to start using ecko."
echo ""
