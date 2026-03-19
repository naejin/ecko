#!/usr/bin/env bash
set -euo pipefail

MARKETPLACE_REPO="naejin/monet-plugins"
MARKETPLACE_NAME="monet-plugins"
PLUGIN_NAME="ecko"

# Colors (only if terminal supports it)
if [ -t 1 ]; then
  BOLD='\033[1m' GREEN='\033[0;32m' YELLOW='\033[0;33m' RED='\033[0;31m' DIM='\033[2m' RESET='\033[0m'
else
  BOLD='' GREEN='' YELLOW='' RED='' DIM='' RESET=''
fi

info()  { echo -e "${BOLD}ecko:${RESET} $1"; }
ok()    { echo -e "${BOLD}ecko:${RESET} ${GREEN}$1${RESET}"; }
warn()  { echo -e "${BOLD}ecko:${RESET} ${YELLOW}$1${RESET}"; }
error() { echo -e "${RED}error:${RESET} $1" >&2; }

show_help() {
  echo "Usage: install.sh [options]"
  echo ""
  echo "Options:"
  echo "  --with-tools    Also install external tools (ruff, black, biome, etc.)"
  echo "  --tools-only    Only install external tools (skip plugin install)"
  echo "  --python-only   Only install Python tools"
  echo "  --node-only     Only install Node.js tools"
  echo "  -h, --help      Show this help"
}

# Parse arguments
WITH_TOOLS=false
TOOLS_ONLY=false
PYTHON_ONLY=false
NODE_ONLY=false

for arg in "$@"; do
  case $arg in
    --with-tools)  WITH_TOOLS=true ;;
    --tools-only)  TOOLS_ONLY=true; WITH_TOOLS=true ;;
    --python-only) PYTHON_ONLY=true; WITH_TOOLS=true ;;
    --node-only)   NODE_ONLY=true; WITH_TOOLS=true ;;
    -h|--help)     show_help; exit 0 ;;
    *)             error "Unknown option: $arg"; show_help; exit 1 ;;
  esac
done

# --- Plugin Installation ---
if [ "$TOOLS_ONLY" = false ]; then
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

  ok "Plugin installed!"
fi

# --- External Tools Installation ---
if [ "$WITH_TOOLS" = true ]; then
  echo ""
  info "Installing external tools..."
  echo ""

  # Detect Python package manager
  PIP_CMD=""
  if [ "$NODE_ONLY" = false ]; then
    if command -v uv >/dev/null 2>&1; then
      PIP_CMD="uv tool install"
      info "Using ${BOLD}uv${RESET} for Python tools"
    elif command -v pipx >/dev/null 2>&1; then
      PIP_CMD="pipx install"
      info "Using ${BOLD}pipx${RESET} for Python tools"
    elif command -v pip >/dev/null 2>&1; then
      PIP_CMD="pip install --user"
      info "Using ${BOLD}pip${RESET} for Python tools"
    elif command -v pip3 >/dev/null 2>&1; then
      PIP_CMD="pip3 install --user"
      info "Using ${BOLD}pip3${RESET} for Python tools"
    else
      warn "No Python package manager found (uv, pipx, pip). Skipping Python tools."
    fi
  fi

  # Detect Node package manager
  NPM_CMD=""
  if [ "$PYTHON_ONLY" = false ]; then
    if command -v npm >/dev/null 2>&1; then
      NPM_CMD="npm install -g"
      info "Using ${BOLD}npm${RESET} for Node tools"
    elif command -v pnpm >/dev/null 2>&1; then
      NPM_CMD="pnpm add -g"
      info "Using ${BOLD}pnpm${RESET} for Node tools"
    else
      warn "No Node package manager found (npm, pnpm). Skipping Node tools."
    fi
  fi

  echo ""

  # Python tools
  PYTHON_TOOLS=("black" "isort" "ruff" "pyright" "vulture")
  if [ -n "$PIP_CMD" ]; then
    for tool in "${PYTHON_TOOLS[@]}"; do
      if command -v "$tool" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓${RESET} ${tool} ${DIM}(already installed)${RESET}"
      else
        echo -ne "  ○ ${tool}..."
        if $PIP_CMD "$tool" >/dev/null 2>&1; then
          echo -e "\r  ${GREEN}✓${RESET} ${tool}"
        else
          echo -e "\r  ${RED}✗${RESET} ${tool} ${DIM}(install failed)${RESET}"
        fi
      fi
    done
  fi

  # Node tools
  NODE_TOOLS=("prettier" "@biomejs/biome" "typescript")
  NODE_BINS=("prettier" "biome" "tsc")
  if [ -n "$NPM_CMD" ]; then
    for i in "${!NODE_TOOLS[@]}"; do
      tool="${NODE_TOOLS[$i]}"
      bin="${NODE_BINS[$i]}"
      if command -v "$bin" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓${RESET} ${tool} ${DIM}(already installed)${RESET}"
      else
        echo -ne "  ○ ${tool}..."
        if $NPM_CMD "$tool" >/dev/null 2>&1; then
          echo -e "\r  ${GREEN}✓${RESET} ${tool}"
        else
          echo -e "\r  ${RED}✗${RESET} ${tool} ${DIM}(install failed)${RESET}"
        fi
      fi
    done
  fi

  # knip runs via npx, no install needed
  if command -v npx >/dev/null 2>&1; then
    echo -e "  ${GREEN}✓${RESET} knip ${DIM}(runs via npx)${RESET}"
  fi

  echo ""
  ok "Tools setup complete!"
fi

echo ""
info "Restart Claude Code to start using ecko."
