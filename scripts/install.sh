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

echo ""
info "${GREEN}Ecko installed!${RESET}"
info "Restart Claude Code to start using ecko."
echo ""

# Check for tool runners
if command -v uvx >/dev/null 2>&1 || command -v npx >/dev/null 2>&1; then
  info "External tools (ruff, biome, etc.) will run automatically via uvx/npx."
else
  info "Tip: install uv (https://docs.astral.sh/uv) or Node.js for full tool coverage."
  info "Ecko works without them — it just runs fewer checks."
fi
