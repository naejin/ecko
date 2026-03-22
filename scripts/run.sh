#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$DIR/target/release/ecko"

# 1. Binary exists -- run it
if [ -f "$BIN" ]; then
  exec "$BIN" "$@"
fi

# 2. Source checkout with Rust -- build from source
if [ -f "$DIR/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  cargo build --release --manifest-path "$DIR/Cargo.toml" >&2
  exec "$BIN" "$@"
fi

# 3. Download pre-built binary from GitHub Releases
REPO="naejin/ecko"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  PLATFORM="linux" ;;
  Darwin) PLATFORM="macos" ;;
  *) printf '%s\n' "Error: unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  ARCH="x86_64" ;;
  arm64|aarch64)  ARCH="aarch64" ;;
  *) printf '%s\n' "Error: unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

ARTIFACT="ecko-${PLATFORM}-${ARCH}.tar.gz"

# Get version from plugin.json if available, otherwise fetch latest
VERSION=""
if [ -f "$DIR/.claude-plugin/plugin.json" ] && command -v python3 >/dev/null 2>&1; then
  VERSION=$(python3 -c "import json; print('v'+json.load(open('$DIR/.claude-plugin/plugin.json'))['version'])" 2>/dev/null || true)
fi
if [ -z "$VERSION" ]; then
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
fi

if [ -z "$VERSION" ]; then
  printf '%s\n' "Error: could not determine ecko version to download." >&2
  printf '%s\n' "Install Rust (https://rustup.rs) and build from source, or download manually from https://github.com/${REPO}/releases" >&2
  exit 1
fi

printf '%s\n' "Downloading ecko ${VERSION} (${PLATFORM}-${ARCH})..." >&2
TMPDIR_DL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_DL"' EXIT

URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"
if ! curl -fsSL -o "$TMPDIR_DL/$ARTIFACT" "$URL" 2>/dev/null; then
  printf '%s\n' "Error: failed to download ecko binary from ${URL}" >&2
  printf '%s\n' "Install manually: curl -fsSL https://github.com/${REPO}/releases/latest/download/install.sh -o /tmp/ecko-install.sh && bash /tmp/ecko-install.sh" >&2
  exit 1
fi

# Verify checksum if sha256sum is available
CHECKSUM_URL="https://github.com/${REPO}/releases/download/${VERSION}/checksums.txt"
if command -v sha256sum >/dev/null 2>&1; then
  if curl -fsSL -o "$TMPDIR_DL/checksums.txt" "$CHECKSUM_URL" 2>/dev/null; then
    EXPECTED=$(grep "$ARTIFACT" "$TMPDIR_DL/checksums.txt" | awk '{print $1}')
    if [ -n "$EXPECTED" ]; then
      ACTUAL=$(sha256sum "$TMPDIR_DL/$ARTIFACT" | awk '{print $1}')
      if [ "$EXPECTED" != "$ACTUAL" ]; then
        printf '%s\n' "Error: checksum mismatch for ${ARTIFACT}" >&2
        printf '%s\n' "  expected: ${EXPECTED}" >&2
        printf '%s\n' "  actual:   ${ACTUAL}" >&2
        exit 1
      fi
    fi
  fi
# macOS uses shasum instead of sha256sum
elif command -v shasum >/dev/null 2>&1; then
  if curl -fsSL -o "$TMPDIR_DL/checksums.txt" "$CHECKSUM_URL" 2>/dev/null; then
    EXPECTED=$(grep "$ARTIFACT" "$TMPDIR_DL/checksums.txt" | awk '{print $1}')
    if [ -n "$EXPECTED" ]; then
      ACTUAL=$(shasum -a 256 "$TMPDIR_DL/$ARTIFACT" | awk '{print $1}')
      if [ "$EXPECTED" != "$ACTUAL" ]; then
        printf '%s\n' "Error: checksum mismatch for ${ARTIFACT}" >&2
        printf '%s\n' "  expected: ${EXPECTED}" >&2
        printf '%s\n' "  actual:   ${ACTUAL}" >&2
        exit 1
      fi
    fi
  fi
fi

tar xzf "$TMPDIR_DL/$ARTIFACT" -C "$TMPDIR_DL"
mkdir -p "$DIR/target/release"
cp "$TMPDIR_DL/ecko/target/release/ecko" "$BIN"
chmod +x "$BIN"
printf '%s\n' "Downloaded ecko ${VERSION} successfully." >&2

exec "$BIN" "$@"
